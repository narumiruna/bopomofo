#include "core_client.h"

#include <climits>
#include <fcitx-utils/utf8.h>
#include <fstream>
#include <limits>
#include <stdexcept>

namespace bopomofo {
static_assert(BPM_ABI_VERSION == 1, "Update the adapter for this C ABI version");
namespace {
[[noreturn]] void invalid() { throw std::runtime_error("Invalid core contract"); }
void fields(const Json &j, std::initializer_list<const char *> keys) {
    if (!j.is_object() || j.size() != keys.size()) { invalid(); }
    for (const auto *key : keys) { if (!j.contains(key)) { invalid(); } }
}
std::uint64_t number(const Json &j) {
    if (!j.is_number_unsigned()) { invalid(); }
    return j.get<std::uint64_t>();
}
std::size_t index(const Json &j) {
    const auto n = number(j);
    if (n > std::numeric_limits<std::size_t>::max()) { invalid(); }
    return static_cast<std::size_t>(n);
}
bool boolean(const Json &j) {
    if (!j.is_boolean()) { invalid(); }
    return j.get<bool>();
}
std::string text(const Json &j) {
    if (!j.is_string()) { invalid(); }
    auto value = j.get<std::string>();
    if (value.size() > INT_MAX || value.find('\0') != std::string::npos ||
        (!value.empty() && !fcitx::utf8::validate(value))) { invalid(); }
    return value;
}
struct Buffer {
    BpmBuffer value{nullptr, 0};
    ~Buffer() { bpm_buffer_free(value); }
    Json parse() const {
        // Outputs are bounded by core limits; reject a broken ABI before parsing.
        if (!value.data || !value.len || value.len > MaxLexiconBytes) { invalid(); }
        return Json::parse(value.data, value.data + value.len);
    }
};
} // namespace

std::string readLexicon(const std::string &path) {
    std::ifstream input(path, std::ios::binary | std::ios::ate);
    if (!input) { throw std::runtime_error("Lexicon unavailable"); }
    const auto length = input.tellg();
    if (length <= 0 || length > static_cast<std::streamoff>(MaxLexiconBytes)) {
        throw std::runtime_error("Invalid lexicon size");
    }
    std::string result(static_cast<std::size_t>(length), '\0');
    input.seekg(0);
    if (!input.read(result.data(), length) || input.peek() != EOF) {
        throw std::runtime_error("Lexicon read failed");
    }
    return result;
}

std::string readChecksum(const std::string &path) {
    std::ifstream input(path, std::ios::binary);
    char bytes[66]{};
    if (!input) { throw std::runtime_error("Lexicon manifest unavailable"); }
    input.read(bytes, sizeof(bytes));
    if (input.bad() || input.gcount() != 65 || bytes[64] != '\n') {
        throw std::runtime_error("Invalid lexicon manifest");
    }
    std::string result(bytes, 64);
    for (const auto byte : result) {
        if (!((byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f'))) {
            throw std::runtime_error("Invalid lexicon manifest");
        }
    }
    return result;
}

void verifyLexicon(const std::string &lexicon, const std::string &sha256) {
    if (bpm_abi_version() != BPM_ABI_VERSION) { throw std::runtime_error("C ABI version mismatch"); }
    if (bpm_lexicon_verify(reinterpret_cast<const uint8_t *>(lexicon.data()), lexicon.size(),
            reinterpret_cast<const uint8_t *>(sha256.data()), sha256.size()) != BPM_OK) {
        throw std::runtime_error("Lexicon checksum failed");
    }
}
std::string loadVerifiedLexicon(const std::string &path, const std::string &checksumPath) {
    auto lexicon = readLexicon(path);
    verifyLexicon(lexicon, readChecksum(checksumPath));
    return lexicon;
}

Snapshot decodeSnapshot(const Json &j) {
    fields(j, {"preedit", "cursor", "candidates", "page", "candidate_index",
               "selecting", "generation", "mode", "active"});
    Snapshot s{text(j.at("preedit")), index(j.at("cursor")), {},
               index(j.at("page")), index(j.at("candidate_index")),
               boolean(j.at("selecting")), number(j.at("generation")),
               text(j.at("mode")), boolean(j.at("active"))};
    if (s.cursor > s.preedit.size() ||
        (s.cursor < s.preedit.size() &&
         (static_cast<unsigned char>(s.preedit[s.cursor]) & 0xc0) == 0x80) ||
        (s.mode != "chinese" && s.mode != "latin")) { invalid(); }
    const auto &candidates = j.at("candidates");
    if (!candidates.is_array() || candidates.size() > INT_MAX) { invalid(); }
    for (const auto &c : candidates) {
        fields(c, {"text", "start", "end"});
        Candidate candidate{text(c.at("text")), index(c.at("start")), index(c.at("end"))};
        // Candidate ranges are syllable positions, NOT text byte offsets.
        if (candidate.text.empty() || candidate.start >= candidate.end || candidate.end > 64) {
            invalid();
        }
        s.candidates.push_back(std::move(candidate));
    }
    if (s.candidates.empty()) {
        if (s.selecting || s.page || s.candidateIndex) { invalid(); }
    } else if (s.page > (s.candidates.size() - 1) / PageSize ||
               s.candidateIndex >= s.candidates.size() ||
               (s.selecting && s.candidateIndex / PageSize != s.page)) { invalid(); }
    return s;
}

Output decodeOutput(const Json &j) {
    fields(j, {"consumed", "snapshot", "commit", "status"});
    Output out{boolean(j.at("consumed")), decodeSnapshot(j.at("snapshot")), {}, {}};
    if (!j.at("commit").is_null()) { out.commit = text(j.at("commit")); }
    if (!j.at("status").is_null()) {
        out.status = text(j.at("status"));
        if (*out.status != "invalid_syllable" && *out.status != "unknown_reading" &&
            *out.status != "limit_reached" && *out.status != "stale_selection" &&
            *out.status != "internal_error") { invalid(); }
    }
    return out;
}

Json keyEvent(const std::string &key) {
    return {{"type", "key"}, {"key", key}, {"ctrl", false}, {"alt", false},
            {"meta", false}, {"shift", false}, {"repeat", false}, {"release", false}};
}

CoreClient::CoreClient(const std::string &lexicon) {
    if (bpm_abi_version() != BPM_ABI_VERSION) { throw std::runtime_error("C ABI version mismatch"); }
    BpmEngine *raw = nullptr;
    const auto status = bpm_engine_new(reinterpret_cast<const uint8_t *>(lexicon.data()),
                                       lexicon.size(), &raw);
    engine_.reset(raw);
    if (status != BPM_OK || !engine_) { throw std::runtime_error("Core initialization failed"); }
    // Validate the initial output before exposing the engine to a host.
    snapshot();
}

Output CoreClient::handle(const Json &event) {
    try {
        if (!engine_) { invalid(); }
        const auto input = event.dump();
        if (input.size() > 4096) { invalid(); }
        Buffer output;
        const auto status = bpm_engine_handle(engine_.get(),
            reinterpret_cast<const uint8_t *>(input.data()), input.size(), &output.value);
        if (status != BPM_OK) { throw std::runtime_error("Core event failed"); }
        auto decoded = decodeOutput(output.parse());
        if (decoded.status && *decoded.status == "internal_error") {
            throw std::runtime_error("Core internal failure");
        }
        return decoded;
    } catch (...) {
        engine_.reset(); // No retry: an event can have produced a commit.
        throw;
    }
}

Snapshot CoreClient::snapshot() {
    try {
        if (!engine_) { invalid(); }
        Buffer output;
        if (bpm_engine_snapshot(engine_.get(), &output.value) != BPM_OK) {
            throw std::runtime_error("Core snapshot failed");
        }
        return decodeSnapshot(output.parse());
    } catch (...) {
        engine_.reset();
        throw;
    }
}
} // namespace bopomofo
