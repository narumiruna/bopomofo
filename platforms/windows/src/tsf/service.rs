use super::{ModuleGuard, com, edit, key, module_path};
use crate::session::{Session, Ticket, boundary};
use ime_core::{Event, KeyEvent, Lexicon, Output};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use windows::Win32::Foundation::*;
use windows::Win32::System::Variant::{VT_EMPTY, VT_I4, VariantClear};
use windows::Win32::UI::TextServices::*;
use windows::core::*;

pub(super) struct Context {
    pub native: ITfContext,
    pub client: u32,
    pub ui: ITfUIElementMgr,
    pub inner: RefCell<ContextInner>,
    _module: ModuleGuard,
}

pub(super) struct ContextInner {
    pub session: Session,
    pub pending: VecDeque<(Ticket, Output)>,
    pub composition: Option<ITfComposition>,
    pub presenter: Option<super::candidate::Presenter>,
    pub focused: bool,
    pub valid: bool,
    pub editing: bool,
    pub scheduled: bool,
}

impl Context {
    pub fn dispatch(self: &Rc<Self>, event: Event) -> Result<bool> {
        let epoch = self
            .inner
            .try_borrow()
            .map_err(|_| Error::from(E_UNEXPECTED))?
            .session
            .epoch();
        let result = com(|| self.dispatch_inner(event));
        if result.is_err()
            && self
                .inner
                .try_borrow()
                .is_ok_and(|state| state.session.epoch() == epoch)
        {
            self.fail();
        }
        result
    }

    fn dispatch_inner(self: &Rc<Self>, event: Event) -> Result<bool> {
        let consumed = {
            let mut state = self
                .inner
                .try_borrow_mut()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            if !state.valid || !state.focused {
                return Ok(false);
            }
            if state.pending.len() >= 256 {
                return Err(TF_E_LOCKED.into());
            }
            let (ticket, output) = state.session.event(event);
            let consumed = output.consumed;
            state.pending.push_back((ticket, output));
            consumed
        };
        edit::request(self)?;
        Ok(consumed)
    }

    pub fn cancel(self: &Rc<Self>, event: Event, valid: bool) -> Result<()> {
        let presenter = {
            let mut state = self
                .inner
                .try_borrow_mut()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            if !matches!(event, Event::Reset) {
                state.focused = false;
            }
            state.valid = valid;
            state.pending.clear();
            let output = state.session.invalidate(event);
            state.pending.push_back(output);
            state.presenter.take()
        };
        drop(presenter);
        edit::request(self)
    }

    /// Reset after an error without replaying an output that could have committed.
    pub fn fail(self: &Rc<Self>) {
        let _ = boundary(|| {
            let valid = self
                .inner
                .try_borrow()
                .map(|state| state.valid)
                .unwrap_or(false);
            self.cancel(Event::Reset, valid)
        });
    }

    pub fn select(self: &Rc<Self>, epoch: u64, generation: u64, index: usize) -> Result<()> {
        {
            let state = self
                .inner
                .try_borrow()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            if !state.valid
                || !state.focused
                || !state.session.selection_is_current(epoch, generation)
            {
                return Err(E_INVALIDARG.into());
            }
            if index >= state.session.snapshot().candidates.len() {
                return Err(E_INVALIDARG.into());
            }
        }
        self.dispatch(Event::Select { generation, index })
            .map(|_| ())
    }

    pub fn abort_selection(self: &Rc<Self>) -> Result<()> {
        let pressed = self.dispatch(Event::Key(KeyEvent {
            key: "escape".into(),
            ..KeyEvent::default()
        }));
        // This UI action is a full logical tap, not a held physical key.
        let released = self.dispatch(Event::Key(KeyEvent {
            key: "escape".into(),
            release: true,
            ..KeyEvent::default()
        }));
        pressed.and(released).map(|_| ())
    }
}

pub(super) fn identity<T: Interface>(object: &T) -> Result<usize> {
    Ok(object.cast::<IUnknown>()?.as_raw() as usize)
}

/// Treat missing/invalid restrictions as disabled, not as permission to bypass TSF.
pub(super) fn allowed(context: &ITfContext) -> bool {
    unsafe {
        let Ok(status) = context.GetStatus() else {
            return false;
        };
        if status.dwDynamicFlags & (TF_SD_READONLY | TF_SD_LOADING) != 0 {
            return false;
        }
        let Ok(manager) = context.cast::<ITfCompartmentMgr>() else {
            return false;
        };
        for guid in [
            GUID_COMPARTMENT_KEYBOARD_DISABLED,
            GUID_COMPARTMENT_EMPTYCONTEXT,
        ] {
            let Ok(compartment) = manager.GetCompartment(&guid) else {
                return false;
            };
            let Ok(mut value) = compartment.GetValue() else {
                return false;
            };
            let variant = &value.Anonymous.Anonymous;
            let enabled = if variant.vt == VT_EMPTY {
                true
            } else {
                variant.vt == VT_I4 && variant.Anonymous.lVal == 0
            };
            let _ = VariantClear(&mut value);
            if !enabled {
                return false;
            }
        }
        true
    }
}

struct Activation {
    manager: ITfThreadMgr,
    keys: ITfKeystrokeMgr,
    source: ITfSource,
    cookie: u32,
    client: u32,
    ui: ITfUIElementMgr,
    lexicon: Arc<Lexicon>,
    contexts: HashMap<usize, Rc<Context>>,
    focus: Option<usize>,
}

#[implement(
    ITfTextInputProcessor,
    ITfKeyEventSink,
    ITfThreadMgrEventSink,
    Agile = false
)]
pub(super) struct Service {
    activation: RefCell<Option<Activation>>,
    _module: ModuleGuard,
}

impl Service {
    pub fn new() -> Self {
        Self {
            activation: RefCell::new(None),
            _module: ModuleGuard::new(),
        }
    }
}

impl Service_Impl {
    fn guarded<T>(&self, call: impl FnOnce() -> Result<T>) -> Result<T> {
        match boundary(call) {
            Ok(result) => result,
            Err(()) => {
                if let Ok(state) = self.activation.try_borrow() {
                    let contexts: Vec<_> = state
                        .as_ref()
                        .map(|state| state.contexts.values().cloned().collect())
                        .unwrap_or_default();
                    drop(state);
                    for context in contexts {
                        context.fail();
                    }
                }
                Err(E_UNEXPECTED.into())
            }
        }
    }

    fn current(&self, native: &ITfContext) -> Result<Option<Rc<Context>>> {
        let id = identity(native)?;
        let state = self
            .activation
            .try_borrow()
            .map_err(|_| Error::from(E_UNEXPECTED))?;
        Ok(state
            .as_ref()
            .filter(|state| state.focus == Some(id))
            .and_then(|state| state.contexts.get(&id).cloned()))
    }

    fn focused_top(&self) -> Result<Option<ITfContext>> {
        let manager = self
            .activation
            .try_borrow()
            .map_err(|_| Error::from(E_UNEXPECTED))?
            .as_ref()
            .map(|state| state.manager.clone());
        Ok(manager.and_then(|manager| unsafe {
            manager
                .GetFocus()
                .and_then(|document| document.GetTop())
                .ok()
        }))
    }

    fn focus(&self, native: Option<ITfContext>) -> Result<()> {
        let next = native.as_ref().map(identity).transpose()?;
        let (previous, current) = {
            let mut activation = self
                .activation
                .try_borrow_mut()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            let Some(state) = activation.as_mut() else {
                return Ok(());
            };
            if state.focus == next {
                return Ok(());
            }
            let previous = state.focus.and_then(|id| state.contexts.get(&id).cloned());
            let current = if let (Some(id), Some(native)) = (next, native) {
                Some(
                    state
                        .contexts
                        .entry(id)
                        .or_insert_with(|| {
                            Rc::new(Context {
                                native,
                                client: state.client,
                                ui: state.ui.clone(),
                                inner: RefCell::new(ContextInner {
                                    session: Session::new(state.lexicon.clone()),
                                    pending: VecDeque::new(),
                                    composition: None,
                                    presenter: None,
                                    focused: false,
                                    valid: true,
                                    editing: false,
                                    scheduled: false,
                                }),
                                _module: ModuleGuard::new(),
                            })
                        })
                        .clone(),
                )
            } else {
                None
            };
            state.focus = next;
            (previous, current)
        };
        let canceled = previous.map_or(Ok(()), |context| context.cancel(Event::FocusLost, true));
        let activated = (|| {
            let Some(context) = current else {
                return Ok(());
            };
            // Native cleanup can reenter focus callbacks. Do not reactivate a
            // context if focus moved again or a nested callback activated it.
            let is_current = self
                .activation
                .try_borrow()
                .map_err(|_| Error::from(E_UNEXPECTED))?
                .as_ref()
                .is_some_and(|state| state.focus == next);
            if !is_current {
                return Ok(());
            }
            {
                let mut state = context
                    .inner
                    .try_borrow_mut()
                    .map_err(|_| Error::from(E_UNEXPECTED))?;
                if !state.valid || state.focused {
                    return Ok(());
                }
                state.focused = true;
            }
            context.dispatch(Event::Activate).map(|_| ())
        })();
        canceled.and(activated)
    }

    fn remove(&self, native: &ITfContext) -> Result<()> {
        let id = identity(native)?;
        let context = {
            let mut state = self
                .activation
                .try_borrow_mut()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            let Some(state) = state.as_mut() else {
                return Ok(());
            };
            if state.focus == Some(id) {
                state.focus = None;
            }
            state.contexts.remove(&id)
        };
        if let Some(context) = context {
            context.cancel(Event::Deactivate, false)?;
        }
        Ok(())
    }

    fn key(
        &self,
        native: Ref<ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
        release: bool,
        test: bool,
    ) -> Result<BOOL> {
        self.guarded(|| {
            let Some(native) = native.as_ref() else {
                return Ok(false.into());
            };
            let Some(context) = self.current(native)? else {
                return Ok(false.into());
            };
            if !allowed(native) {
                // Test callbacks must not change even lifecycle state.
                if !test {
                    context.cancel(Event::Reset, true)?;
                }
                return Ok(false.into());
            }
            let Some(event) = key::translate(wparam, lparam, release) else {
                // No logical modifier edge when the other physical Shift is held.
                return Ok(false.into());
            };
            if test {
                let state = context
                    .inner
                    .try_borrow()
                    .map_err(|_| Error::from(E_UNEXPECTED))?;
                return Ok(
                    (state.valid && state.focused && state.session.test_delivery(&event)).into(),
                );
            }
            context.dispatch(Event::Key(event)).map(BOOL::from)
        })
    }

    fn deactivate(&self) -> Result<()> {
        let state = self
            .activation
            .try_borrow_mut()
            .map_err(|_| Error::from(E_UNEXPECTED))?
            .take();
        if let Some(state) = state {
            // Invalidate first: unadvising can reenter the service.
            for context in state.contexts.values() {
                let _ = context.cancel(Event::Deactivate, false);
            }
            unsafe {
                let _ = state.keys.UnadviseKeyEventSink(state.client);
                let _ = state.source.UnadviseSink(state.cookie);
            }
        }
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for Service_Impl {
    fn Activate(&self, manager: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        self.guarded(|| {
            self.deactivate()?;
            let manager = manager
                .as_ref()
                .ok_or_else(|| Error::from(E_POINTER))?
                .clone();
            // File I/O and index construction occur once per activation, never on key callbacks.
            let lexicon = load_lexicon()?;
            let keys: ITfKeystrokeMgr = manager.cast()?;
            let source: ITfSource = manager.cast()?;
            let ui: ITfUIElementMgr = manager.cast()?;
            let key_sink: ITfKeyEventSink = self.to_interface();
            let thread_sink: ITfThreadMgrEventSink = self.to_interface();
            let cookie = unsafe { source.AdviseSink(&ITfThreadMgrEventSink::IID, &thread_sink)? };
            if let Err(error) = unsafe { keys.AdviseKeyEventSink(client, &key_sink, true) } {
                unsafe {
                    let _ = source.UnadviseSink(cookie);
                }
                return Err(error);
            }
            *self.activation.borrow_mut() = Some(Activation {
                manager: manager.clone(),
                keys,
                source,
                cookie,
                client,
                ui,
                lexicon,
                contexts: HashMap::new(),
                focus: None,
            });
            let focused = unsafe {
                manager
                    .GetFocus()
                    .and_then(|document| document.GetTop())
                    .ok()
            };
            if let Err(error) = self.focus(focused) {
                let _ = self.deactivate();
                return Err(error);
            }
            Ok(())
        })
    }
    fn Deactivate(&self) -> Result<()> {
        self.guarded(|| self.deactivate())
    }
}

fn load_lexicon() -> Result<Arc<Lexicon>> {
    let directory = module_path()?
        .parent()
        .ok_or_else(|| Error::from(E_FAIL))?
        .to_path_buf();
    crate::resource::load(&directory).map_err(|_| E_FAIL.into())
}

impl ITfKeyEventSink_Impl for Service_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        self.guarded(|| {
            if !foreground.as_bool() {
                return self.focus(None);
            }
            self.focus(self.focused_top()?)
        })
    }
    fn OnTestKeyDown(&self, context: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        self.key(context, w, l, false, true)
    }
    fn OnTestKeyUp(&self, context: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        self.key(context, w, l, true, true)
    }
    fn OnKeyDown(&self, context: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        self.key(context, w, l, false, false)
    }
    fn OnKeyUp(&self, context: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        self.key(context, w, l, true, false)
    }
    fn OnPreservedKey(&self, _: Ref<ITfContext>, _: *const GUID) -> Result<BOOL> {
        com(|| Ok(false.into()))
    }
}

impl ITfThreadMgrEventSink_Impl for Service_Impl {
    fn OnInitDocumentMgr(&self, _: Ref<ITfDocumentMgr>) -> Result<()> {
        com(|| Ok(()))
    }
    fn OnUninitDocumentMgr(&self, document: Ref<ITfDocumentMgr>) -> Result<()> {
        self.guarded(|| {
            let Some(document) = document.as_ref() else {
                return Ok(());
            };
            let id = identity(document)?;
            let contexts: Vec<_> = self
                .activation
                .borrow()
                .as_ref()
                .map(|state| state.contexts.values().cloned().collect())
                .unwrap_or_default();
            for context in contexts {
                if unsafe { context.native.GetDocumentMgr() }
                    .ok()
                    .and_then(|doc| identity(&doc).ok())
                    == Some(id)
                {
                    self.remove(&context.native)?;
                }
            }
            Ok(())
        })
    }
    fn OnSetFocus(&self, focus: Ref<ITfDocumentMgr>, _: Ref<ITfDocumentMgr>) -> Result<()> {
        self.guarded(|| self.focus(focus.as_ref().and_then(|doc| unsafe { doc.GetTop().ok() })))
    }
    fn OnPushContext(&self, context: Ref<ITfContext>) -> Result<()> {
        self.guarded(|| {
            let Some(context) = context.as_ref() else {
                return Ok(());
            };
            let focused = self.focused_top()?;
            if focused.as_ref().map(identity).transpose()? == Some(identity(context)?) {
                self.focus(Some(context.clone()))?;
            }
            Ok(())
        })
    }
    fn OnPopContext(&self, context: Ref<ITfContext>) -> Result<()> {
        self.guarded(|| {
            let Some(context) = context.as_ref() else {
                return Ok(());
            };
            let removed = self.remove(context);
            // Pop need not change document focus. Restore its new top even if
            // cleanup of the removed context failed.
            let focused = self.focused_top().and_then(|native| self.focus(native));
            removed.and(focused)
        })
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
