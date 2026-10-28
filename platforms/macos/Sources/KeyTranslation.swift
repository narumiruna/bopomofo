import Foundation

struct NativeModifiers: Codable, Equatable {
    var ctrl = false
    var alt = false
    var meta = false
    var shift = false
    var capsLock = false
    var function = false
}

enum NativeKeyKind: String, Codable { case down, up, flags }

struct NativeKey: Codable {
    let kind: NativeKeyKind
    let code: UInt16
    var characters: String? = nil
    var modifiers = NativeModifiers()
    var isRepeat = false
}

struct KeyTranslator {
    private var previous = NativeModifiers()
    private var pressed: [UInt16: String] = [:]

    mutating func reset(modifiers: NativeModifiers = NativeModifiers()) {
        previous = modifiers
        pressed.removeAll()
    }

    mutating func translate(_ native: NativeKey) -> KeyEvent? {
        let old = previous
        previous = native.modifiers
        let flags = native.modifiers
        let key: String
        var release = native.kind == .up
        if native.kind == .flags {
            let transition: (String, Bool, Bool)
            switch native.code {
            case 56, 60: transition = ("shift", old.shift, flags.shift)
            case 59, 62: transition = ("control", old.ctrl, flags.ctrl)
            case 58, 61: transition = ("alt", old.alt, flags.alt)
            case 54, 55: transition = ("meta", old.meta, flags.meta)
            case 57: transition = ("caps_lock", old.capsLock, flags.capsLock)
            case 63: transition = ("fn", old.function, flags.function)
            default: return nil
            }
            guard transition.1 != transition.2 else { return nil }
            key = transition.0
            // macOS reports Caps Lock latch changes, not a physical key release.
            // Each change is one logical press; the core owns the mode rule.
            release = native.code == 57 ? false : !transition.2
        } else {
            // Unmapped physical keys must not alias a Dachen key through
            // characters produced by another layout or a numeric keypad.
            let name = Self.physicalKeys[native.code] ?? "native_\(native.code)"
            if native.kind == .up {
                key = pressed.removeValue(forKey: native.code) ?? name
            } else {
                key = pressed[native.code] ?? name
                pressed[native.code] = key
            }
        }
        return KeyEvent(key: key, ctrl: flags.ctrl, alt: flags.alt, meta: flags.meta,
            shift: flags.shift, repeat: native.isRepeat, release: release)
    }

    // Dachen is a physical layout. Do not turn Shift+1 into a different Dachen key.
    private static let physicalKeys: [UInt16: String] = [
        0: "a", 1: "s", 2: "d", 3: "f", 4: "h", 5: "g", 6: "z", 7: "x", 8: "c", 9: "v",
        11: "b", 12: "q", 13: "w", 14: "e", 15: "r", 16: "y", 17: "t", 18: "1", 19: "2",
        20: "3", 21: "4", 22: "6", 23: "5", 24: "=", 25: "9", 26: "7", 27: "-", 28: "8",
        29: "0", 30: "]", 31: "o", 32: "u", 33: "[", 34: "i", 35: "p", 37: "l", 38: "j",
        39: "'", 40: "k", 41: ";", 42: "\\", 43: ",", 44: "/", 45: "n", 46: "m", 47: ".",
        36: "enter", 76: "enter", 53: "escape", 51: "backspace", 117: "delete", 48: "tab",
        49: "space", 123: "left", 124: "right", 125: "down", 126: "up", 115: "home",
        119: "end", 116: "page_up", 121: "page_down",
    ]
}
