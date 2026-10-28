#include "addon.h"
#include "core_client.h"

#include <fcitx-utils/textformatflags.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputpanel.h>
#include <fstream>
#include <iostream>
#include <stdexcept>

namespace {
using bopomofo::Json;
struct TestFailure : std::runtime_error { using std::runtime_error::runtime_error; };
void require(bool value, const char *message) {
    if (!value) { throw TestFailure(message); }
}
template<class F> void rejects(F action) {
    bool failed = false;
    try { action(); } catch (...) { failed = true; }
    require(failed, "Invalid output was accepted");
}
void matches(const Json &actual, const Json &expected) {
    if (expected.is_object()) {
        require(actual.is_object(), "Replay output shape mismatch");
        for (const auto &[name, value] : expected.items()) {
            require(actual.contains(name), "Replay field missing");
            matches(actual.at(name), value);
        }
    } else { require(actual == expected, "Replay output mismatch"); }
}
struct Buffer {
    BpmBuffer value{nullptr, 0};
    ~Buffer() { bpm_buffer_free(value); }
    Json json() const {
        require(value.data && value.len, "Missing ABI buffer");
        return Json::parse(value.data, value.data + value.len);
    }
};
struct Engine {
    BpmEngine *value = nullptr;
    explicit Engine(const std::string &lexicon) {
        require(bpm_engine_new(reinterpret_cast<const uint8_t *>(lexicon.data()), lexicon.size(), &value) == BPM_OK && value,
                "ABI creation failed");
    }
    ~Engine() { bpm_engine_free(value); }
    Json handle(const Json &event) {
        const auto bytes = event.dump();
        Buffer output;
        require(bpm_engine_handle(value, reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size(), &output.value) == BPM_OK,
                "ABI event failed");
        const auto json = output.json();
        bopomofo::decodeOutput(json);
        return json;
    }
    Json snapshot() const {
        Buffer output;
        require(bpm_engine_snapshot(value, &output.value) == BPM_OK, "ABI snapshot failed");
        const auto json = output.json();
        bopomofo::decodeSnapshot(json);
        return json;
    }
};
void checksum(const std::string &lexicon, const std::string &checksumPath) {
    const auto hash = bopomofo::readChecksum(checksumPath);
    bopomofo::verifyLexicon(lexicon, hash);
    rejects([&] { bopomofo::verifyLexicon(lexicon + "x", hash); });
    auto invalid = hash; invalid[0] = hash[0] == '0' ? '1' : '0';
    rejects([&] { bopomofo::verifyLexicon(lexicon, invalid); });
    rejects([&] { bopomofo::verifyLexicon(lexicon, "invalid"); });
    rejects([&] { bopomofo::readChecksum(checksumPath + ".missing"); });
}
void abi(const std::string &lexicon) {
    for (int i = 0; i < 100; ++i) {
        Engine first(lexicon), second(lexicon);
        const auto before = first.snapshot();
        require(before == second.snapshot(), "Contexts do not start independently");
        Buffer out;
        require(bpm_engine_handle(first.value, nullptr, 1, &out.value) == BPM_INVALID, "Null event not rejected");
        const uint8_t bad[] = {0xff};
        require(bpm_engine_handle(first.value, bad, 1, &out.value) == BPM_DATA, "Invalid UTF-8 not rejected");
        const std::string invalid = R"({"type":"not_an_event"})";
        require(bpm_engine_handle(first.value, reinterpret_cast<const uint8_t *>(invalid.data()), invalid.size(), &out.value) == BPM_DATA,
                "Unknown event not rejected");
        require(!out.value.data && out.value.len == 0, "Error returned an unexpected allocation");
        require(first.snapshot() == before, "Rejected event changed state");
        auto key = bopomofo::keyEvent("1");
        key.erase("type");
        const auto bytes = key.dump();
        bool consumed = false;
        require(bpm_engine_probe(first.value, reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size(), &consumed) == BPM_OK,
                "ABI probe failed");
        require(first.snapshot() == before, "Probe changed state");
        first.handle(bopomofo::keyEvent("1"));
        auto release = bopomofo::keyEvent("1"); release["release"] = true;
        require(first.handle(release).at("commit").is_null(), "Key release committed text");
        require(second.snapshot() == before, "Context composition was shared");
    }
    bpm_engine_free(nullptr);
    bpm_buffer_free({nullptr, 0});
    // A C++ boundary failure discards the handle. It cannot retry a commit.
    bopomofo::CoreClient broken(lexicon);
    rejects([&] { broken.handle({{"type", "invalid"}}); });
    rejects([&] { broken.handle(bopomofo::keyEvent("enter")); });
}
void replay(const std::string &lexicon, const std::string &path) {
    std::ifstream file(path);
    require(static_cast<bool>(file), "Replay fixture unavailable");
    Engine engine(lexicon);
    std::string line;
    std::size_t count = 0;
    while (std::getline(file, line)) {
        if (line.empty()) { continue; }
        const auto record = Json::parse(line);
        const auto output = engine.handle(record.at("event"));
        matches(output, record.at("expect"));
        ++count;
    }
    require(count > 0 && file.eof(), "Replay fixture is empty or unreadable");
}
void unicode() {
    const std::string sample = "𠀀😀é"; // non-BMP Han, emoji, combining mark.
    Engine engine("# bopomofo-lexicon-v1\nㄅㄚ\t" + sample + "\t100\n");
    engine.handle(bopomofo::keyEvent("1"));
    engine.handle(bopomofo::keyEvent("8"));
    const auto output = engine.handle(bopomofo::keyEvent("space"));
    const auto snapshot = bopomofo::decodeSnapshot(output.at("snapshot"));
    require(snapshot.preedit == sample && snapshot.cursor == sample.size(), "Unicode byte cursor mismatch");
    fcitx::Text text(snapshot.preedit, fcitx::TextFormatFlag::DontCommit);
    text.setCursor(static_cast<int>(snapshot.cursor));
    require(text.cursor() == static_cast<int>(sample.size()), "Fcitx cursor is not a byte offset");
    require(text.toStringForCommit().empty(), "Fcitx would auto-commit on focus loss");
    const auto commit = engine.handle(bopomofo::keyEvent("enter"));
    require(commit.at("commit") == sample, "Unicode commit changed text");
    require(!engine.snapshot().contains("commit"), "Snapshot repeats a transient commit");
    require(engine.handle({{"type", "reset"}}).at("commit").is_null(), "Reset repeated a commit");
    auto invalid = output.at("snapshot"); invalid["cursor"] = std::uint64_t(1);
    rejects([&] { bopomofo::decodeSnapshot(invalid); });
    invalid = output.at("snapshot"); invalid["page"] = std::uint64_t(999);
    rejects([&] { bopomofo::decodeSnapshot(invalid); });
    invalid = output.at("snapshot"); invalid["mode"] = "invalid";
    rejects([&] { bopomofo::decodeSnapshot(invalid); });
    invalid = output; invalid["status"] = "invalid";
    rejects([&] { bopomofo::decodeOutput(invalid); });
    invalid = output; invalid["consumed"] = 1;
    rejects([&] { bopomofo::decodeOutput(invalid); });
}
void mapping() {
    bopomofo::Keymap map;
    bopomofo::ShiftState shift;
    using fcitx::Key; using fcitx::KeyState;
    auto translate = [&](const Key &key, bool release) { return map.translate(key, release, shift).event; };
    auto shifted = translate(Key(FcitxKey_exclam, fcitx::KeyStates{KeyState::Shift, KeyState::Repeat}, 10), false);
    require(shifted.at("key") == "1" && shifted.at("shift") == true && shifted.at("repeat") == true,
            "Physical shifted digit mapping failed");
    auto qwerty = translate(Key(FcitxKey_a, fcitx::KeyStates{KeyState::Ctrl, KeyState::Alt, KeyState::Super}, 24), true);
    require(qwerty.at("key") == "q" && qwerty.at("ctrl") == true && qwerty.at("alt") == true &&
            qwerty.at("meta") == true && qwerty.at("release") == true, "Physical US shortcut mapping failed");
    require(translate(Key(FcitxKey_A, KeyState::CapsLock), false).at("key") == "a", "Caps keysym mapping failed");
    require(translate(Key(FcitxKey_Shift_R), true).at("key") == "shift", "Shift release mapping failed");
    require(translate(Key(FcitxKey_Caps_Lock), false).at("key") == "caps_lock", "Caps key mapping failed");
    require(translate(Key(FcitxKey_Control_L, fcitx::KeyStates(), 66), false).at("key") == "control",
            "Control-on-Caps remap became Caps Lock");
    require(translate(Key(FcitxKey_Escape, fcitx::KeyStates(), 66), false).at("key") == "escape",
            "Escape-on-Caps remap became Caps Lock");
    require(translate(Key(FcitxKey_a, fcitx::KeyStates(), 66), false).at("key") == "a",
            "Printable-on-Caps remap became Caps Lock");
    require(translate(Key(FcitxKey_F1, fcitx::KeyStates(), 38), false).at("key") != "a",
            "Native action remap became a Dachen printable");
    require(translate(Key(FcitxKey_Control_L), false).at("key") == "control", "Control transition dropped");
    require(translate(Key(FcitxKey_Alt_R), false).at("key") == "alt", "Alt transition dropped");
    require(translate(Key(FcitxKey_Super_L), true).at("key") == "meta", "Meta transition dropped");
    require(translate(Key(FcitxKey_ISO_Left_Tab, KeyState::Shift), false).at("shift") == true,
            "Shift flag was normalized away");
    require(translate(Key(FcitxKey_at), false).at("key") == "2", "Code-less US fallback failed");
    require(translate(Key(FcitxKey_a, KeyState::Mod5), false).at("alt") == true, "AltGr must pass through");
    translate(Key(FcitxKey_Shift_L), false);
    translate(Key(FcitxKey_Shift_R, KeyState::Shift), false);
    require(translate(Key(FcitxKey_Shift_L, KeyState::Shift), true).at("shift") == true,
            "First Shift release lost the other held side");
    require(translate(Key(FcitxKey_Shift_R, KeyState::Shift), true).at("shift") == false,
            "Final Shift release retained the pre-release native mask");
}

class InputContext final : public fcitx::InputContext {
public:
    explicit InputContext(fcitx::InputContextManager &manager) : fcitx::InputContext(manager) {
        created(); setCapabilityFlags(fcitx::CapabilityFlag::Preedit); focusIn();
    }
    ~InputContext() override { destroy(); }
    const char *frontend() const override { return "bopomofo-contract"; }
    std::vector<std::string> commits;
private:
    void commitStringImpl(const std::string &text) override { commits.push_back(text); }
    void deleteSurroundingTextImpl(int, unsigned int) override {}
    void forwardKeyImpl(const fcitx::ForwardKeyEvent &) override {}
    void updatePreeditImpl() override {}
};
void nativeApi(const std::string &lexiconPath, const std::string &checksumPath) {
    fcitx::InputContextManager manager;
    manager.setPreeditEnabledByDefault(true);
    rejects([&] { bopomofo::Addon invalid(manager, lexiconPath, checksumPath + ".missing"); });
    bopomofo::Addon addon(manager, lexiconPath, checksumPath);
    const fcitx::InputMethodEntry entry("bopomofo-dev", "Bopomofo Development", "zh_TW", "bopomofo-dev");
    InputContext first(manager), second(manager);
    auto activate = [&](InputContext &ic) { fcitx::FocusInEvent event(&ic); addon.activate(entry, event); };
    auto rawKey = [&](InputContext &ic, fcitx::KeySym sym, bool release = false, fcitx::KeyStates flags = fcitx::KeyStates(), int code = 0) {
        fcitx::KeyEvent event(&ic, fcitx::Key(sym, flags, code), release);
        addon.keyEvent(entry, event);
        require(event.accepted() == event.filtered(), "Consumed event not filtered");
        return event.accepted();
    };
    auto key = [&](InputContext &ic, fcitx::KeySym sym, bool release = false, fcitx::KeyStates flags = fcitx::KeyStates()) {
        const auto consumed = rawKey(ic, sym, release, flags);
        // Ordinary calls represent a full tap. Modifier gesture tests send
        // explicit down/up transitions, including pre-release X11 masks.
        if (!release && !fcitx::Key(sym).isModifier()) { rawKey(ic, sym, true, flags); }
        return consumed;
    };
    auto ba = [&](InputContext &ic) { key(ic, FcitxKey_1); key(ic, FcitxKey_8); key(ic, FcitxKey_space); };
    auto reset = [&](InputContext &ic) { fcitx::ResetEvent event(&ic); addon.reset(entry, event); };
    activate(first); activate(second);
    ba(first);
    require(first.inputPanel().clientPreedit().toString() == "八", "Native preedit mismatch");
    require(first.inputPanel().clientPreedit().cursor() == 3, "Native cursor mismatch");
    require(first.inputPanel().clientPreedit().toStringForCommit().empty(), "Focus-out could auto-commit");
    require(second.inputPanel().clientPreedit().empty(), "Native contexts share composition");
    require(!rawKey(first, FcitxKey_Control_L, false, fcitx::KeyStates(), 66), "Remapped Control press consumed");
    require(!rawKey(first, FcitxKey_Control_L, true, fcitx::KeyState::Ctrl, 66), "Remapped Control release consumed");
    require(first.inputPanel().clientPreedit().toString() == "八", "Control-on-Caps changed composition or mode");
    require(rawKey(first, FcitxKey_Escape, false, fcitx::KeyStates(), 66), "Remapped Escape was not handled");
    rawKey(first, FcitxKey_Escape, true, fcitx::KeyStates(), 66);
    require(first.inputPanel().empty() && first.commits.empty(), "Escape-on-Caps did not cancel safely");
    require(rawKey(first, FcitxKey_a, false, fcitx::KeyStates(), 66), "Remapped printable became a mode switch");
    rawKey(first, FcitxKey_a, true, fcitx::KeyStates(), 66);
    require(first.inputPanel().clientPreedit().toString() == "ㄇ", "Printable-on-Caps did not remain printable");
    reset(first); ba(first);
    require(!key(first, FcitxKey_a, false, fcitx::KeyState::Ctrl), "Shortcut consumed");
    require(!key(first, FcitxKey_8, true), "Release consumed");
    key(first, FcitxKey_Down);
    auto page0 = first.inputPanel().candidateList();
    require(page0 && page0->size() == 9 && page0->cursorIndex() == 0, "Native candidate page zero mismatch");
    page0->toPageable()->next();
    auto page1 = first.inputPanel().candidateList();
    require(page1 && page1->toPageable()->currentPage() == 1 && page1->size() == 3 && page1->cursorIndex() == 0,
            "Native page callback did not use core");
    page0->toPageable()->next(); // Old page callbacks cannot advance current state.
    require(first.inputPanel().candidateList() == page1, "Stale pager changed current page");
    page1->candidate(0).select(&second);
    require(second.inputPanel().clientPreedit().empty(), "Candidate crossed input contexts");
    page1->candidate(0).select(&first);
    require(first.inputPanel().clientPreedit().toString() == "豝", "Absolute candidate callback mismatch");
    require(first.commits.empty(), "Candidate committed outside core rules");
    key(first, FcitxKey_Return);
    require(first.commits.size() == 1 && first.commits[0] == "豝", "Native commit mismatch");
    reset(first); reset(first);
    require(first.commits.size() == 1, "Repeated reset committed twice");
    ba(first); key(first, FcitxKey_Down);
    auto pager = first.inputPanel().candidateList(); pager->toPageable()->next();
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    require(!key(first, FcitxKey_1), "Native page action left a synthetic key held");
    key(first, FcitxKey_Caps_Lock); reset(first);
    ba(first); key(first, FcitxKey_Down); page0 = first.inputPanel().candidateList();
    reset(first); ba(first); key(first, FcitxKey_Down);
    auto current = first.inputPanel().candidateList();
    page0->candidate(1).select(&first); // Core validates stale generation.
    require(first.inputPanel().clientPreedit().toString() == "八" && first.commits.size() == 1,
            "Stale selection changed composition");
    require(first.inputPanel().candidateList()->toPageable()->currentPage() == current->toPageable()->currentPage(),
            "Stale selection changed page");
    key(first, FcitxKey_Page_Down); key(first, FcitxKey_1);
    require(first.inputPanel().clientPreedit().toString() == "豝", "Digit selection did not use core page");
    fcitx::FocusOutEvent out(&first); addon.deactivate(entry, out); first.focusOut();
    require(first.inputPanel().clientPreedit().empty() && first.commits.size() == 1, "Focus loss committed text");
    first.focusIn(); activate(first);
    ba(first); reset(first);
    key(first, FcitxKey_Shift_L);
    key(first, FcitxKey_Control_L, false, fcitx::KeyState::Shift);
    key(first, FcitxKey_Control_L, true, fcitx::KeyStates{fcitx::KeyState::Shift, fcitx::KeyState::Ctrl});
    key(first, FcitxKey_Shift_L, true);
    require(key(first, FcitxKey_1), "Intervening Control incorrectly allowed a Shift-alone toggle");
    reset(first);
    rawKey(first, FcitxKey_Shift_L);
    reset(first);
    rawKey(first, FcitxKey_Shift_R, false, fcitx::KeyState::Shift);
    require(!rawKey(first, FcitxKey_Shift_R, true, fcitx::KeyState::Shift),
            "Reset lost the other physical Shift and toggled too early");
    require(!rawKey(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift),
            "Secondary Shift rearmed a gesture cancelled by Reset");
    require(key(first, FcitxKey_1), "Reset with held Shift unexpectedly changed mode");
    reset(first);
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    require(!key(first, FcitxKey_1), "Fresh Shift tap after Reset did not switch mode");
    key(first, FcitxKey_Caps_Lock); reset(first);
    rawKey(first, FcitxKey_Return); rawKey(first, FcitxKey_KP_Enter);
    rawKey(first, FcitxKey_Return, true);
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    require(key(first, FcitxKey_1), "One Enter-alias release forgot the other held alias");
    key(first, FcitxKey_Escape); rawKey(first, FcitxKey_KP_Enter, true);
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    require(!key(first, FcitxKey_1), "Final Enter-alias release did not clear held state");
    key(first, FcitxKey_Caps_Lock); reset(first);
    rawKey(first, FcitxKey_Tab);
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    rawKey(first, FcitxKey_Tab, true);
    require(key(first, FcitxKey_1), "Held pass-through key incorrectly allowed a Shift-alone toggle");
    reset(first); ba(first);
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_R, false, fcitx::KeyState::Shift);
    key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    require(first.inputPanel().clientPreedit().toString() == "八", "First Shift release toggled mode too early");
    key(first, FcitxKey_Shift_R, true, fcitx::KeyState::Shift);
    require(!key(first, FcitxKey_1), "Final double-Shift release did not toggle mode");
    key(first, FcitxKey_Caps_Lock); reset(first);
    key(first, FcitxKey_Shift_L); key(first, FcitxKey_Shift_L, true, fcitx::KeyState::Shift);
    require(!key(first, FcitxKey_1), "Shift-alone did not switch to Latin");
    key(first, FcitxKey_Caps_Lock); ba(first);
    require(first.inputPanel().clientPreedit().toString() == "八", "Caps did not switch mode through core");
    key(first, FcitxKey_Down);
    auto restrictedCandidates = first.inputPanel().candidateList();
    require(restrictedCandidates && !restrictedCandidates->empty(), "Missing candidates before capability change");
    const auto normalFlags = first.capabilityFlags();
    const fcitx::CapabilityFlags sensitive{fcitx::CapabilityFlag::Preedit, fcitx::CapabilityFlag::Sensitive};
    first.setCapabilityFlags(sensitive);
    fcitx::CapabilityChangedEvent sensitiveEvent(&first, normalFlags, sensitive);
    // A standalone mock manager has no Instance event bus. Call the exact
    // registered watcher handler, with no intervening key or candidate click.
    addon.capabilityChanged(sensitiveEvent);
    require(first.inputPanel().empty() && first.commits.size() == 1,
            "Sensitive-only capability change did not immediately cancel presentation");
    first.setCapabilityFlags(normalFlags);
    fcitx::CapabilityChangedEvent normalEvent(&first, sensitive, normalFlags);
    addon.capabilityChanged(normalEvent);
    require(!key(first, FcitxKey_Return) && first.inputPanel().empty() && first.commits.size() == 1,
            "Old composition revived after Sensitive was removed");
    restrictedCandidates->candidate(1).select(&first);
    require(first.inputPanel().empty() && first.commits.size() == 1,
            "Restricted-interval candidate callback revived composition");
    ba(first);
    first.setCapabilityFlags(fcitx::CapabilityFlag::Password);
    require(!key(first, FcitxKey_1) && first.inputPanel().clientPreedit().empty(), "Password field bypassed");
    require(first.commits.size() == 1, "Sensitive-field cancellation committed text");
    first.setCapabilityFlags(fcitx::CapabilityFlag::NoFlag); activate(first);
    page0->candidate(1).select(&first); // The restricted interval cancelled the old snapshot.
    require(first.inputPanel().empty(), "Callback survived restricted-context reset");
    std::shared_ptr<fcitx::CandidateList> destroyedContext;
    {
        InputContext temporary(manager); activate(temporary); ba(temporary); key(temporary, FcitxKey_Down);
        destroyedContext = temporary.inputPanel().candidateList();
    }
    destroyedContext->candidate(0).select(&first);
    require(first.inputPanel().empty(), "Callback survived input-context destruction");
    ba(first);
    require(first.inputPanel().clientPreedit().empty() && first.inputPanel().preedit().toString() == "八",
            "Panel preedit fallback failed");
    fcitx::InputContextEvent deactivate(&first, fcitx::EventType::InputContextSwitchInputMethod);
    addon.deactivate(entry, deactivate);
    fcitx::CapabilityChangedEvent disabled(&first, fcitx::CapabilityFlag::Disable, fcitx::CapabilityFlag::NoFlag);
    addon.capabilityChanged(disabled);
    require(first.inputPanel().empty() && !key(first, FcitxKey_1), "Capability change reactivated a disabled context");

    // Controlled model of Fcitx5 5.1.7 DBus SelectCandidate(int): it resolves
    // the CURRENT list, without the generation of the previously rendered UI.
    // Reproducing this gap is not a native stale-UI safety acceptance test.
    activate(first); ba(first); key(first, FcitxKey_Down);
    auto rendered = first.inputPanel().candidateList(); rendered->toPageable()->next();
    rendered = first.inputPanel().candidateList();
    const int delayedRenderedIndex = 0;
    require(rendered->candidate(delayedRenderedIndex).text().toString() == "豝", "Missing old rendered page for gap model");
    reset(first); ba(first); key(first, FcitxKey_Down);
    auto resolvedNow = first.inputPanel().candidateList();
    require(resolvedNow->candidate(delayedRenderedIndex).text().toString() == "八", "Missing replacement page for gap model");
    rendered->candidate(delayedRenderedIndex).select(&first);
    require(static_cast<bool>(first.inputPanel().candidateList()), "Captured stale Word was not rejected");
    // This is the index-only DBus resolution step, NOT the old Word callback.
    first.inputPanel().candidateList()->candidate(delayedRenderedIndex).select(&first);
    require(!first.inputPanel().candidateList() && first.inputPanel().preedit().toString() == "八" && first.commits.size() == 1,
            "Known index-only current-list gap did not reproduce");
    reset(first);
    std::cout << "Known DBus index-only stale-UI gap reproduced; native safety remains blocked\n";
}
} // namespace

int main(int argc, char **argv) {
    try {
        require(argc >= 4, "Pass lexicon, checksum, and replay paths");
        require(bpm_abi_version() == BPM_ABI_VERSION, "C ABI version mismatch");
        const auto lexicon = bopomofo::loadVerifiedLexicon(argv[1], argv[2]);
        mapping(); checksum(lexicon, argv[2]); abi(lexicon); unicode();
        for (int i = 3; i < argc; ++i) { replay(lexicon, argv[i]); }
        nativeApi(argv[1], argv[2]);
        std::cout << "C++ ABI, replay, key mapping, and headless Fcitx API contracts passed\n";
        return 0;
    } catch (const TestFailure &failure) {
        std::cerr << failure.what() << '\n'; // Only fixed, non-input assertion labels.
        return 1;
    } catch (const std::exception &) {
        // Exception details can contain a JSON value. Never print input text.
        std::cerr << "C++ adapter contract failed\n";
        return 1;
    }
}
