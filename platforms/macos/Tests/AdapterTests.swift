import Foundation

private struct Failure: Error { let name: String }
private var assertions = 0

private func check(_ condition: @autoclosure () throws -> Bool, _ name: String) throws {
    assertions += 1
    if try !condition() { throw Failure(name: name) }
}

private func rejects(_ name: String, _ body: () throws -> Void) throws {
    do { try body() } catch is AdapterError { assertions += 1; return }
    throw Failure(name: name)
}

private func snapshot(_ text: String = "", generation: UInt64 = 1, candidates: [Candidate] = [],
                      index: Int = 0, page: Int = 0, active: Bool = true) -> Snapshot {
    Snapshot(preedit: text, cursor: text.utf8.count, candidates: candidates, page: page,
        candidateIndex: index, selecting: !candidates.isEmpty, generation: generation, mode: "chinese", active: active)
}

private func output(_ value: Snapshot, commit: String? = nil) -> EngineOutput {
    EngineOutput(consumed: true, snapshot: value, commit: commit, status: nil)
}

private func wireTests() throws {
    let key = KeyEvent(key: "1", ctrl: true, alt: true, meta: true, shift: true, repeat: true, release: true)
    let data = try JSONEncoder().encode(EngineEvent.key(key))
    let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    try check(Set(json.keys) == Set(["type", "key", "ctrl", "alt", "meta", "shift", "repeat", "release"]), "flat key schema")
    try check(json["type"] as? String == "key", "key tag")
    for name in ["ctrl", "alt", "meta", "shift", "repeat", "release"] {
        try check(json[name] as? Bool == true, "key boolean field")
    }
    let events: [(EngineEvent, String)] = [(.activate, "activate"), (.deactivate, "deactivate"),
        (.focusLost, "focus_lost"), (.reset, "reset")]
    for (event, tag) in events {
        let fields = try JSONSerialization.jsonObject(with: JSONEncoder().encode(event)) as! [String: Any]
        try check(fields["type"] as? String == tag && fields.count == 1, "lifecycle encoding")
    }
    let selection = try JSONEncoder().encode(EngineEvent.select(generation: UInt64.max, index: 8))
    let fields = try JSONSerialization.jsonObject(with: selection) as! [String: Any]
    try check((fields["generation"] as? NSNumber)?.stringValue == String(UInt64.max), "uint64 selection precision")
    let value = snapshot("𠀀😀e\u{301}", generation: UInt64.max)
    let roundtrip = try JSONDecoder().decode(Snapshot.self, from: JSONEncoder().encode(value))
    try check(value == roundtrip, "snapshot UTF and uint64 roundtrip")
    let outputJSON = """
    {"consumed":true,"snapshot":\(String(data: try JSONEncoder().encode(value), encoding: .utf8)!),"commit":null,"status":"limit_reached"}
    """
    let result = try JSONDecoder().decode(EngineOutput.self, from: Data(outputJSON.utf8))
    try check(result.commit == nil && result.status == "limit_reached", "optional output fields")
}

private func rangeTests() throws {
    let text = "A𠀀😀e\u{301}👩‍💻Z"
    let positions = [0: 0, 1: 1, 5: 3, 9: 5, 10: 6, 12: 7, 16: 9, 19: 10, 23: 12, 24: 13]
    for byte in 0...text.utf8.count {
        if let expected = positions[byte] {
            try check(try TextRange.utf16Offset(in: text, utf8Offset: byte) == expected, "scalar boundary conversion")
        } else {
            try rejects("interior UTF8 byte") { _ = try TextRange.utf16Offset(in: text, utf8Offset: byte) }
        }
    }
    try check(try TextRange.utf16Range(in: text, start: 5, end: 9) == NSRange(location: 3, length: 2), "emoji surrogate pair")
    try check(try TextRange.utf16Range(in: text, start: 9, end: 12) == NSRange(location: 5, length: 2), "combining sequence")
    try check(try TextRange.utf16Range(in: text, start: 12, end: 23) == NSRange(location: 7, length: 5), "ZWJ sequence")
    try check(try TextRange.utf16Offset(in: "", utf8Offset: 0) == 0, "empty UTF range")
    for invalid in [-1, 25, Int.max] {
        try rejects("out of range offset") { _ = try TextRange.utf16Offset(in: text, utf8Offset: invalid) }
    }
    try rejects("reversed range") { _ = try TextRange.utf16Range(in: text, start: 12, end: 9) }
}

private struct Replay: Decodable { let name: String; let steps: [Step] }
private struct Step: Decodable {
    let kind: NativeKeyKind
    let code: UInt16
    let characters: String?
    let flags: [String]
    let isRepeat: Bool?
    let expected: Expected?
}
private struct Expected: Decodable {
    let key: String
    let ctrl: Bool?
    let alt: Bool?
    let meta: Bool?
    let shift: Bool?
    let `repeat`: Bool?
    let release: Bool?
}

private func keyTests(_ path: String) throws {
    let cases = try JSONDecoder().decode([Replay].self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    for replay in cases {
        var translator = KeyTranslator()
        for step in replay.steps {
            let flags = NativeModifiers(ctrl: step.flags.contains("ctrl"), alt: step.flags.contains("alt"),
                meta: step.flags.contains("meta"), shift: step.flags.contains("shift"), capsLock: step.flags.contains("caps_lock"),
                function: step.flags.contains("fn"))
            let actual = translator.translate(NativeKey(kind: step.kind, code: step.code,
                characters: step.characters, modifiers: flags, isRepeat: step.isRepeat ?? false))
            let expected = step.expected.map { KeyEvent(key: $0.key, ctrl: $0.ctrl ?? false, alt: $0.alt ?? false,
                meta: $0.meta ?? false, shift: $0.shift ?? false, repeat: $0.repeat ?? false, release: $0.release ?? false) }
            try check(actual == expected, replay.name)
        }
    }
    var translator = KeyTranslator()
    translator.reset(modifiers: NativeModifiers(shift: true))
    let release = translator.translate(NativeKey(kind: .flags, code: 56))
    try check(release?.release == true, "activation with already held modifier")
    translator.reset()
    try check(translator.translate(NativeKey(kind: .down, code: 49))?.key == "space", "space key normalization")
    try check(translator.translate(NativeKey(kind: .down, code: 82, characters: "1"))?.key == "native_82", "unmapped keypad cannot alias Dachen")
    try check(translator.translate(NativeKey(kind: .up, code: 82, characters: nil))?.key == "native_82", "unmapped physical release is stable")
}

private func presentationTests() throws {
    let candidates = [Candidate(text: "𠀀", start: 0, end: 1), Candidate(text: "😀", start: 0, end: 1),
        Candidate(text: "e\u{301}", start: 0, end: 1)]
    var state = PresentationState()
    let first = try state.accept(output(snapshot("𠀀😀e\u{301}", generation: UInt64.max,
        candidates: candidates, index: 1)), delivery: 1)!
    try check(first.selection == NSRange(location: 6, length: 0), "native cursor uses UTF16")
    try check(first.effects == [.mark("𠀀😀e\u{301}", first.selection)], "native mark effect")
    try check(try state.accept(output(first.snapshot), delivery: 2) == nil, "unchanged snapshot does not update native UI")
    let attributed = CandidateAttributes.string(first.candidates[1])
    let token = CandidateAttributes.token(attributed)!
    try check(token == first.candidates[1].token && token.generation == UInt64.max, "candidate token roundtrip")
    try check(CandidateAttributes.token(attributed.copy() as! NSAttributedString) == token, "copied candidate retains token")
    try check(CandidateAttributes.token(NSAttributedString(string: attributed.string)) == nil, "stripped candidate attributes rejected")
    try check(CandidateAttributes.token(NSAttributedString(string: "")) == nil, "empty candidate rejected")
    let second = try state.accept(output(snapshot("𠀀", generation: UInt64.max,
        candidates: candidates, index: 1)), delivery: 3)!
    try check(state.select(token, text: attributed.string) == nil, "same generation old publication rejected")
    let fresh = second.candidates[1].token
    try check(state.select(fresh, text: "different") == nil, "modified callback rejected")
    try check(state.select(CandidateToken(publication: fresh.publication, generation: 1, index: 1), text: "😀") == nil, "old generation rejected")
    try check(state.select(CandidateToken(publication: fresh.publication, generation: fresh.generation, index: -1), text: "😀") == nil, "negative index rejected")
    try check(state.select(fresh, text: "😀") == .select(generation: UInt64.max, index: 1), "final candidate selection")
    try check(state.select(fresh, text: "😀") == nil, "duplicate callback rejected")
    state.invalidate()
    try check(state.select(fresh, text: "😀") == nil, "focus invalidation")
    try check(state.cancel() == [.mark("", NSRange(location: 0, length: 0))], "failure cancellation clears mark")
    try check(state.cancel().isEmpty, "duplicate failure cancellation")

    let many = (0..<20).map { Candidate(text: "candidate-\($0)", start: 0, end: 1) }
    let page = try state.accept(output(snapshot("𠀀", candidates: many, index: 11, page: 1)), delivery: 4)!
    try check(page.candidates.count == 9 && page.candidates.first?.token.index == 9 && page.candidates.last?.token.index == 17, "page absolute indices")
    let lastPage = try state.accept(output(snapshot("𠀀", candidates: many, index: 19, page: 2)), delivery: 5)!
    try check(lastPage.candidates.count == 2, "last partial page")
    try rejects("overflow page") { _ = try state.accept(output(snapshot("", page: Int.max)), delivery: 6) }
    let invalid = Snapshot(preedit: "𠀀", cursor: 1, candidates: [], page: 0, candidateIndex: 0,
        selecting: false, generation: 1, mode: "chinese", active: true)
    try rejects("invalid snapshot cursor") { _ = try state.accept(output(invalid, commit: "never"), delivery: 6) }
    try check(state.snapshot?.generation == lastPage.snapshot.generation, "invalid output leaves state unchanged")

    let commit = output(snapshot(), commit: "𠀀😀e\u{301}")
    let committed = try state.accept(commit, delivery: 6)!
    try check(committed.effects == [.commit("𠀀😀e\u{301}")], "commit replaces marked text without pre-clear")
    try check(try state.accept(commit, delivery: 6) == nil, "duplicate delivery does not commit")
    try check(try state.accept(commit, delivery: 5) == nil, "out of order delivery does not commit")
    state.invalidate()
    try check(try state.accept(commit, delivery: 6) == nil, "reset never replays commit")
    for receipt in 7...9 {
        let reset = try state.accept(output(snapshot()), delivery: UInt64(receipt))
        try check(reset?.effects.isEmpty ?? true, "repeated reset has no host effects")
    }
    let sameTextNewEvent = try state.accept(commit, delivery: 10)!
    try check(sameTextNewEvent.effects.count == 1, "new event may commit equal text")
    let mark = try state.accept(output(snapshot("𠀀")), delivery: 11)!
    try check(mark.effects.count == 1, "new composition mark")
    let reset = try state.accept(output(snapshot()), delivery: 12)!
    try check(reset.effects == [.mark("", NSRange(location: 0, length: 0))], "reset clears preedit without commit")
    let beforeError = state.snapshot
    let failed = EngineOutput(consumed: true, snapshot: snapshot("never"), commit: "never", status: "internal_error")
    try rejects("internal engine error rejects all host effects") { _ = try state.accept(failed, delivery: 13) }
    try check(state.snapshot == beforeError, "failed output preserves presentation until cancellation")
    let unknown = EngineOutput(consumed: true, snapshot: snapshot(), commit: nil, status: "unknown_status")
    try rejects("unknown output status") { _ = try state.accept(unknown, delivery: 13) }
    let full = (0..<256).map { Candidate(text: "candidate-\($0)", start: 0, end: 1) }
    let fullPage = try state.accept(output(snapshot("字", candidates: full, index: 255, page: 28)), delivery: 13)!
    try check(fullPage.candidates.count == 4 && fullPage.candidates.last?.token.index == 255,
        "all 256 core candidates remain reachable")
    try rejects("candidate count exceeds core limit") {
        _ = try state.accept(output(snapshot("字", candidates: full + [Candidate(text: "extra", start: 0, end: 1)])), delivery: 14)
    }
    let badSpan = snapshot("x", candidates: [Candidate(text: "x", start: 0, end: 0)])
    try rejects("candidate spans are nonempty syllable ranges") { _ = try state.accept(output(badSpan), delivery: 14) }
}

@main
struct AdapterTests {
    static func main() {
        do {
            guard CommandLine.arguments.count == 2 else { throw Failure(name: "fixture argument") }
            try wireTests()
            try rangeTests()
            try keyTests(CommandLine.arguments[1])
            try presentationTests()
            print("Swift adapter logic: \(assertions) assertions passed")
        } catch let failure as Failure {
            print("Swift adapter logic: FAILED (\(failure.name))")
            exit(1)
        } catch {
            print("Swift adapter logic: FAILED (test error)")
            exit(1)
        }
    }
}
