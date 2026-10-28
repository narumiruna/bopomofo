#pragma once

#include "keymap.h"
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>

namespace bopomofo {
struct Context;
class ContextProperty : public fcitx::InputContextProperty {
public:
    ContextProperty();
    std::shared_ptr<Context> context;
    // The default needCopy() is false. Composition and mode are never shared.
};

class Addon final : public fcitx::InputMethodEngine {
public:
    Addon(fcitx::InputContextManager &manager, const std::string &lexiconPath, const std::string &checksumPath);
    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override;
    void activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override;
    void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override;
    void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override;
    // Shared by the Instance watcher and the headless capability contract test.
    void capabilityChanged(fcitx::InputContextEvent &event);
private:
    void lifecycle(fcitx::InputContextEvent &event, const char *type, bool enabled);
    const std::string lexicon_;
    Keymap keymap_;
    fcitx::FactoryFor<ContextProperty> factory_;
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>> capabilityWatcher_;
};
} // namespace bopomofo
