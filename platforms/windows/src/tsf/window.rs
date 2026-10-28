//! Native, non-activating LISTBOX fallback for hosts without UI-less rendering.
use super::{ModuleGuard, candidate::Data};
use crate::session::boundary;
use ime_core::PAGE_SIZE;
use std::rc::Rc;
use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

const CLASS: PCWSTR = w!("Bopomofo.TSF.Candidates.v1");
static CLASSES: Mutex<usize> = Mutex::new(0);

// windows-rs aliases these APIs to LONG (i32) on x86 and LONG_PTR (isize)
// on 64-bit targets. Keep the pointer-width conversion at this boundary.
unsafe fn get_window_long(hwnd: HWND, index: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "32")]
    {
        (unsafe { GetWindowLongPtrW(hwnd, index) }) as isize
    }
    #[cfg(target_pointer_width = "64")]
    {
        unsafe { GetWindowLongPtrW(hwnd, index) }
    }
}

unsafe fn set_window_long(hwnd: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> isize {
    #[cfg(target_pointer_width = "32")]
    {
        (unsafe { SetWindowLongPtrW(hwnd, index, value as i32) }) as isize
    }
    #[cfg(target_pointer_width = "64")]
    {
        unsafe { SetWindowLongPtrW(hwnd, index, value) }
    }
}

struct WindowClass {
    instance: HINSTANCE,
}
impl WindowClass {
    fn acquire() -> Result<Self> {
        let mut count = CLASSES.lock().unwrap_or_else(|error| error.into_inner());
        let mut module = HMODULE::default();
        unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(procedure as *const () as *const u16),
                &mut module,
            )?;
            let instance = HINSTANCE(module.0);
            if *count == 0 {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(procedure),
                    hInstance: instance,
                    lpszClassName: CLASS,
                    hCursor: LoadCursorW(None, IDC_ARROW)?,
                    hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 {
                    return Err(Error::from_thread());
                }
            }
            *count += 1;
            Ok(Self { instance })
        }
    }
}
impl Drop for WindowClass {
    fn drop(&mut self) {
        let mut count = CLASSES.lock().unwrap_or_else(|error| error.into_inner());
        *count -= 1;
        if *count == 0 {
            unsafe {
                let _ = UnregisterClassW(CLASS, Some(self.instance));
            }
        }
    }
}

pub(super) struct CandidateWindow {
    hwnd: HWND,
    _class: WindowClass,
    _module: ModuleGuard,
}
impl CandidateWindow {
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn new(data: Rc<Data>, caret: RECT) -> Result<Self> {
        let class = WindowClass::acquire()?;
        let module = ModuleGuard::new();
        let start = data.page.get() as usize * PAGE_SIZE;
        let rows = data.texts.len().saturating_sub(start).min(PAGE_SIZE);
        if rows == 0 {
            return Err(E_INVALIDARG.into());
        }
        let dpi = data
            .context
            .upgrade()
            .and_then(|context| unsafe {
                context
                    .native
                    .GetActiveView()
                    .and_then(|view| view.GetWnd())
                    .ok()
            })
            .map(|hwnd| unsafe { GetDpiForWindow(hwnd) })
            .filter(|dpi| *dpi != 0)
            .unwrap_or(96) as i32;
        let row_height = 26 * dpi / 96;
        let width = 320 * dpi / 96;
        let height = rows as i32 * row_height + 4;
        let mut x = caret.left;
        let mut y = caret.bottom;
        unsafe {
            let monitor = MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                x = x.min(info.rcWork.right - width).max(info.rcWork.left);
                if y + height > info.rcWork.bottom {
                    y = caret.top - height;
                }
                y = y.max(info.rcWork.top);
            }
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                CLASS,
                w!("bopomofo candidates"),
                WS_POPUP | WS_BORDER,
                x,
                y,
                width,
                height,
                None,
                None,
                Some(class.instance),
                None,
            )?;
            // Establish the RAII owner immediately so child-creation errors destroy the parent.
            let window = Self {
                hwnd,
                _class: class,
                _module: module,
            };
            let list = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("LISTBOX"),
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WINDOW_STYLE(LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32),
                0,
                0,
                width - 2,
                height - 2,
                Some(hwnd),
                Some(HMENU(std::ptr::without_provenance_mut(1))),
                Some(window._class.instance),
                None,
            )?;
            let font = GetStockObject(DEFAULT_GUI_FONT);
            SendMessageW(
                list,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(0)),
            );
            SendMessageW(
                list,
                LB_SETITEMHEIGHT,
                Some(WPARAM(0)),
                Some(LPARAM(row_height as isize)),
            );
            for (index, text) in data.texts.iter().skip(start).take(PAGE_SIZE).enumerate() {
                let display: Vec<u16> = format!("{}. {}", index + 1, text)
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
                let result = SendMessageW(
                    list,
                    LB_ADDSTRING,
                    None,
                    Some(LPARAM(display.as_ptr() as isize)),
                );
                if result.0 < 0 {
                    return Err(E_OUTOFMEMORY.into());
                }
            }
            let selected = data.selected.get() as usize;
            if selected >= start && selected < start + rows {
                SendMessageW(list, LB_SETCURSEL, Some(WPARAM(selected - start)), None);
            }
            let state = Box::new(WindowData { data, list, start });
            set_window_long(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            subclass_list(list, hwnd)?;
            Ok(window)
        }
    }
}
impl Drop for CandidateWindow {
    fn drop(&mut self) {
        // DestroyWindow sends WM_NCDESTROY, which releases WindowData.
        let _ = boundary(|| unsafe { DestroyWindow(self.hwnd) });
    }
}

struct WindowData {
    data: Rc<Data>,
    list: HWND,
    start: usize,
}

struct ListProcedure {
    parent: HWND,
    previous: WNDPROC,
}

unsafe fn subclass_list(list: HWND, parent: HWND) -> Result<()> {
    let previous = unsafe { get_window_long(list, GWLP_WNDPROC) };
    if previous == 0 {
        return Err(E_FAIL.into());
    }
    let state = Box::new(ListProcedure {
        parent,
        previous: unsafe { std::mem::transmute::<isize, WNDPROC>(previous) },
    });
    let pointer = Box::into_raw(state);
    unsafe {
        set_window_long(list, GWLP_USERDATA, pointer as isize);
        if set_window_long(list, GWLP_WNDPROC, list_procedure as *const () as isize) == 0 {
            set_window_long(list, GWLP_USERDATA, 0);
            drop(Box::from_raw(pointer));
            return Err(E_FAIL.into());
        }
    }
    Ok(())
}

unsafe extern "system" fn list_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    boundary(|| {
        let pointer = unsafe { get_window_long(hwnd, GWLP_USERDATA) as *mut ListProcedure };
        if pointer.is_null() {
            return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
        }
        let (parent, previous) = unsafe { ((*pointer).parent, (*pointer).previous) };
        if message == WM_NCDESTROY {
            unsafe {
                set_window_long(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(pointer));
            }
        } else if message == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        } else if message == WM_LBUTTONDOWN {
            // Do not call the stock control's mouse handler: it calls SetFocus.
            // Hit testing and highlight still use native LISTBOX messages.
            let parent_data = unsafe { get_window_long(parent, GWLP_USERDATA) as *mut WindowData };
            if !parent_data.is_null() {
                let (data, start) = unsafe { ((*parent_data).data.clone(), (*parent_data).start) };
                let hit =
                    unsafe { SendMessageW(hwnd, LB_ITEMFROMPOINT, None, Some(lparam)) }.0 as u32;
                if hit >> 16 == 0 {
                    let row = (hit & 0xffff) as usize;
                    unsafe {
                        SendMessageW(hwnd, LB_SETCURSEL, Some(WPARAM(row)), None);
                    }
                    let _ = data.select((start + row) as u32);
                }
            }
            return LRESULT(0);
        } else if matches!(
            message,
            WM_LBUTTONUP | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONUP
        ) {
            return LRESULT(0);
        }
        unsafe { CallWindowProcW(previous, hwnd, message, wparam, lparam) }
    })
    .unwrap_or(LRESULT(0))
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    boundary(|| {
        if message == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        let pointer = unsafe { get_window_long(hwnd, GWLP_USERDATA) as *mut WindowData };
        if message == WM_NCDESTROY {
            if !pointer.is_null() {
                unsafe {
                    set_window_long(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(pointer));
                }
            }
        } else if message == WM_COMMAND
            && ((wparam.0 >> 16) as u32) == LBN_SELCHANGE
            && !pointer.is_null()
        {
            // Clone before dispatch: a synchronous edit can destroy this same window.
            let (data, list, start) =
                unsafe { ((*pointer).data.clone(), (*pointer).list, (*pointer).start) };
            if lparam.0 != list.0 as isize {
                return LRESULT(0);
            }
            let index = unsafe { SendMessageW(list, LB_GETCURSEL, None, None) }.0;
            if index >= 0 {
                // Context::dispatch contains engine failures. A stale UI error
                // must not reset a newer epoch or its replacement composition.
                let _ = data.select((start + index as usize) as u32);
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    })
    .unwrap_or(LRESULT(0))
}
