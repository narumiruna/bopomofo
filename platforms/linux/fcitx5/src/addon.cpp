#include "addon.h"

#include <algorithm>
#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/textformatflags.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputpanel.h>
#include <stdexcept>

namespace bopomofo {
struct Context {
    std::shared_ptr<CoreClient> core;
    std::optional<Snapshot> snapshot;
    ShiftState shift;
    bool enabled = false;
};

namespace {
bool allowed(fcitx::InputContext *ic) {
    const auto flags = ic->capabilityFlags();
    return !flags.test(fcitx::CapabilityFlag::Disable) &&
           !flags.test(fcitx::CapabilityFlag::Password) &&
           !flags.test(fcitx::CapabilityFlag::Sensitive);
}
void clear(fcitx::InputContext *ic) {
    ic->inputPanel().reset();
    ic->updatePreedit();
    ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel, true);
}
void cancel(const std::shared_ptr<Context> &state, fcitx::InputContext *ic) noexcept {
    if (state) {
        state->enabled = false;
        state->snapshot.reset();
        state->core.reset();
    }
    // Never let C++ or JSON exceptions cross a native callback. No input logs.
    try { clear(ic); } catch (...) {}
}
bool dispatch(const std::shared_ptr<Context> &state, fcitx::InputContext *ic,
              const Json &event) noexcept;

class Word final : public fcitx::CandidateWord {
public:
    Word(std::string text, const std::shared_ptr<Context> &state, fcitx::InputContext *ic,
         std::uint64_t generation, std::size_t index)
        : CandidateWord(fcitx::Text(std::move(text))), state_(state),
          ic_(ic->watch()), core_(state->core), generation_(generation), index_(index) {}
    void select(fcitx::InputContext *ic) const override {
        if (!ic || ic != ic_.get() || core_.expired()) { return; }
        if (auto state = state_.lock(); state && state->enabled && ic->hasFocus() && allowed(ic)) {
            try {
                // Always send the captured generation and absolute index to
                // core. Core alone validates stale selection and locks spans.
                dispatch(state, ic, {{"type", "select"}, {"generation", generation_}, {"index", index_}});
            } catch (...) { cancel(state, ic); }
        }
    }
private:
    std::weak_ptr<Context> state_;
    fcitx::TrackableObjectReference<fcitx::InputContext> ic_;
    std::weak_ptr<CoreClient> core_; // Invalidates callbacks after engine replacement.
    std::uint64_t generation_;
    std::size_t index_;
};

// This is a view of ONE core snapshot. Fcitx must not change its page or
// selection locally (CommonCandidateList would have its own paging rules).
class List final : public fcitx::CandidateList, public fcitx::PageableCandidateList {
public:
    List(const std::shared_ptr<Context> &state, fcitx::InputContext *ic, const Snapshot &s)
        : state_(state), ic_(ic->watch()), core_(state->core), generation_(s.generation), page_(s.page),
          total_((s.candidates.size() + PageSize - 1) / PageSize),
          cursor_(static_cast<int>(s.candidateIndex - s.page * PageSize)) {
        setPageable(this);
        const auto start = page_ * PageSize;
        const auto end = std::min(start + PageSize, s.candidates.size());
        for (auto i = start; i < end; ++i) {
            words_.push_back(std::make_unique<Word>(s.candidates[i].text, state, ic, s.generation, i));
            labels_.emplace_back(std::to_string(i - start + 1) + ".");
        }
    }
    const fcitx::Text &label(int i) const override { return labels_.at(i); }
    const fcitx::CandidateWord &candidate(int i) const override { return *words_.at(i); }
    int size() const override { return static_cast<int>(words_.size()); }
    int cursorIndex() const override { return cursor_; }
    fcitx::CandidateLayoutHint layoutHint() const override { return fcitx::CandidateLayoutHint::Vertical; }
    bool hasPrev() const override { return page_ > 0; }
    bool hasNext() const override { return page_ + 1 < total_; }
    int totalPages() const override { return static_cast<int>(total_); }
    int currentPage() const override { return static_cast<int>(page_); }
    bool usedNextBefore() const override { return page_ > 0; }
    void prev() override { pageKey("page_up"); }
    void next() override { pageKey("page_down"); }
    // Arbitrary setPage is intentionally not implemented: the core API exposes
    // next/previous only. Native digit presses go through Addon::keyEvent.
private:
    void pageKey(const char *key) noexcept {
        auto state = state_.lock();
        auto *ic = ic_.get();
        if (!state || !ic || core_.expired() || !state->enabled || !ic->hasFocus() || !allowed(ic) ||
            !state->snapshot || state->snapshot->generation != generation_) { return; }
        try {
            auto event = keyEvent(key);
            dispatch(state, ic, event);
            // A UI page action represents a complete tap, not a held physical
            // key. Supply its release to core held-key tracking as well.
            if (state->enabled && state->core) {
                event["release"] = true;
                dispatch(state, ic, event);
            }
        } catch (...) { cancel(state, ic); }
    }
    std::weak_ptr<Context> state_;
    fcitx::TrackableObjectReference<fcitx::InputContext> ic_;
    std::weak_ptr<CoreClient> core_;
    std::uint64_t generation_;
    std::size_t page_, total_;
    int cursor_;
    std::vector<std::unique_ptr<Word>> words_;
    std::vector<fcitx::Text> labels_;
};

bool dispatch(const std::shared_ptr<Context> &state, fcitx::InputContext *ic,
              const Json &event) noexcept {
    try {
        if (!state->core) { return false; }
        auto output = state->core->handle(event);
        const auto &s = output.snapshot;
        // Prepare the full UI before committing. A malformed snapshot never
        // causes a partial native update or a retry of a transient commit.
        fcitx::Text preedit;
        if (!s.preedit.empty()) {
            preedit.append(s.preedit, fcitx::TextFormatFlags{fcitx::TextFormatFlag::Underline, fcitx::TextFormatFlag::DontCommit});
            preedit.setCursor(static_cast<int>(s.cursor)); // Fcitx Text uses UTF-8 BYTES.
        }
        std::unique_ptr<fcitx::CandidateList> list;
        if (s.selecting) { list = std::make_unique<List>(state, ic, s); }
        state->snapshot = std::move(output.snapshot);
        auto &panel = ic->inputPanel();
        panel.reset();
        if (ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit) && ic->isPreeditEnabled()) {
            panel.setClientPreedit(preedit);
        } else {
            panel.setPreedit(preedit);
        }
        panel.setCandidateList(std::move(list));
        // commit belongs only to this event, never to the stored snapshot.
        if (output.commit && !output.commit->empty()) { ic->commitString(*output.commit); }
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        return output.consumed;
    } catch (...) {
        cancel(state, ic);
        return false;
    }
}
} // namespace

ContextProperty::ContextProperty() : context(std::make_shared<Context>()) {}

Addon::Addon(fcitx::InputContextManager &manager, const std::string &lexiconPath, const std::string &checksumPath)
    : lexicon_(loadVerifiedLexicon(lexiconPath, checksumPath)),
      factory_([](fcitx::InputContext &) { return new ContextProperty; }) {
    // Reject an unavailable ABI or invalid versioned lexicon at addon creation,
    // not on the first key. Source bytes remain immutable thereafter.
    CoreClient validate(lexicon_);
    if (!manager.registerProperty("bopomofo-dev-context", &factory_)) {
        throw std::runtime_error("Context registration failed");
    }
    if (auto *instance = manager.instance()) {
        capabilityWatcher_ = instance->watchEvent(fcitx::EventType::InputContextCapabilityChanged,
            fcitx::EventWatcherPhase::PreInputMethod, [this](fcitx::Event &event) {
                capabilityChanged(static_cast<fcitx::InputContextEvent &>(event));
            });
    }
}

void Addon::capabilityChanged(fcitx::InputContextEvent &event) {
    auto *ic = event.inputContext();
    std::shared_ptr<Context> state;
    try {
        state = ic->propertyFor(&factory_)->context;
        // Fcitx itself switches input methods for Password/Disable, but not
        // Sensitive. Clear a restricted owned context before any further key.
        // Never reactivate an input method or clear another engine's panel.
        if (!state->enabled || allowed(ic)) { return; }
        dispatch(state, ic, {{"type", "focus_lost"}});
        clear(ic);
    } catch (...) { cancel(state, ic); }
}

void Addon::keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) {
    auto *ic = event.inputContext();
    std::shared_ptr<Context> state;
    try {
        state = ic->propertyFor(&factory_)->context;
        auto normalized = keymap_.translate(event.rawKey(), event.isRelease(), state->shift);
        if (!ic->hasFocus()) { cancel(state, ic); return; }
        if (!allowed(ic)) { capabilityChanged(event); return; }
        if (!state->enabled || !normalized.deliver) { return; } // Recover errors only at the next Activate.
        if (dispatch(state, ic, normalized.event)) {
            event.filterAndAccept();
        }
    } catch (...) { cancel(state, ic); }
}

void Addon::lifecycle(fcitx::InputContextEvent &event, const char *type, bool enabled) {
    auto *ic = event.inputContext();
    std::shared_ptr<Context> state;
    try {
        state = ic->propertyFor(&factory_)->context;
        if (enabled && !allowed(ic)) { cancel(state, ic); return; }
        if (enabled && !state->core) { state->core = std::make_shared<CoreClient>(lexicon_); }
        state->enabled = enabled;
        dispatch(state, ic, {{"type", type}});
    } catch (...) { cancel(state, ic); }
}
void Addon::activate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    lifecycle(event, "activate", true);
}
void Addon::deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    lifecycle(event, event.type() == fcitx::EventType::InputContextFocusOut ? "focus_lost" : "deactivate", false);
}
void Addon::reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) {
    auto *ic = event.inputContext();
    std::shared_ptr<Context> state;
    try {
        state = ic->propertyFor(&factory_)->context;
        if (state->core) { dispatch(state, ic, {{"type", "reset"}}); }
        else { clear(ic); }
    } catch (...) { cancel(state, ic); }
}
} // namespace bopomofo
