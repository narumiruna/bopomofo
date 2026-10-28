#include "keymap.h"
#include <stdexcept>
#include <string_view>

namespace bopomofo {
namespace {
bool nativeAction(xkb_keysym_t symbol) {
    // Preserve actual host modifier/action remaps (for example Ctrl or Escape
    // on the Caps position). Dead character keysyms still use physical US.
    return symbol == XKB_KEY_space || (symbol >= 0xff00 && symbol <= 0xffff) ||
           (symbol >= XKB_KEY_ISO_Lock && symbol <= XKB_KEY_ISO_Level5_Lock) ||
           symbol == XKB_KEY_ISO_Left_Tab;
}
std::string keyName(xkb_keysym_t sym) {
    switch (sym) {
    case XKB_KEY_Return: case XKB_KEY_KP_Enter: return "enter";
    case XKB_KEY_Escape: return "escape";
    case XKB_KEY_BackSpace: return "backspace";
    case XKB_KEY_Delete: case XKB_KEY_KP_Delete: return "delete";
    case XKB_KEY_Left: case XKB_KEY_KP_Left: return "left";
    case XKB_KEY_Right: case XKB_KEY_KP_Right: return "right";
    case XKB_KEY_Up: case XKB_KEY_KP_Up: return "up";
    case XKB_KEY_Down: case XKB_KEY_KP_Down: return "down";
    case XKB_KEY_Home: case XKB_KEY_KP_Home: return "home";
    case XKB_KEY_End: case XKB_KEY_KP_End: return "end";
    case XKB_KEY_Page_Up: case XKB_KEY_KP_Page_Up: return "page_up";
    case XKB_KEY_Page_Down: case XKB_KEY_KP_Page_Down: return "page_down";
    case XKB_KEY_space: return "space";
    case XKB_KEY_Tab: case XKB_KEY_ISO_Left_Tab: return "tab";
    case XKB_KEY_Shift_L: case XKB_KEY_Shift_R: return "shift";
    case XKB_KEY_Control_L: case XKB_KEY_Control_R: return "control";
    case XKB_KEY_Alt_L: case XKB_KEY_Alt_R: case XKB_KEY_ISO_Level3_Shift: return "alt";
    case XKB_KEY_Meta_L: case XKB_KEY_Meta_R: case XKB_KEY_Super_L: case XKB_KEY_Super_R:
    case XKB_KEY_Hyper_L: case XKB_KEY_Hyper_R: return "meta";
    case XKB_KEY_Caps_Lock: return "caps_lock";
    default: break;
    }
    if (sym >= 'A' && sym <= 'Z') { sym += 'a' - 'A'; }
    // Code-less frontends can supply a shifted US keysym. Preserve the actual
    // Shift flag separately; never turn shifted punctuation into a tone here.
    constexpr std::string_view shifted = "~!@#$%^&*()_+{}|:\"<>?";
    constexpr std::string_view base = "`1234567890-=[]\\;',./";
    const auto pos = shifted.find(static_cast<char>(sym));
    if (sym < 128 && pos != std::string_view::npos) { sym = base[pos]; }
    if (sym >= 0x21 && sym <= 0x7e) { return std::string(1, static_cast<char>(sym)); }
    return "unidentified";
}
} // namespace

Keymap::Keymap() : context_(xkb_context_new(XKB_CONTEXT_NO_ENVIRONMENT_NAMES)) {
    if (!context_) { throw std::runtime_error("Keyboard map unavailable"); }
    xkb_context_set_log_fn(context_.get(), [](xkb_context *, xkb_log_level, const char *, va_list) {});
    const xkb_rule_names names{"evdev", "pc105", "us", "", ""};
    keymap_.reset(xkb_keymap_new_from_names(context_.get(), &names, XKB_KEYMAP_COMPILE_NO_FLAGS));
    if (!keymap_) { throw std::runtime_error("Keyboard map unavailable"); }
}

NormalizedKey Keymap::translate(const fcitx::Key &key, bool release, ShiftState &shift) const {
    auto symbol = static_cast<xkb_keysym_t>(key.sym());
    if (key.code() > 0 && !nativeAction(symbol)) {
        const xkb_keysym_t *symbols = nullptr;
        // Fcitx uses XKB keycodes (evdev + 8) for X11 and Wayland. Override
        // only printable US positions, never a US Caps/Control/action key.
        if (xkb_keymap_key_get_syms_by_level(keymap_.get(), key.code(), 0, 0, &symbols) == 1 &&
            symbols[0] >= 0x21 && symbols[0] <= 0x7e) {
            symbol = symbols[0];
        }
    }
    auto result = keyEvent(keyName(symbol));
    if (result.at("key") == "unidentified") {
        // Keep unknown held keys distinct. Their releases still matter to
        // core modifier tracking, even though they have no phonetic meaning.
        result["key"] = key.code() > 0 ? "code_" + std::to_string(key.code())
                                      : "sym_" + std::to_string(static_cast<uint32_t>(key.sym()));
    }
    const auto states = key.states();
    result["ctrl"] = states.test(fcitx::KeyState::Ctrl);
    result["alt"] = states.test(fcitx::KeyState::Alt) || states.test(fcitx::KeyState::Mod5);
    result["meta"] = states.test(fcitx::KeyState::Super) || states.test(fcitx::KeyState::Super2) ||
                       states.test(fcitx::KeyState::Meta) || states.test(fcitx::KeyState::Hyper) ||
                       states.test(fcitx::KeyState::Hyper2);
    result["shift"] = states.test(fcitx::KeyState::Shift);
    const bool isShift = symbol == XKB_KEY_Shift_L || symbol == XKB_KEY_Shift_R;
    if (!states.test(fcitx::KeyState::Shift) && !(isShift && release)) {
        // Resynchronize after releases outside this context. Keep the old
        // state for a release itself: some frontends report its post-mask.
        shift = {};
    }
    const bool before = shift.left || shift.right;
    bool deliver = true;
    if (isShift) {
        auto &side = symbol == XKB_KEY_Shift_L ? shift.left : shift.right;
        side = !release;
        const bool after = shift.left || shift.right;
        result["shift"] = after;
        // Normalize physical sides to one logical Shift. This is not a new
        // gesture after Reset if another physical side remains held.
        deliver = before != after;
    }
    result["repeat"] = states.test(fcitx::KeyState::Repeat);
    result["release"] = release;
    return {std::move(result), deliver};
}
} // namespace bopomofo
