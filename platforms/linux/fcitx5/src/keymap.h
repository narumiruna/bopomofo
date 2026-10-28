#pragma once

#include "core_client.h"
#include <fcitx-utils/key.h>
#include <memory>
#include <xkbcommon/xkbcommon.h>

namespace bopomofo {
// Native Shift state is context-local. X11 release masks describe the state
// before release; core needs the remaining aggregate Shift state instead.
struct ShiftState { bool left = false; bool right = false; };
struct NormalizedKey {
    Json event;
    bool deliver; // Only aggregate Shift edges are delivered to core.
};

// A fixed US physical layout, loaded once. No Zhuyin mapping is done here.
class Keymap {
public:
    Keymap();
    NormalizedKey translate(const fcitx::Key &key, bool release, ShiftState &shift) const;
private:
    struct ContextFree { void operator()(xkb_context *p) const { xkb_context_unref(p); } };
    struct KeymapFree { void operator()(xkb_keymap *p) const { xkb_keymap_unref(p); } };
    std::unique_ptr<xkb_context, ContextFree> context_;
    std::unique_ptr<xkb_keymap, KeymapFree> keymap_;
};
} // namespace bopomofo
