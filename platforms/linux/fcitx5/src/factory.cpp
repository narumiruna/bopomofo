#include "addon.h"
#include "config.h"
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/instance.h>

namespace bopomofo {
class Factory final : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        try {
            if (!manager || !manager->instance()) { return nullptr; }
            return new Addon(manager->instance()->inputContextManager(), BOPOMOFO_LEXICON_FILE, BOPOMOFO_LEXICON_CHECKSUM);
        } catch (...) {
            // Fcitx can continue with other input methods. Never log input text.
            return nullptr;
        }
    }
};
} // namespace bopomofo
FCITX_ADDON_FACTORY(bopomofo::Factory)
