#pragma once

#include <bopomofo.h>
#include <cstdint>
#include <memory>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <vector>

namespace bopomofo {
using Json = nlohmann::json;
inline constexpr std::size_t PageSize = 9; // ime-core::PAGE_SIZE, ABI v1.
inline constexpr std::size_t MaxLexiconBytes = 32 * 1024 * 1024;

struct Candidate {
    std::string text;
    std::size_t start;
    std::size_t end;
};
struct Snapshot {
    std::string preedit;
    std::size_t cursor;
    std::vector<Candidate> candidates;
    std::size_t page;
    std::size_t candidateIndex; // Absolute, not page-relative.
    bool selecting;
    std::uint64_t generation;
    std::string mode;
    bool active;
};
struct Output {
    bool consumed;
    Snapshot snapshot;
    std::optional<std::string> commit;
    std::optional<std::string> status;
};

std::string readLexicon(const std::string &path);
std::string readChecksum(const std::string &path);
void verifyLexicon(const std::string &lexicon, const std::string &sha256);
std::string loadVerifiedLexicon(const std::string &path, const std::string &checksumPath);
Snapshot decodeSnapshot(const Json &json);
Output decodeOutput(const Json &json);
Json keyEvent(const std::string &key);

// FFI allocations never use a C++ allocator. Each handle is context-local.
class CoreClient {
public:
    explicit CoreClient(const std::string &lexicon);
    Output handle(const Json &event);
    Snapshot snapshot();
    CoreClient(const CoreClient &) = delete;
    CoreClient &operator=(const CoreClient &) = delete;
private:
    struct Deleter { void operator()(BpmEngine *value) const { bpm_engine_free(value); } };
    std::unique_ptr<BpmEngine, Deleter> engine_;
};
} // namespace bopomofo
