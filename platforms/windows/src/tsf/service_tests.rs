use super::*;
use ime_core::{InputMode, KeyEvent};
use std::cell::Cell;
use windows::Win32::System::Com::{IAgileObject, IEnumGUID};
use windows::Win32::System::Variant::VARIANT;

macro_rules! unsupported {
    ($(fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {
        $(fn $name(&self $(, $arg: $ty)*) -> Result<$ret> {
            Err(E_NOTIMPL.into())
        })*
    };
}

#[implement(ITfCompartment, Agile = false)]
struct Compartment {
    value: Option<i32>,
}

impl ITfCompartment_Impl for Compartment_Impl {
    fn GetValue(&self) -> Result<VARIANT> {
        let mut value = VARIANT::default();
        if let Some(number) = self.value {
            // VT_I4 owns no pointers. VT_EMPTY is the default value.
            unsafe {
                let inner = &mut *value.Anonymous.Anonymous;
                inner.vt = VT_I4;
                inner.Anonymous.lVal = number;
            }
        }
        Ok(value)
    }

    unsupported! {
        fn SetValue(&self, _client: u32, _value: *const VARIANT) -> ();
    }
}

#[implement(ITfContext, ITfCompartmentMgr, Agile = false)]
struct NativeContext {
    status: Cell<TS_STATUS>,
    compartment: ITfCompartment,
    fail_next_edit: Cell<bool>,
    on_edit: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl NativeContext {
    fn new(value: Option<i32>) -> ComObject<Self> {
        ComObject::new(Self {
            status: Cell::new(TS_STATUS::default()),
            compartment: Compartment { value }.into(),
            fail_next_edit: Cell::new(false),
            on_edit: RefCell::new(None),
        })
    }
}

impl ITfContext_Impl for NativeContext_Impl {
    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(self.status.get())
    }

    fn RequestEditSession(
        &self,
        _client: u32,
        _edit: Ref<ITfEditSession>,
        _flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    ) -> Result<HRESULT> {
        let callback = self.on_edit.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
        if self.fail_next_edit.replace(false) {
            return Err(E_FAIL.into());
        }
        // Do not retain the edit session or call a text store.
        Ok(TF_S_ASYNC)
    }

    unsupported! {
        fn InWriteSession(&self, _client: u32) -> BOOL;
        fn GetSelection(&self, _cookie: u32, _index: u32, _count: u32, _selection: *mut TF_SELECTION, _fetched: *mut u32) -> ();
        fn SetSelection(&self, _cookie: u32, _count: u32, _selection: *const TF_SELECTION) -> ();
        fn GetStart(&self, _cookie: u32) -> ITfRange;
        fn GetEnd(&self, _cookie: u32) -> ITfRange;
        fn GetActiveView(&self) -> ITfContextView;
        fn EnumViews(&self) -> IEnumTfContextViews;
        fn GetProperty(&self, _guid: *const GUID) -> ITfProperty;
        fn GetAppProperty(&self, _guid: *const GUID) -> ITfReadOnlyProperty;
        fn TrackProperties(&self, _properties: *const *const GUID, _count: u32, _app: *const *const GUID, _app_count: u32) -> ITfReadOnlyProperty;
        fn EnumProperties(&self) -> IEnumTfProperties;
        fn GetDocumentMgr(&self) -> ITfDocumentMgr;
        fn CreateRangeBackup(&self, _cookie: u32, _range: Ref<ITfRange>) -> ITfRangeBackup;
    }
}

impl ITfCompartmentMgr_Impl for NativeContext_Impl {
    fn GetCompartment(&self, guid: *const GUID) -> Result<ITfCompartment> {
        if guid.is_null() {
            return Err(E_POINTER.into());
        }
        let guid = unsafe { *guid };
        if guid != GUID_COMPARTMENT_KEYBOARD_DISABLED && guid != GUID_COMPARTMENT_EMPTYCONTEXT {
            return Err(E_INVALIDARG.into());
        }
        Ok(self.compartment.clone())
    }

    unsupported! {
        fn ClearCompartment(&self, _client: u32, _guid: *const GUID) -> ();
        fn EnumCompartments(&self) -> IEnumGUID;
    }
}

#[implement(ITfDocumentMgr, Agile = false)]
struct Document {
    stack: RefCell<Vec<ITfContext>>,
}

impl ITfDocumentMgr_Impl for Document_Impl {
    fn GetTop(&self) -> Result<ITfContext> {
        self.stack
            .borrow()
            .last()
            .cloned()
            .ok_or_else(|| E_FAIL.into())
    }

    fn Push(&self, context: Ref<ITfContext>) -> Result<()> {
        self.stack.borrow_mut().push(
            context
                .as_ref()
                .ok_or_else(|| Error::from(E_POINTER))?
                .clone(),
        );
        Ok(())
    }

    fn Pop(&self, _flags: u32) -> Result<()> {
        self.stack.borrow_mut().pop();
        Ok(())
    }

    unsupported! {
        fn CreateContext(&self, _client: u32, _flags: u32, _owner: Ref<IUnknown>, _context: OutRef<ITfContext>, _cookie: *mut u32) -> ();
        fn GetBase(&self) -> ITfContext;
        fn EnumContexts(&self) -> IEnumTfContexts;
    }
}

#[implement(
    ITfThreadMgr,
    ITfKeystrokeMgr,
    ITfSource,
    ITfUIElementMgr,
    Agile = false
)]
struct Manager {
    focus: RefCell<Option<ITfDocumentMgr>>,
}

impl ITfThreadMgr_Impl for Manager_Impl {
    fn GetFocus(&self) -> Result<ITfDocumentMgr> {
        self.focus.borrow().clone().ok_or_else(|| E_FAIL.into())
    }

    unsupported! {
        fn Activate(&self) -> u32;
        fn Deactivate(&self) -> ();
        fn CreateDocumentMgr(&self) -> ITfDocumentMgr;
        fn EnumDocumentMgrs(&self) -> IEnumTfDocumentMgrs;
        fn SetFocus(&self, _document: Ref<ITfDocumentMgr>) -> ();
        fn AssociateFocus(&self, _window: HWND, _document: Ref<ITfDocumentMgr>) -> ITfDocumentMgr;
        fn IsThreadFocus(&self) -> BOOL;
        fn GetFunctionProvider(&self, _class: *const GUID) -> ITfFunctionProvider;
        fn EnumFunctionProviders(&self) -> IEnumTfFunctionProviders;
        fn GetGlobalCompartment(&self) -> ITfCompartmentMgr;
    }
}

impl ITfKeystrokeMgr_Impl for Manager_Impl {
    fn UnadviseKeyEventSink(&self, _client: u32) -> Result<()> {
        Ok(())
    }

    unsupported! {
        fn AdviseKeyEventSink(&self, _client: u32, _sink: Ref<ITfKeyEventSink>, _foreground: BOOL) -> ();
        fn GetForeground(&self) -> GUID;
        fn TestKeyDown(&self, _w: WPARAM, _l: LPARAM) -> BOOL;
        fn TestKeyUp(&self, _w: WPARAM, _l: LPARAM) -> BOOL;
        fn KeyDown(&self, _w: WPARAM, _l: LPARAM) -> BOOL;
        fn KeyUp(&self, _w: WPARAM, _l: LPARAM) -> BOOL;
        fn GetPreservedKey(&self, _context: Ref<ITfContext>, _key: *const TF_PRESERVEDKEY) -> GUID;
        fn IsPreservedKey(&self, _guid: *const GUID, _key: *const TF_PRESERVEDKEY) -> BOOL;
        fn PreserveKey(&self, _client: u32, _guid: *const GUID, _key: *const TF_PRESERVEDKEY, _description: &PCWSTR, _count: u32) -> ();
        fn UnpreserveKey(&self, _guid: *const GUID, _key: *const TF_PRESERVEDKEY) -> ();
        fn SetPreservedKeyDescription(&self, _guid: *const GUID, _description: &PCWSTR, _count: u32) -> ();
        fn GetPreservedKeyDescription(&self, _guid: *const GUID) -> BSTR;
        fn SimulatePreservedKey(&self, _context: Ref<ITfContext>, _guid: *const GUID) -> BOOL;
    }
}

impl ITfSource_Impl for Manager_Impl {
    fn UnadviseSink(&self, _cookie: u32) -> Result<()> {
        Ok(())
    }

    unsupported! {
        fn AdviseSink(&self, _iid: *const GUID, _sink: Ref<IUnknown>) -> u32;
    }
}

impl ITfUIElementMgr_Impl for Manager_Impl {
    unsupported! {
        fn BeginUIElement(&self, _element: Ref<ITfUIElement>, _show: *mut BOOL, _id: *mut u32) -> ();
        fn UpdateUIElement(&self, _id: u32) -> ();
        fn EndUIElement(&self, _id: u32) -> ();
        fn GetUIElement(&self, _id: u32) -> ITfUIElement;
        fn EnumUIElements(&self) -> IEnumTfUIElements;
    }
}

fn lexicon() -> Arc<Lexicon> {
    // Self-authored data. No files or installed resources are used.
    Arc::new(Lexicon::parse("# bopomofo-lexicon-v1\nㄅㄚ\t八\t20\nㄅㄚ\t巴\t10\n").unwrap())
}

fn manager() -> ComObject<Manager> {
    ComObject::new(Manager {
        focus: RefCell::new(None),
    })
}

fn context(native: &ComObject<NativeContext>) -> Rc<Context> {
    Rc::new(Context {
        native: native.to_interface(),
        client: 1,
        ui: manager().to_interface(),
        inner: RefCell::new(ContextInner {
            session: Session::new(lexicon()),
            pending: VecDeque::new(),
            composition: None,
            presenter: None,
            focused: true,
            valid: true,
            editing: false,
            scheduled: false,
        }),
        _module: ModuleGuard::new(),
    })
}

fn physical(context: &Rc<Context>, vk: u16, release: bool) {
    let event = crate::keys::normalize(
        vk,
        0,
        release,
        KeyEvent {
            shift: vk == 0x10 && !release,
            ..KeyEvent::default()
        },
    );
    context.dispatch(Event::Key(event)).unwrap();
}

fn tap(context: &Rc<Context>, vk: u16) {
    physical(context, vk, false);
    physical(context, vk, true);
}

fn select_candidates(context: &Rc<Context>) {
    for vk in [0x31, 0x38, 0x20, 0x28] {
        tap(context, vk);
    }
    let state = context.inner.borrow();
    assert!(state.session.snapshot().selecting);
    assert_eq!(state.session.snapshot().mode, InputMode::Chinese);
    assert_no_commit(context);
}

fn assert_no_commit(context: &Context) {
    assert!(
        context
            .inner
            .borrow()
            .pending
            .iter()
            .all(|(_, output)| output.commit.is_none())
    );
}

#[test]
fn allowed_uses_dynamic_status_not_overlapping_static_flags() {
    for value in [None, Some(0)] {
        let native = NativeContext::new(value);
        let interface: ITfContext = native.to_interface();
        assert!(interface.cast::<IAgileObject>().is_err());
        for flags in [
            TF_SD_READONLY,
            TF_SD_LOADING,
            TF_SD_READONLY | TF_SD_LOADING,
        ] {
            native.status.set(TS_STATUS {
                dwDynamicFlags: flags,
                dwStaticFlags: 0,
            });
            assert!(!allowed(&interface));
        }
        for flags in [
            0,
            TF_SS_DISJOINTSEL,
            TF_SS_REGIONS,
            TF_SS_DISJOINTSEL | TF_SS_REGIONS,
        ] {
            native.status.set(TS_STATUS {
                dwDynamicFlags: 0,
                dwStaticFlags: flags,
            });
            assert!(allowed(&interface));
        }
    }
    let disabled = NativeContext::new(Some(1));
    assert!(!allowed(&disabled.to_interface()));
}

#[test]
fn abort_balances_escape_and_allows_shift_tap() {
    let native = NativeContext::new(None);
    let context = context(&native);
    select_candidates(&context);
    context.abort_selection().unwrap();
    let snapshot = context.inner.borrow().session.snapshot();
    assert!(!snapshot.selecting);
    assert!(!snapshot.preedit.is_empty());
    assert_no_commit(&context);
    tap(&context, 0x10);
    assert_eq!(
        context.inner.borrow().session.snapshot().mode,
        InputMode::Latin
    );
    assert_no_commit(&context);
}

#[test]
fn abort_preserves_a_held_physical_key() {
    let native = NativeContext::new(Some(0));
    let context = context(&native);
    select_candidates(&context);
    physical(&context, 0x70, false); // F1 remains held across Abort.
    context.abort_selection().unwrap();
    assert!(!context.inner.borrow().session.snapshot().selecting);
    tap(&context, 0x10);
    assert_eq!(
        context.inner.borrow().session.snapshot().mode,
        InputMode::Chinese
    );
    physical(&context, 0x70, true);
    tap(&context, 0x10);
    assert_eq!(
        context.inner.borrow().session.snapshot().mode,
        InputMode::Latin
    );
    assert_no_commit(&context);
}

#[test]
fn abort_releases_escape_when_press_fails() {
    let native = NativeContext::new(None);
    let context = context(&native);
    select_candidates(&context);
    context.inner.borrow_mut().scheduled = false;
    native.fail_next_edit.set(true);
    assert_eq!(context.abort_selection().unwrap_err().code(), E_FAIL);
    {
        let state = context.inner.borrow();
        assert!(!state.session.snapshot().selecting);
        // Error recovery queues Reset. The matching release must follow Reset.
        assert_eq!(state.pending.len(), 2);
        assert_eq!(state.pending.back().unwrap().0.sequence, 2);
        assert!(!state.pending.back().unwrap().1.consumed);
    }
    tap(&context, 0x10);
    assert_eq!(
        context.inner.borrow().session.snapshot().mode,
        InputMode::Latin
    );
    assert_no_commit(&context);
}

fn activated(manager: &ComObject<Manager>) -> ComObject<Service> {
    let service = ComObject::new(Service::new());
    *service.activation.borrow_mut() = Some(Activation {
        manager: manager.to_interface(),
        keys: manager.to_interface(),
        source: manager.to_interface(),
        cookie: 1,
        client: 1,
        ui: manager.to_interface(),
        lexicon: lexicon(),
        contexts: HashMap::new(),
        focus: None,
    });
    service
}

fn current_context(service: &ComObject<Service>, native: &ITfContext) -> Rc<Context> {
    service.activation.borrow().as_ref().unwrap().contexts[&identity(native).unwrap()].clone()
}

fn assert_focus(service: &ComObject<Service>, expected: Option<&ITfContext>) {
    let state = service.activation.borrow();
    let state = state.as_ref().unwrap();
    assert_eq!(
        state.focus,
        expected.map(|context| identity(context).unwrap())
    );
    for (id, context) in &state.contexts {
        assert_eq!(context.inner.borrow().focused, state.focus == Some(*id));
    }
}

fn assert_key_delivery(service: &ComObject<Service>, native: &ITfContext) {
    let keys: ITfKeyEventSink = service.to_interface();
    let context = current_context(service, native);
    let before = context.inner.borrow().pending.len();
    unsafe {
        assert!(
            keys.OnTestKeyDown(native, WPARAM(0x31), LPARAM(0))
                .unwrap()
                .as_bool()
        );
        assert_eq!(context.inner.borrow().pending.len(), before);
        let _ = keys.OnKeyDown(native, WPARAM(0x31), LPARAM(0)).unwrap();
        assert_eq!(context.inner.borrow().pending.len(), before + 1);
        let _ = keys
            .OnKeyUp(native, WPARAM(0x31), LPARAM(1isize << 31))
            .unwrap();
        assert_eq!(context.inner.borrow().pending.len(), before + 2);
    }
    assert_no_commit(&context);
}

#[test]
fn focus_activates_new_context_when_previous_cleanup_fails() {
    for push in [false, true] {
        let manager = manager();
        let a = NativeContext::new(None);
        let b = NativeContext::new(None);
        let a_native: ITfContext = a.to_interface();
        let b_native: ITfContext = b.to_interface();
        let previous: ITfDocumentMgr = Document {
            stack: RefCell::new(vec![a_native.clone()]),
        }
        .into();
        *manager.focus.borrow_mut() = Some(previous.clone());
        let service = activated(&manager);
        let sink: ITfThreadMgrEventSink = service.to_interface();
        unsafe {
            sink.OnSetFocus(&previous, None).unwrap();
            assert_key_delivery(&service, &a_native);
            let old = current_context(&service, &a_native);
            old.inner.borrow_mut().scheduled = false;
            a.fail_next_edit.set(true);
            let next = if push {
                previous.Push(&b_native).unwrap();
                previous.clone()
            } else {
                let next: ITfDocumentMgr = Document {
                    stack: RefCell::new(vec![b_native.clone()]),
                }
                .into();
                *manager.focus.borrow_mut() = Some(next.clone());
                next
            };
            let result = if push {
                sink.OnPushContext(&b_native)
            } else {
                sink.OnSetFocus(&next, &previous)
            };
            assert_eq!(result.unwrap_err().code(), E_FAIL);
            assert_focus(&service, Some(&b_native));
            let active = current_context(&service, &b_native);
            assert_eq!(active.inner.borrow().pending.len(), 1);
            assert_key_delivery(&service, &b_native);
            let pending = active.inner.borrow().pending.len();
            sink.OnSetFocus(&next, &previous).unwrap();
            assert_eq!(active.inner.borrow().pending.len(), pending);
            assert_key_delivery(&service, &b_native);
            assert_no_commit(&old);
        }
    }
}

#[test]
fn cleanup_reentry_does_not_reactivate_a_superseded_context() {
    let manager = Rc::new(manager());
    let a = NativeContext::new(None);
    let a_native: ITfContext = a.to_interface();
    let b_native: ITfContext = NativeContext::new(None).to_interface();
    let c_native: ITfContext = NativeContext::new(None).to_interface();
    let document = |native: &ITfContext| -> ITfDocumentMgr {
        Document {
            stack: RefCell::new(vec![native.clone()]),
        }
        .into()
    };
    let first = document(&a_native);
    let second = document(&b_native);
    let third = document(&c_native);
    *manager.focus.borrow_mut() = Some(first.clone());
    let service = Rc::new(activated(&manager));
    let sink: ITfThreadMgrEventSink = service.to_interface();
    unsafe {
        sink.OnSetFocus(&first, None).unwrap();
        current_context(&service, &a_native)
            .inner
            .borrow_mut()
            .scheduled = false;
        let weak_service = Rc::downgrade(&service);
        let weak_manager = Rc::downgrade(&manager);
        *a.on_edit.borrow_mut() = Some(Box::new(move || {
            let service = weak_service.upgrade().unwrap();
            let manager = weak_manager.upgrade().unwrap();
            *manager.focus.borrow_mut() = Some(third.clone());
            let sink: ITfThreadMgrEventSink = service.to_interface();
            sink.OnSetFocus(&third, None).unwrap();
        }));
        *manager.focus.borrow_mut() = Some(second.clone());
        sink.OnSetFocus(&second, &first).unwrap();
        assert_focus(&service, Some(&c_native));
        assert_key_delivery(&service, &c_native);
        let keys: ITfKeyEventSink = service.to_interface();
        assert!(
            !keys
                .OnTestKeyDown(&b_native, WPARAM(0x31), LPARAM(0))
                .unwrap()
                .as_bool()
        );
        assert_no_commit(&current_context(&service, &a_native));
        assert_no_commit(&current_context(&service, &b_native));
    }
}

#[test]
fn pop_restores_focused_document_top_even_when_cleanup_fails() {
    for fail_cleanup in [false, true] {
        let manager = manager();
        let document: ITfDocumentMgr = Document {
            stack: RefCell::new(Vec::new()),
        }
        .into();
        *manager.focus.borrow_mut() = Some(document.clone());
        let service = activated(&manager);
        let sink: ITfThreadMgrEventSink = service.to_interface();
        let a = NativeContext::new(None);
        let b = NativeContext::new(None);
        let a_native: ITfContext = a.to_interface();
        let b_native: ITfContext = b.to_interface();
        unsafe {
            document.Push(&a_native).unwrap();
            sink.OnSetFocus(&document, None).unwrap();
            assert_focus(&service, Some(&a_native));
            assert_key_delivery(&service, &a_native);
            document.Push(&b_native).unwrap();
            sink.OnPushContext(&b_native).unwrap();
            assert_focus(&service, Some(&b_native));
            assert_key_delivery(&service, &b_native);
            let popped = current_context(&service, &b_native);
            if fail_cleanup {
                popped.inner.borrow_mut().scheduled = false;
                b.fail_next_edit.set(true);
            }
            document.Pop(0).unwrap();
            let result = sink.OnPopContext(&b_native);
            if fail_cleanup {
                assert_eq!(result.unwrap_err().code(), E_FAIL);
            } else {
                result.unwrap();
            }
            assert!(!popped.inner.borrow().valid);
            assert_focus(&service, Some(&a_native));
            assert_key_delivery(&service, &a_native);
            assert!(
                !service
                    .activation
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .contexts
                    .contains_key(&identity(&b_native).unwrap())
            );
            // No document focus callback occurs during either stack change.
            document.Pop(0).unwrap();
            sink.OnPopContext(&a_native).unwrap();
            assert_focus(&service, None);
            let keys: ITfKeyEventSink = service.to_interface();
            assert!(
                !keys
                    .OnTestKeyDown(&a_native, WPARAM(0x31), LPARAM(0))
                    .unwrap()
                    .as_bool()
            );
        }
    }
}

#[test]
fn pop_in_nonfocused_document_keeps_focused_context() {
    let manager = manager();
    let focused: ITfDocumentMgr = Document {
        stack: RefCell::new(Vec::new()),
    }
    .into();
    let other: ITfDocumentMgr = Document {
        stack: RefCell::new(Vec::new()),
    }
    .into();
    *manager.focus.borrow_mut() = Some(other.clone());
    let service = activated(&manager);
    let sink: ITfThreadMgrEventSink = service.to_interface();
    let a: ITfContext = NativeContext::new(None).to_interface();
    let b: ITfContext = NativeContext::new(None).to_interface();
    unsafe {
        other.Push(&b).unwrap();
        sink.OnSetFocus(&other, None).unwrap();
        let previous = current_context(&service, &b);
        focused.Push(&a).unwrap();
        *manager.focus.borrow_mut() = Some(focused.clone());
        sink.OnSetFocus(&focused, &other).unwrap();
        assert_focus(&service, Some(&a));
        sink.OnPushContext(&b).unwrap();
        assert_focus(&service, Some(&a));
        other.Pop(0).unwrap();
        sink.OnPopContext(&b).unwrap();
        assert!(!previous.inner.borrow().valid);
        assert_focus(&service, Some(&a));
        assert_key_delivery(&service, &a);
    }
}
