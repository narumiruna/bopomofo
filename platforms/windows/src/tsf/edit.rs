use super::service::{Context, identity};
use super::{ModuleGuard, candidate, com};
use crate::session::{Ticket, boundary};
use crate::text::TextSnapshot;
use ime_core::{Event, Output};
use std::cell::Cell;
use std::rc::{Rc, Weak};
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

pub(super) fn request(context: &Rc<Context>) -> Result<()> {
    let epoch = {
        let mut state = context
            .inner
            .try_borrow_mut()
            .map_err(|_| Error::from(E_UNEXPECTED))?;
        if state.pending.is_empty() || state.editing || state.scheduled {
            return Ok(());
        }
        state.scheduled = true;
        state.session.epoch()
    };
    let called = Rc::new(Cell::new(false));
    let session: ITfEditSession = Edit {
        context: context.clone(),
        epoch,
        called: called.clone(),
        _module: ModuleGuard::new(),
    }
    .into();
    let result = unsafe {
        context
            .native
            .RequestEditSession(context.client, &session, TF_ES_SYNC | TF_ES_READWRITE)
    };
    match result {
        Ok(result) if result.is_ok() => Ok(()),
        result
            if !called.get()
                && match &result {
                    Ok(code) => *code == TF_E_SYNCHRONOUS || *code == TF_E_LOCKED,
                    Err(error) => error.code() == TF_E_SYNCHRONOUS || error.code() == TF_E_LOCKED,
                } =>
        {
            // Only retry a refused lock, never an edit that was executed.
            match unsafe {
                context.native.RequestEditSession(
                    context.client,
                    &session,
                    TF_ES_ASYNC | TF_ES_READWRITE,
                )
            } {
                Ok(result) if result.is_ok() => Ok(()),
                result => {
                    context.inner.borrow_mut().scheduled = false;
                    result.and_then(|code| code.ok())
                }
            }
        }
        result => {
            context.inner.borrow_mut().scheduled = false;
            result.and_then(|code| code.ok())
        }
    }
}

#[implement(ITfEditSession, Agile = false)]
struct Edit {
    // Keep the context alive until cancellation is applied, even after Deactivate.
    context: Rc<Context>,
    epoch: u64,
    called: Rc<Cell<bool>>,
    _module: ModuleGuard,
}
impl ITfEditSession_Impl for Edit_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        self.called.set(true);
        let result = com(|| {
            {
                let mut state = self
                    .context
                    .inner
                    .try_borrow_mut()
                    .map_err(|_| Error::from(E_UNEXPECTED))?;
                state.scheduled = false;
                if state.session.epoch() != self.epoch {
                    drop(state);
                    return request(&self.context);
                }
                if state.editing {
                    return Err(TF_E_LOCKED.into());
                }
                state.editing = true;
            }
            let result = boundary(|| flush(&self.context, cookie, self.epoch));
            self.context.inner.borrow_mut().editing = false;
            match result {
                Ok(Ok(())) => request(&self.context),
                Ok(Err(error)) => Err(error),
                Err(()) => Err(E_UNEXPECTED.into()),
            }
        });
        if result.is_err()
            && self
                .context
                .inner
                .try_borrow()
                .is_ok_and(|state| state.session.epoch() != self.epoch)
        {
            // A reentrant focus/lifecycle change owns the new state. An old
            // session's host error must not reset that replacement context.
            let _ = com(|| request(&self.context));
            return Ok(());
        }
        if result.is_err() {
            // Clear the text with the current write cookie. Do not replay failed commits.
            let _ = boundary(|| {
                let (composition, presenter) = {
                    let mut state = self.context.inner.borrow_mut();
                    state.pending.clear();
                    state.session.invalidate(Event::Reset);
                    state.editing = false;
                    (state.composition.take(), state.presenter.take())
                };
                drop(presenter);
                if let Some(composition) = composition {
                    unsafe {
                        if let Ok(range) = composition.GetRange() {
                            let _ = range.SetText(cookie, 0, &[]);
                        }
                        let _ = composition.EndComposition(cookie);
                    }
                }
            });
        }
        result
    }
}

fn current(context: &Context, ticket: Ticket) -> bool {
    context
        .inner
        .try_borrow()
        .is_ok_and(|state| state.session.epoch() == ticket.epoch)
}

fn flush(context: &Rc<Context>, cookie: u32, epoch: u64) -> Result<()> {
    loop {
        let pending = {
            let mut state = context
                .inner
                .try_borrow_mut()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            if state.session.epoch() != epoch {
                return Ok(());
            }
            let output = state.pending.pop_front();
            if let Some((ticket, _)) = output.as_ref()
                && !state.session.accept(*ticket)
            {
                return Err(E_UNEXPECTED.into());
            }
            output
        };
        let Some((ticket, output)) = pending else {
            return Ok(());
        };
        apply(context, cookie, ticket, output)?;
    }
}

fn take_composition(context: &Context) -> Option<ITfComposition> {
    context.inner.borrow_mut().composition.take()
}

fn selection(context: &Context, cookie: u32, range: &ITfRange) -> Result<()> {
    // TF_SELECTION uses ManuallyDrop for its COM pointer. Borrow the interface
    // rather than leak one reference at every cursor update.
    let mut value = TF_SELECTION {
        range: std::mem::ManuallyDrop::new(Some(range.clone())),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_NONE,
            fInterimChar: false.into(),
        },
    };
    let result = unsafe {
        context
            .native
            .SetSelection(cookie, std::slice::from_ref(&value))
    };
    unsafe {
        std::mem::ManuallyDrop::drop(&mut value.range);
    }
    result
}

fn finish(context: &Context, cookie: u32, ticket: Ticket, text: &[u16]) -> Result<()> {
    if let Some(composition) = take_composition(context) {
        let result = (|| unsafe {
            let range = composition.GetRange()?;
            if !current(context, ticket) {
                return Ok(());
            }
            range.SetText(cookie, 0, text)?;
            if !current(context, ticket) {
                return Ok(());
            }
            // Set the insertion point at the end, before ending this composition.
            range.Collapse(cookie, TF_ANCHOR_END)?;
            selection(context, cookie, &range)
        })();
        // Always end the composition, including when the host rejects SetText.
        // The output was reserved already and must not be retried.
        let end = unsafe { composition.EndComposition(cookie) };
        result.and(end)?;
    } else if !text.is_empty() {
        let insert: ITfInsertAtSelection = context.native.cast()?;
        if current(context, ticket) {
            unsafe {
                insert.InsertTextAtSelection(cookie, TF_IAS_NO_DEFAULT_COMPOSITION, text)?;
            }
        }
    }
    Ok(())
}

fn apply(context: &Rc<Context>, cookie: u32, ticket: Ticket, output: Output) -> Result<()> {
    // Validate every core offset before changing the host document.
    let text = TextSnapshot::checked(&output.snapshot).map_err(|_| Error::from(E_INVALIDARG))?;
    if !current(context, ticket) {
        return Ok(());
    }
    // A queued edit must recheck restrictions: TSF can disable a context after
    // key delivery but before granting this asynchronous write lock.
    if (!text.text.is_empty() || output.commit.is_some())
        && !super::service::allowed(&context.native)
    {
        if !current(context, ticket) {
            return Ok(());
        }
        let valid = context.inner.borrow().valid;
        context.cancel(Event::Reset, valid)?;
        return Ok(());
    }
    if let Some(commit) = output.commit {
        let commit: Vec<u16> = commit.encode_utf16().collect();
        i32::try_from(commit.len()).map_err(|_| Error::from(E_INVALIDARG))?;
        finish(context, cookie, ticket, &commit)?;
    } else if text.text.is_empty() {
        finish(context, cookie, ticket, &[])?;
    }
    if !current(context, ticket) {
        return Ok(());
    }
    let mut caret = RECT::default();
    if !text.text.is_empty() {
        let existing = context.inner.borrow().composition.clone();
        let composition = if let Some(composition) = existing {
            composition
        } else {
            let insert: ITfInsertAtSelection = context.native.cast()?;
            let manager: ITfContextComposition = context.native.cast()?;
            let range = unsafe { insert.InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, &[])? };
            let sink: ITfCompositionSink = CompositionSink {
                context: Rc::downgrade(context),
                epoch: ticket.epoch,
                _module: ModuleGuard::new(),
            }
            .into();
            let composition = unsafe { manager.StartComposition(cookie, &range, &sink)? };
            if !current(context, ticket) {
                unsafe {
                    let _ = composition.EndComposition(cookie);
                }
                return Ok(());
            }
            context.inner.borrow_mut().composition = Some(composition.clone());
            composition
        };
        unsafe {
            let range = composition.GetRange()?;
            if !current(context, ticket) {
                return Ok(());
            }
            range.SetText(cookie, 0, &text.text)?;
            if !current(context, ticket) {
                return Ok(());
            }
            let cursor = range.Clone()?;
            cursor.Collapse(cookie, TF_ANCHOR_START)?;
            let mut shifted = 0;
            cursor.ShiftEnd(cookie, text.cursor, &mut shifted, std::ptr::null())?;
            if shifted != text.cursor {
                return Err(E_INVALIDARG.into());
            }
            cursor.Collapse(cookie, TF_ANCHOR_END)?;
            selection(context, cookie, &cursor)?;
            if !current(context, ticket) {
                return Ok(());
            }
            let mut clipped = false.into();
            // Candidate spans are syllable indices. Position the native list
            // at the validated text caret, not at a fabricated character span.
            let view = context.native.GetActiveView()?;
            if let Err(error) = view.GetTextExt(cookie, &cursor, &mut caret, &mut clipped) {
                if error.code() != TS_E_NOLAYOUT {
                    return Err(error);
                }
                // Some stores defer layout until the write lock is released.
                // Use the native view origin until the next available caret rect.
                caret = view.GetScreenExt()?;
                caret.right = caret.left;
                caret.bottom = caret.top;
            }
        }
    }
    if current(context, ticket) {
        candidate::update(context, &output.snapshot, caret)?;
    }
    Ok(())
}

#[implement(ITfCompositionSink, Agile = false)]
struct CompositionSink {
    context: Weak<Context>,
    epoch: u64,
    _module: ModuleGuard,
}
impl ITfCompositionSink_Impl for CompositionSink_Impl {
    fn OnCompositionTerminated(&self, cookie: u32, composition: Ref<ITfComposition>) -> Result<()> {
        com(|| {
            let Some(context) = self.context.upgrade() else {
                return Ok(());
            };
            let Some(composition) = composition.as_ref() else {
                return Err(E_POINTER.into());
            };
            let actual = identity(composition)?;
            let current = context.inner.borrow().composition.clone();
            if context.inner.borrow().session.epoch() != self.epoch
                || current.as_ref().map(identity).transpose()? != Some(actual)
            {
                return Ok(());
            }
            let presenter = {
                let mut state = context.inner.borrow_mut();
                state.composition = None;
                state.pending.clear();
                state.session.invalidate(Event::Reset);
                state.presenter.take()
            };
            drop(presenter);
            // External termination cancels, never converts preedit into an implicit commit.
            unsafe { composition.GetRange()?.SetText(cookie, 0, &[]) }
        })
    }
}
