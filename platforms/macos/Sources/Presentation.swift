import Foundation

struct CandidateToken: Equatable {
    let publication: UUID
    let generation: UInt64
    let index: Int
}

struct PresentedCandidate {
    let text: String
    let token: CandidateToken
}

enum TextEffect: Equatable {
    case commit(String)
    case mark(String, NSRange)
}

struct Presentation {
    let snapshot: Snapshot
    let selection: NSRange
    let candidates: [PresentedCandidate]
    let effects: [TextEffect]
}

// Pure state: each event delivery can produce host effects once only.
struct PresentationState {
    private(set) var snapshot: Snapshot?
    private var lastDelivery: UInt64 = 0
    private var publication = UUID()
    private var hasMarkedText = false

    mutating func accept(_ output: EngineOutput, delivery: UInt64) throws -> Presentation? {
        try output.validateStatus()
        guard delivery > lastDelivery else { return nil }
        let next = output.snapshot
        let cursor = try TextRange.utf16Offset(in: next.preedit, utf8Offset: next.cursor)
        guard next.page >= 0, next.candidateIndex >= 0, next.candidates.count <= 256,
              next.mode == "chinese" || next.mode == "latin",
              next.candidates.allSatisfy({ !$0.text.isEmpty && $0.start >= 0 && $0.end > $0.start && $0.end <= 64 }) else {
            throw AdapterError.invalidSnapshot
        }
        let (pageStart, overflow) = next.page.multipliedReportingOverflow(by: 9)
        guard !overflow else { throw AdapterError.invalidSnapshot }
        if next.selecting {
            guard next.active, next.candidateIndex / 9 == next.page,
                  next.candidates.indices.contains(next.candidateIndex),
                  next.candidates.indices.contains(pageStart) else { throw AdapterError.invalidSnapshot }
        }
        lastDelivery = delivery
        if snapshot == next && output.commit == nil { return nil }
        let previous = snapshot
        publication = UUID()
        snapshot = next
        var items: [PresentedCandidate] = []
        if next.active && next.selecting {
            // PAGE_SIZE=9 is the shared snapshot contract, not an input rule.
            let count = min(9, next.candidates.count - pageStart)
            for index in pageStart..<(pageStart + count) {
                items.append(PresentedCandidate(text: next.candidates[index].text,
                    token: CandidateToken(publication: publication, generation: next.generation, index: index)))
            }
        }
        let selection = NSRange(location: cursor, length: 0)
        var effects: [TextEffect] = []
        if let commit = output.commit {
            effects.append(.commit(commit))
            hasMarkedText = false
        }
        if next.preedit.isEmpty {
            if hasMarkedText { effects.append(.mark("", selection)) }
            hasMarkedText = false
        } else if !hasMarkedText || previous?.preedit != next.preedit || previous?.cursor != next.cursor {
            effects.append(.mark(next.preedit, selection))
            hasMarkedText = true
        }
        return Presentation(snapshot: next, selection: selection, candidates: items, effects: effects)
    }

    mutating func select(_ token: CandidateToken, text: String) -> EngineEvent? {
        guard let snapshot, snapshot.active, snapshot.selecting,
              token.publication == publication, token.generation == snapshot.generation,
              snapshot.candidates.indices.contains(token.index),
              snapshot.candidates[token.index].text == text else { return nil }
        // Invalidate before calling the engine or native client (which can re-enter).
        publication = UUID()
        return .select(generation: token.generation, index: token.index)
    }

    mutating func invalidate() {
        publication = UUID()
        snapshot = nil
    }

    mutating func cancel() -> [TextEffect] {
        invalidate()
        guard hasMarkedText else { return [] }
        hasMarkedText = false
        return [.mark("", NSRange(location: 0, length: 0))]
    }
}

// Keep an opaque publication ID on each native attributed candidate. Never fall
// back to matching text: a stale list can contain exactly the same strings.
enum CandidateAttributes {
    private static let publication = NSAttributedString.Key("org.bopomofo.development.publication")
    private static let generation = NSAttributedString.Key("org.bopomofo.development.generation")
    private static let index = NSAttributedString.Key("org.bopomofo.development.index")

    static func string(_ candidate: PresentedCandidate) -> NSAttributedString {
        NSAttributedString(string: candidate.text, attributes: [
            publication: candidate.token.publication.uuidString,
            generation: String(candidate.token.generation),
            index: NSNumber(value: candidate.token.index),
        ])
    }

    static func token(_ candidate: NSAttributedString) -> CandidateToken? {
        guard candidate.length > 0,
              let id = candidate.attribute(publication, at: 0, effectiveRange: nil) as? String,
              let uuid = UUID(uuidString: id),
              let value = candidate.attribute(generation, at: 0, effectiveRange: nil) as? String,
              let version = UInt64(value),
              let number = candidate.attribute(index, at: 0, effectiveRange: nil) as? NSNumber,
              number.int64Value >= 0 else { return nil }
        return CandidateToken(publication: uuid, generation: version, index: number.intValue)
    }
}
