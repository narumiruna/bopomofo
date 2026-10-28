//! Physical US Dachen keys and post-event Windows modifier normalization.
//! Numeric values are SDK virtual-key codes and Set 1 scan codes.
use ime_core::KeyEvent;

pub(crate) fn normalize(vk: u16, flags: u32, release: bool, mut event: KeyEvent) -> KeyEvent {
    event.key = if let Some(key) = physical_key(vk, flags) {
        key.into()
    } else {
        match vk {
            0x41..=0x5a => char::from_u32(u32::from(vk) + 32).unwrap().to_string(),
            0x30..=0x39 => char::from_u32(u32::from(vk)).unwrap().to_string(),
            0x20 => "space".into(),
            0x0d => "enter".into(),
            0x1b => "escape".into(),
            0x08 => "backspace".into(),
            0x2e => "delete".into(),
            0x25 => "left".into(),
            0x27 => "right".into(),
            0x26 => "up".into(),
            0x28 => "down".into(),
            0x24 => "home".into(),
            0x23 => "end".into(),
            0x21 => "page_up".into(),
            0x22 => "page_down".into(),
            0x14 => "caps_lock".into(),
            0x10 | 0xa0 | 0xa1 => "shift".into(),
            0x11 | 0xa2 | 0xa3 => "control".into(),
            0x12 | 0xa4 | 0xa5 => "alt".into(),
            0x5b | 0x5c => "meta".into(),
            0x09 => "tab".into(),
            0xba => ";".into(),
            0xbf => "/".into(),
            0xbc => ",".into(),
            0xbe => ".".into(),
            0xbd => "-".into(),
            // Keep distinct identities so unrelated held keys cannot clear each other.
            _ => format!("vk_{vk:04x}"),
        }
    };
    event.repeat = !release && flags & (1 << 30) != 0;
    event.release = release;
    event
}

// Only printable native keys use the physical US map. Modifier/action remaps
// must retain their native meaning, and code-less frontends use virtual keys.
fn physical_key(vk: u16, flags: u32) -> Option<&'static str> {
    let printable = matches!(vk, 0x30..=0x39 | 0x41..=0x5a | 0xba..=0xc0 | 0xdb..=0xdf | 0xe2);
    if !printable || flags & (1 << 24) != 0 {
        return None;
    }
    Some(match (flags >> 16) & 0xff {
        0x02 => "1",
        0x03 => "2",
        0x04 => "3",
        0x05 => "4",
        0x06 => "5",
        0x07 => "6",
        0x08 => "7",
        0x09 => "8",
        0x0a => "9",
        0x0b => "0",
        0x0c => "-",
        0x0d => "=",
        0x10 => "q",
        0x11 => "w",
        0x12 => "e",
        0x13 => "r",
        0x14 => "t",
        0x15 => "y",
        0x16 => "u",
        0x17 => "i",
        0x18 => "o",
        0x19 => "p",
        0x1a => "[",
        0x1b => "]",
        0x1e => "a",
        0x1f => "s",
        0x20 => "d",
        0x21 => "f",
        0x22 => "g",
        0x23 => "h",
        0x24 => "j",
        0x25 => "k",
        0x26 => "l",
        0x27 => ";",
        0x28 => "'",
        0x29 => "`",
        0x2b => "\\",
        0x2c => "z",
        0x2d => "x",
        0x2e => "c",
        0x2f => "v",
        0x30 => "b",
        0x31 => "n",
        0x32 => "m",
        0x33 => ",",
        0x34 => ".",
        0x35 => "/",
        _ => return None,
    })
}

#[derive(Default)]
pub(crate) struct ModifierState {
    pub left_shift: bool,
    pub right_shift: bool,
    pub left_ctrl: bool,
    pub right_ctrl: bool,
    pub left_alt: bool,
    pub right_alt: bool,
    pub left_meta: bool,
    pub right_meta: bool,
}

/// Convert either pre-message or post-message OS samples to post-event flags.
/// No engine state changes here; unchanged aggregate Shift edges have no event.
pub(crate) fn from_native(
    vk: u16,
    flags: u32,
    release: bool,
    mut state: ModifierState,
) -> Option<KeyEvent> {
    let extended = flags & (1 << 24) != 0;
    let pressed = !release;
    match vk {
        0x10 | 0xa0 | 0xa1 => {
            let right = vk == 0xa1 || (vk == 0x10 && (flags >> 16) & 0xff == 0x36);
            if if right {
                state.left_shift
            } else {
                state.right_shift
            } {
                return None;
            }
            if right {
                state.right_shift = pressed;
            } else {
                state.left_shift = pressed;
            }
        }
        0x11 | 0xa2 | 0xa3 => {
            if vk == 0xa3 || (vk == 0x11 && extended) {
                state.right_ctrl = pressed;
            } else {
                state.left_ctrl = pressed;
            }
        }
        0x12 | 0xa4 | 0xa5 => {
            if vk == 0xa5 || (vk == 0x12 && extended) {
                state.right_alt = pressed;
            } else {
                state.left_alt = pressed;
            }
        }
        0x5b => state.left_meta = pressed,
        0x5c => state.right_meta = pressed,
        _ => {}
    }
    Some(normalize(
        vk,
        flags,
        release,
        KeyEvent {
            shift: state.left_shift || state.right_shift,
            ctrl: state.left_ctrl || state.right_ctrl,
            alt: state.left_alt || state.right_alt,
            meta: state.left_meta || state.right_meta,
            ..Default::default()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lowercase_names_match_core_contract() {
        for (native, normalized) in [
            (0x14, "caps_lock"),
            (0x21, "page_up"),
            (0x22, "page_down"),
            (0x41, "a"),
            (0x5a, "z"),
            (0x31, "1"),
            (0xbf, "/"),
            (0xa1, "shift"),
        ] {
            assert_eq!(
                normalize(native, 0, false, KeyEvent::default()).key,
                normalized
            );
        }
    }

    #[test]
    fn physical_keys_ignore_text_layout_but_preserve_native_actions() {
        assert_eq!(
            normalize(0x41, 0x10 << 16, false, KeyEvent::default()).key,
            "q"
        );
        assert_eq!(
            normalize(0x59, 0x2c << 16, false, KeyEvent::default()).key,
            "z"
        );
        assert_eq!(
            normalize(0x11, 0x3a << 16, false, KeyEvent::default()).key,
            "control"
        );
        assert_eq!(
            normalize(0x1b, 0x3a << 16, false, KeyEvent::default()).key,
            "escape"
        );
        assert_eq!(
            normalize(0x70, 0x1e << 16, false, KeyEvent::default()).key,
            "vk_0070"
        );
        assert_eq!(
            normalize(0x61, 0x4f << 16, false, KeyEvent::default()).key,
            "vk_0061"
        );
    }

    #[test]
    fn modifier_samples_are_post_event_and_shift_edges_are_aggregated() {
        for old in [false, true] {
            let down = from_native(
                0x10,
                0x2a << 16,
                false,
                ModifierState {
                    left_shift: old,
                    ..Default::default()
                },
            )
            .unwrap();
            let up = from_native(
                0x10,
                0x2a << 16,
                true,
                ModifierState {
                    left_shift: old,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(down.shift && !up.shift);
            let ctrl = from_native(
                0x11,
                0x1d << 16,
                false,
                ModifierState {
                    left_ctrl: old,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(ctrl.ctrl);
            let alt = from_native(
                0x12,
                0x38 << 16,
                true,
                ModifierState {
                    left_alt: old,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(!alt.alt);
        }
        assert!(
            from_native(
                0x10,
                0x36 << 16,
                false,
                ModifierState {
                    left_shift: true,
                    ..Default::default()
                }
            )
            .is_none()
        );
        assert!(
            from_native(
                0x10,
                0x36 << 16,
                true,
                ModifierState {
                    left_shift: true,
                    right_shift: true,
                    ..Default::default()
                }
            )
            .is_none()
        );
        let final_up = from_native(
            0x10,
            0x2a << 16,
            true,
            ModifierState {
                left_shift: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!final_up.shift && final_up.release);
    }

    #[test]
    fn modifiers_repeat_and_release_are_not_lost() {
        let modifiers = KeyEvent {
            ctrl: true,
            alt: true,
            shift: true,
            meta: true,
            ..KeyEvent::default()
        };
        let repeated = normalize(0x41, 1 << 30, false, modifiers.clone());
        assert!(repeated.repeat && !repeated.release);
        assert!(repeated.ctrl && repeated.alt && repeated.shift && repeated.meta);
        let release = normalize(0x41, 1 << 31 | 1 << 30, true, modifiers);
        assert!(release.release);
        assert!(!release.repeat);
        assert_eq!(
            normalize(0xffff, 0, false, KeyEvent::default()).key,
            "vk_ffff"
        );
    }
}
