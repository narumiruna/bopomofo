import Foundation
import CryptoKit

private enum TestFailure: Error { case contract }
private var assertions = 0
private func require(_ condition: @autoclosure () throws -> Bool) throws {
    assertions += 1
    guard try condition() else { throw TestFailure.contract }
}
private func manifest(_ data: Data) -> Data {
    let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    return Data((digest + "\n").utf8)
}
private func keys(_ names: [String], _ engine: CoreEngine) throws -> EngineOutput {
    var result = try engine.send(.reset)
    for name in names {
        result = try engine.send(.key(KeyEvent(key: name)))
        _ = try engine.send(.key(KeyEvent(key: name, release: true)))
    }
    return result
}

@main
struct FFITests {
    static func main() {
        do {
            guard CommandLine.arguments.count == 5 else { throw TestFailure.contract }
            let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
            let pinned = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))
            let lexicon = try VerifiedLexicon(data: data, manifest: pinned)
            try require(pinned == manifest(data))
            for badManifest in [Data(), Data(repeating: 0x30, count: 65), Data(pinned.dropLast()),
                                Data((String(repeating: "0", count: 64) + "\n").utf8)] {
                do {
                    _ = try VerifiedLexicon(data: data, manifest: badManifest)
                    throw TestFailure.contract
                } catch AdapterError.integrity { assertions += 1 }
            }
            do {
                _ = try VerifiedLexicon(data: data + Data("\n".utf8), manifest: pinned)
                throw TestFailure.contract
            } catch AdapterError.integrity { assertions += 1 }
            for _ in 0..<32 {
                let engine = try CoreEngine(lexicon: lexicon)
                try require(try engine.snapshot().preedit.isEmpty)
            }
            let first = try CoreEngine(lexicon: lexicon)
            let second = try CoreEngine(lexicon: lexicon)
            let phrase = try keys(["s", "u", "3", "c", "l", "3"], first)
            try require(phrase.snapshot.preedit == "你好" && phrase.commit == nil)
            try require(try second.snapshot().preedit.isEmpty)
            let committed = try first.send(.key(KeyEvent(key: "enter")))
            try require(committed.commit == "你好" && committed.snapshot.preedit.isEmpty)
            try require(try first.snapshot().preedit.isEmpty)
            for event in [EngineEvent.reset, .reset, .focusLost, .deactivate, .activate] {
                let output = try first.send(event)
                try require(output.commit == nil && output.snapshot.preedit.isEmpty)
            }

            for event in [EngineEvent.reset, .focusLost, .deactivate] {
                let lifecycleEngine = try CoreEngine(lexicon: lexicon)
                let open = try keys(["1", "8", "space", "down"], lifecycleEngine)
                var lifecycleState = PresentationState()
                let before = try lifecycleState.accept(open, delivery: 1)!
                let cancelled = try lifecycleEngine.send(event)
                try require(cancelled.commit == nil && cancelled.snapshot.preedit.isEmpty &&
                    !cancelled.snapshot.selecting && cancelled.snapshot.candidates.isEmpty)
                try require(try lifecycleState.accept(cancelled, delivery: 2)!.effects ==
                    [.mark("", NSRange(location: 0, length: 0))])
                let old = before.candidates[0]
                try require(lifecycleState.select(old.token, text: old.text) == nil)
                let repeated = try lifecycleEngine.send(event)
                try require(repeated.commit == nil && repeated.snapshot.preedit.isEmpty)
                let activated = try lifecycleEngine.send(.activate)
                try require(activated.commit == nil && activated.snapshot.active && activated.snapshot.preedit.isEmpty)
            }

            let choices = try keys(["1", "8", "space", "down"], first)
            try require(choices.snapshot.selecting && choices.snapshot.candidates.count == 12)
            var state = PresentationState()
            let page = try state.accept(choices, delivery: 1)!
            let callback = CandidateAttributes.string(page.candidates[1])
            let token = CandidateAttributes.token(callback)!
            let event = state.select(token, text: callback.string)!
            let selected = try first.send(event)
            try require(selected.commit == nil && selected.snapshot.preedit == "巴" && !selected.snapshot.selecting)
            let stale = try first.send(event)
            try require(stale.status == "stale_selection" && stale.commit == nil && stale.snapshot == selected.snapshot)
            try require(try first.send(.key(KeyEvent(key: "enter"))).commit == "巴")
            try require(try first.send(.key(KeyEvent(key: "enter", repeat: true))).commit == nil)
            let later = try keys(["1", "8", "space", "down", "page_down"], first)
            let nextPage = try state.accept(later, delivery: 2)!
            try require(nextPage.candidates.count == 3 && nextPage.candidates.first?.token.index == 9)
            let chosen = nextPage.candidates[0]
            let selection = state.select(chosen.token, text: chosen.text)!
            try require(try first.send(selection).snapshot.preedit == "豝")
            let cancelled = try first.send(.reset)
            try require(cancelled.commit == nil && cancelled.snapshot.preedit.isEmpty)
            try require(try state.accept(cancelled, delivery: 3)!.effects == [.mark("", NSRange(location: 0, length: 0))])
            try require(state.select(chosen.token, text: chosen.text) == nil)

            let unicode = "𠀀😀e\u{301}"
            let custom = Data("# bopomofo-lexicon-v1\nㄅㄚ\t\(unicode)\t1\n".utf8)
            let unicodeEngine = try CoreEngine(lexicon: VerifiedLexicon(data: custom, manifest: manifest(custom)))
            let unicodeOutput = try keys(["1", "8", "space"], unicodeEngine)
            try require(unicodeOutput.snapshot.preedit == unicode)
            var unicodeState = PresentationState()
            try require(try unicodeState.accept(unicodeOutput, delivery: 1)!.selection.location == 6)
            let unicodeCommit = try unicodeEngine.send(.key(KeyEvent(key: "enter")))
            try require(try unicodeState.accept(unicodeCommit, delivery: 2)!.effects == [.commit(unicode)])
            try require(try unicodeState.accept(unicodeCommit, delivery: 2) == nil)

            let modifierEngine = try CoreEngine(lexicon: lexicon)
            var translator = KeyTranslator()
            func forward(_ input: NativeKey) throws -> EngineOutput? {
                guard let key = translator.translate(input) else { return nil }
                return try modifierEngine.send(.key(key))
            }
            _ = try forward(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
            _ = try forward(NativeKey(kind: .down, code: 0, characters: "A", modifiers: NativeModifiers(shift: true)))
            _ = try forward(NativeKey(kind: .up, code: 0, characters: "A", modifiers: NativeModifiers(shift: true)))
            let chordEnd = try forward(NativeKey(kind: .flags, code: 56))!
            try require(chordEnd.snapshot.mode == "chinese")
            _ = try modifierEngine.send(.reset)
            for code: UInt16 in [59, 58, 55, 63] {
                _ = try forward(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
                let flags = NativeModifiers(ctrl: code == 59, alt: code == 58, meta: code == 55,
                    shift: true, function: code == 63)
                let down = try forward(NativeKey(kind: .flags, code: code, modifiers: flags))!
                let up = try forward(NativeKey(kind: .flags, code: code, modifiers: NativeModifiers(shift: true)))!
                try require(!down.consumed && !up.consumed)
                try require(try forward(NativeKey(kind: .flags, code: 56))!.snapshot.mode == "chinese")
            }
            _ = try forward(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
            try require(try forward(NativeKey(kind: .flags, code: 56))!.snapshot.mode == "latin")
            try require(try modifierEngine.send(.key(KeyEvent(key: "1"))).consumed == false)
            try require(try forward(NativeKey(kind: .flags, code: 57, modifiers: NativeModifiers(capsLock: true)))!.snapshot.mode == "chinese")
            try require(try forward(NativeKey(kind: .flags, code: 57))!.snapshot.mode == "latin")

            let heldEngine = try CoreEngine(lexicon: lexicon)
            var heldTranslator = KeyTranslator()
            func forwardHeld(_ input: NativeKey) throws -> EngineOutput {
                try heldEngine.send(.key(heldTranslator.translate(input)!))
            }
            _ = try forwardHeld(NativeKey(kind: .down, code: 18, characters: "1"))
            _ = try forwardHeld(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
            try require(try forwardHeld(NativeKey(kind: .flags, code: 56)).snapshot.mode == "chinese")
            try require(try forwardHeld(NativeKey(kind: .up, code: 18, characters: "1")).consumed == false)
            _ = try forwardHeld(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
            try require(try forwardHeld(NativeKey(kind: .flags, code: 56)).snapshot.mode == "latin")

            let aliasEngine = try CoreEngine(lexicon: lexicon)
            var aliasTranslator = KeyTranslator()
            func alias(_ input: NativeKey) throws -> EngineOutput {
                try aliasEngine.send(.key(aliasTranslator.translate(input)!))
            }
            _ = try alias(NativeKey(kind: .down, code: 36))
            _ = try alias(NativeKey(kind: .down, code: 76))
            _ = try alias(NativeKey(kind: .up, code: 36))
            _ = try alias(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
            try require(try alias(NativeKey(kind: .flags, code: 56)).snapshot.mode == "chinese")
            _ = try alias(NativeKey(kind: .up, code: 76))
            _ = try alias(NativeKey(kind: .flags, code: 56, modifiers: NativeModifiers(shift: true)))
            try require(try alias(NativeKey(kind: .flags, code: 56)).snapshot.mode == "latin")

            let shortcutEngine = try CoreEngine(lexicon: lexicon)
            let pending = try keys(["1"], shortcutEngine)
            let shortcut = try shortcutEngine.send(.key(KeyEvent(key: "a", meta: true)))
            try require(!shortcut.consumed && shortcut.snapshot == pending.snapshot)
            do {
                let invalid = Data("invalid".utf8)
                _ = try CoreEngine(lexicon: VerifiedLexicon(data: invalid, manifest: manifest(invalid)))
                throw TestFailure.contract
            } catch AdapterError.initialization { assertions += 1 }
            let cnsPath = URL(fileURLWithPath: CommandLine.arguments[3])
            let cnsData = try Data(contentsOf: cnsPath)
            let cnsManifest = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[4]))
            let cnsEngine = try CoreEngine(lexicon: VerifiedLexicon(data: cnsData, manifest: cnsManifest))
            for (sequence, text) in [
                (["j", "i", "3"], "我"), (["e", "j", "i", "6"], "國"),
                (["q", "u", "8", "space"], "啪"), (["z", "u", "l", "4"], "覅"),
                (["s", "j", "o", "6"], "捼"), (["x", "m", "0", "6"], "孿"),
                (["x", "m", "0", "3"], "孌"), (["n", "o", "space"], "塞")
            ] {
                let composed = try keys(sequence, cnsEngine)
                try require(composed.status == nil && composed.snapshot.preedit == text)
                try require(try cnsEngine.send(.key(KeyEvent(key: "enter"))).commit == text)
            }
            let all = try keys(["u", "4", "down"], cnsEngine)
            try require(all.snapshot.candidates.count == 154 && all.snapshot.selecting)
            var cnsState = PresentationState()
            _ = try cnsState.accept(all, delivery: 1)
            let tail = try keys(["u", "4", "down"] + Array(repeating: "page_down", count: 17), cnsEngine)
            let tailPage = try cnsState.accept(tail, delivery: 2)!
            try require(tailPage.candidates.count == 1 && tailPage.candidates[0].token.index == 153)
            let tailCandidate = tailPage.candidates[0]
            try require(tailCandidate.text == "齸")
            let tailEvent = cnsState.select(tailCandidate.token, text: tailCandidate.text)!
            try require(try cnsEngine.send(tailEvent).snapshot.preedit == "齸")
            try require(try cnsEngine.send(.key(KeyEvent(key: "enter"))).commit == "齸")
            try require(try cnsEngine.send(.key(KeyEvent(key: "enter"))).commit == nil)
            try require(cnsState.select(tailCandidate.token, text: tailCandidate.text) == nil)
            for name in ["Lexicon-NOTICE.txt", "Lexicon-LICENSE.txt", "Project-LICENSE.txt"] {
                try require(try Data(contentsOf: cnsPath.deletingLastPathComponent().appendingPathComponent(name)).count > 0)
            }
            print("Swift C ABI integration: \(assertions) assertions passed")
        } catch {
            // Do not print input, output JSON, or error descriptions.
            print("Swift C ABI integration: FAILED (contract)")
            exit(1)
        }
    }
}
