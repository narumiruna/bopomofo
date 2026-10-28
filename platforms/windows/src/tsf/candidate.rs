use super::service::Context;
use super::{GUID_CANDIDATES, ModuleGuard, com, window};
use ime_core::{PAGE_SIZE, Snapshot};
use std::cell::Cell;
use std::rc::{Rc, Weak};
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

pub(super) struct Data {
    pub context: Weak<Context>,
    pub epoch: u64,
    pub generation: u64,
    pub texts: Vec<String>,
    pub selected: Cell<u32>,
    pub page: Cell<u32>,
    pub shown: Cell<bool>,
    pub live: Cell<bool>,
    pub hwnd: Cell<HWND>,
}
impl Data {
    pub fn check(&self) -> Result<Rc<Context>> {
        if !self.live.get() {
            return Err(E_INVALIDARG.into());
        }
        let context = self
            .context
            .upgrade()
            .ok_or_else(|| Error::from(E_INVALIDARG))?;
        let current = {
            let state = context
                .inner
                .try_borrow()
                .map_err(|_| Error::from(E_UNEXPECTED))?;
            state.valid
                && state.focused
                && state
                    .session
                    .selection_is_current(self.epoch, self.generation)
        };
        if !current {
            return Err(E_INVALIDARG.into());
        }
        Ok(context)
    }

    pub fn select(&self, index: u32) -> Result<()> {
        if index as usize >= self.texts.len() {
            return Err(E_INVALIDARG.into());
        }
        let context = self.check()?;
        context.select(self.epoch, self.generation, index as usize)
    }
}

pub(super) struct Presenter {
    manager: ITfUIElementMgr,
    id: u32,
    _element: ITfCandidateListUIElementBehavior,
    data: Rc<Data>,
    _window: window::CandidateWindow,
}
impl Drop for Presenter {
    fn drop(&mut self) {
        self.data.live.set(false);
        // EndUIElement can call the host; never drop while a context is borrowed.
        let _ = super::com(|| unsafe { self.manager.EndUIElement(self.id) });
    }
}

pub(super) fn update(context: &Rc<Context>, snapshot: &Snapshot, caret: RECT) -> Result<()> {
    let old = context.inner.borrow_mut().presenter.take();
    drop(old);
    if !snapshot.selecting || snapshot.candidates.is_empty() {
        return Ok(());
    }
    let epoch = {
        let state = context.inner.borrow();
        let epoch = state.session.epoch();
        if !state.valid
            || !state.focused
            || !state
                .session
                .selection_is_current(epoch, snapshot.generation)
        {
            return Ok(());
        }
        epoch
    };
    let data = Rc::new(Data {
        context: Rc::downgrade(context),
        epoch,
        generation: snapshot.generation,
        texts: snapshot
            .candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect(),
        selected: Cell::new(snapshot.candidate_index as u32),
        page: Cell::new(snapshot.page as u32),
        shown: Cell::new(true),
        live: Cell::new(true),
        hwnd: Cell::new(HWND::default()),
    });
    let window = window::CandidateWindow::new(data.clone(), caret)?;
    if data.check().is_err() {
        return Ok(());
    }
    data.hwnd.set(window.hwnd());
    let element: ITfCandidateListUIElementBehavior = CandidateElement {
        data: data.clone(),
        _module: ModuleGuard::new(),
    }
    .into();
    let ui: ITfUIElement = element.cast()?;
    let mut show = true.into();
    let mut id = 0;
    unsafe {
        context.ui.BeginUIElement(&ui, &mut show, &mut id)?;
    }
    let presenter = Presenter {
        manager: context.ui.clone(),
        id,
        _element: element,
        data: data.clone(),
        _window: window,
    };
    // A UI-less host sets show=FALSE and draws the list. Otherwise use a native,
    // non-activating Windows LISTBOX, not a cross-platform candidate window.
    data.shown.set(show.as_bool());
    unsafe {
        let _ = ShowWindow(
            data.hwnd.get(),
            if show.as_bool() {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
    }
    unsafe {
        context.ui.UpdateUIElement(id)?;
    }
    {
        let mut state = context.inner.borrow_mut();
        if !state.valid
            || !state.focused
            || !state
                .session
                .selection_is_current(epoch, snapshot.generation)
        {
            drop(state);
            drop(presenter);
            return Ok(());
        }
        state.presenter = Some(presenter);
    }
    Ok(())
}

#[implement(ITfCandidateListUIElementBehavior, Agile = false)]
struct CandidateElement {
    data: Rc<Data>,
    _module: ModuleGuard,
}
impl ITfUIElement_Impl for CandidateElement_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        com(|| Ok(BSTR::from("bopomofo candidates")))
    }
    fn GetGUID(&self) -> Result<GUID> {
        com(|| Ok(GUID_CANDIDATES))
    }
    fn Show(&self, show: BOOL) -> Result<()> {
        com(|| {
            self.data.check()?;
            self.data.shown.set(show.as_bool());
            unsafe {
                let _ = ShowWindow(
                    self.data.hwnd.get(),
                    if show.as_bool() {
                        SW_SHOWNOACTIVATE
                    } else {
                        SW_HIDE
                    },
                );
            }
            Ok(())
        })
    }
    fn IsShown(&self) -> Result<BOOL> {
        com(|| Ok((self.data.live.get() && self.data.shown.get()).into()))
    }
}
impl ITfCandidateListUIElement_Impl for CandidateElement_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        com(|| {
            Ok(TF_CLUIE_DOCUMENTMGR
                | TF_CLUIE_COUNT
                | TF_CLUIE_SELECTION
                | TF_CLUIE_STRING
                | TF_CLUIE_PAGEINDEX
                | TF_CLUIE_CURRENTPAGE)
        })
    }
    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        com(|| unsafe { self.data.check()?.native.GetDocumentMgr() })
    }
    fn GetCount(&self) -> Result<u32> {
        com(|| Ok(self.data.texts.len() as u32))
    }
    fn GetSelection(&self) -> Result<u32> {
        com(|| Ok(self.data.selected.get()))
    }
    fn GetString(&self, index: u32) -> Result<BSTR> {
        com(|| {
            self.data
                .texts
                .get(index as usize)
                .map(|text| BSTR::from(text.as_str()))
                .ok_or_else(|| E_INVALIDARG.into())
        })
    }
    fn GetPageIndex(&self, indices: *mut u32, size: u32, count: *mut u32) -> Result<()> {
        com(|| {
            if count.is_null() || (size != 0 && indices.is_null()) {
                return Err(E_POINTER.into());
            }
            let pages = self.data.texts.len().div_ceil(PAGE_SIZE) as u32;
            unsafe {
                count.write(pages);
                for index in 0..size.min(pages) {
                    indices.add(index as usize).write(index * PAGE_SIZE as u32);
                }
            }
            // Preserve the TSF S_FALSE success status for a short output buffer.
            if size < pages {
                Err(Error::from_hresult(S_FALSE))
            } else {
                Ok(())
            }
        })
    }
    fn SetPageIndex(&self, indices: *const u32, count: u32) -> Result<()> {
        com(|| {
            if indices.is_null() {
                return Err(E_POINTER.into());
            }
            let pages = self.data.texts.len().div_ceil(PAGE_SIZE) as u32;
            if count != pages {
                return Err(E_INVALIDARG.into());
            }
            for index in 0..count {
                if unsafe { indices.add(index as usize).read() } != index * PAGE_SIZE as u32 {
                    return Err(E_INVALIDARG.into());
                }
            }
            // Pagination is a core rule; the adapter accepts the canonical page map only.
            Ok(())
        })
    }
    fn GetCurrentPage(&self) -> Result<u32> {
        com(|| Ok(self.data.page.get()))
    }
}
impl ITfCandidateListUIElementBehavior_Impl for CandidateElement_Impl {
    fn SetSelection(&self, index: u32) -> Result<()> {
        com(|| {
            self.data.check()?;
            if index as usize >= self.data.texts.len() {
                return Err(E_INVALIDARG.into());
            }
            self.data.selected.set(index);
            self.data.page.set(index / PAGE_SIZE as u32);
            Ok(())
        })
    }
    fn Finalize(&self) -> Result<()> {
        com(|| self.data.select(self.data.selected.get()))
    }
    fn Abort(&self) -> Result<()> {
        com(|| self.data.check()?.abort_selection())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_com_strings_pages_and_stale_behavior() {
        let data = Rc::new(Data {
            context: Weak::new(),
            epoch: 1,
            generation: 1,
            texts: (0..10).map(|_| "𠀀e\u{301}".into()).collect(),
            selected: Cell::new(9),
            page: Cell::new(1),
            shown: Cell::new(false),
            live: Cell::new(false),
            hwnd: Cell::new(HWND::default()),
        });
        let element: ITfCandidateListUIElementBehavior = CandidateElement {
            data,
            _module: ModuleGuard::new(),
        }
        .into();
        unsafe {
            assert!(element.cast::<ITfUIElement>().is_ok());
            assert_eq!(element.GetCount().unwrap(), 10);
            assert_eq!(element.GetSelection().unwrap(), 9);
            assert_eq!(element.GetString(0).unwrap().to_string(), "𠀀e\u{301}");
            assert_eq!(element.GetString(10).unwrap_err().code(), E_INVALIDARG);
            let mut pages = [0u32; 2];
            let mut count = 0;
            element.GetPageIndex(&mut pages, &mut count).unwrap();
            assert_eq!(pages, [0, 9]);
            assert_eq!(count, 2);
            assert_eq!(element.GetCurrentPage().unwrap(), 1);
            element.SetPageIndex(&pages).unwrap();
            assert_eq!(element.SetSelection(0).unwrap_err().code(), E_INVALIDARG);
            assert_eq!(element.Finalize().unwrap_err().code(), E_INVALIDARG);
            assert_eq!(element.Abort().unwrap_err().code(), E_INVALIDARG);
        }
    }
}
