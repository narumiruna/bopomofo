import Foundation

struct KeyEvent: Codable, Equatable {
    var key: String
    var ctrl = false
    var alt = false
    var meta = false
    var shift = false
    var `repeat` = false
    var release = false
}

enum EngineEvent: Encodable, Equatable {
    case key(KeyEvent)
    case activate, deactivate, focusLost, reset
    case select(generation: UInt64, index: Int)

    private enum CodingKeys: String, CodingKey {
        case type, key, ctrl, alt, meta, shift, `repeat`, release, generation, index
    }

    func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .key(let event):
            try values.encode("key", forKey: .type)
            try values.encode(event.key, forKey: .key)
            try values.encode(event.ctrl, forKey: .ctrl)
            try values.encode(event.alt, forKey: .alt)
            try values.encode(event.meta, forKey: .meta)
            try values.encode(event.shift, forKey: .shift)
            try values.encode(event.repeat, forKey: .repeat)
            try values.encode(event.release, forKey: .release)
        case .activate: try values.encode("activate", forKey: .type)
        case .deactivate: try values.encode("deactivate", forKey: .type)
        case .focusLost: try values.encode("focus_lost", forKey: .type)
        case .reset: try values.encode("reset", forKey: .type)
        case .select(let generation, let index):
            try values.encode("select", forKey: .type)
            try values.encode(generation, forKey: .generation)
            try values.encode(index, forKey: .index)
        }
    }
}

struct Candidate: Codable, Equatable {
    let text: String
    let start: Int
    let end: Int
}

struct Snapshot: Codable, Equatable {
    let preedit: String
    let cursor: Int
    let candidates: [Candidate]
    let page: Int
    let candidateIndex: Int
    let selecting: Bool
    let generation: UInt64
    let mode: String
    let active: Bool

    enum CodingKeys: String, CodingKey {
        case preedit, cursor, candidates, page, selecting, generation, mode, active
        case candidateIndex = "candidate_index"
    }
}

struct EngineOutput: Decodable {
    let consumed: Bool
    let snapshot: Snapshot
    let commit: String?
    let status: String?

    func validateStatus() throws {
        switch status {
        case nil, "invalid_syllable", "unknown_reading", "limit_reached", "stale_selection": return
        case "internal_error": throw AdapterError.internalEngine
        default: throw AdapterError.outputDecoding
        }
    }
}

enum AdapterError: Error {
    case abi, initialization, integrity, eventEncoding, ffi, outputDecoding, invalidOffset, invalidSnapshot, internalEngine
}

enum TextRange {
    // Core offsets are UTF-8 scalar boundaries, not grapheme boundaries.
    static func utf16Offset(in text: String, utf8Offset: Int) throws -> Int {
        guard utf8Offset >= 0 else { throw AdapterError.invalidOffset }
        var bytes = 0
        var units = 0
        for scalar in text.unicodeScalars {
            if bytes == utf8Offset { return units }
            bytes += scalar.utf8.count
            units += scalar.value > 0xFFFF ? 2 : 1
        }
        guard bytes == utf8Offset else { throw AdapterError.invalidOffset }
        return units
    }

    static func utf16Range(in text: String, start: Int, end: Int) throws -> NSRange {
        guard end >= start else { throw AdapterError.invalidOffset }
        let lower = try utf16Offset(in: text, utf8Offset: start)
        let upper = try utf16Offset(in: text, utf8Offset: end)
        return NSRange(location: lower, length: upper - lower)
    }
}
