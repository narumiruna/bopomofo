use crate::keys::ModifierState;
use ime_core::KeyEvent;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub(super) fn translate(wparam: WPARAM, lparam: LPARAM, release: bool) -> Option<KeyEvent> {
    let down = |key: VIRTUAL_KEY| unsafe { GetKeyState(i32::from(key.0)) < 0 };
    crate::keys::from_native(
        wparam.0 as u16,
        lparam.0 as u32,
        release,
        ModifierState {
            left_shift: down(VK_LSHIFT),
            right_shift: down(VK_RSHIFT),
            left_ctrl: down(VK_LCONTROL),
            right_ctrl: down(VK_RCONTROL),
            left_alt: down(VK_LMENU),
            right_alt: down(VK_RMENU),
            left_meta: down(VK_LWIN),
            right_meta: down(VK_RWIN),
        },
    )
}
