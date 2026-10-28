import AppKit
import InputMethodKit

@objc(BopomofoInputController)
final class BopomofoInputController: IMKInputController {
    private var engine: CoreEngine?
    private var state = PresentationState()
    private var translator = KeyTranslator()
    private var panel: IMKCandidates?
    private var candidateData: [NSAttributedString] = []
    private var handling = false
    private var delivery: UInt64 = 0

    override init!(server: IMKServer!, delegate: Any!, client inputClient: Any!) {
        super.init(server: server, delegate: delegate, client: inputClient)
        panel = IMKCandidates(server: server, panelType: kIMKSingleColumnScrollingCandidatePanel)
        panel?.setAttributes([IMKCandidatesSendServerKeyEventFirst: NSNumber(value: true)])
        panel?.setSelectionKeys([18, 19, 20, 21, 23, 22, 26, 28, 25].map { NSNumber(value: $0) })
        createEngine()
    }

    override func recognizedEvents(_ sender: Any!) -> Int {
        let mask: NSEvent.EventTypeMask = [.keyDown, .keyUp, .flagsChanged,
            .leftMouseDown, .rightMouseDown, .otherMouseDown]
        return Int(mask.rawValue)
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event else { return false }
        let kind: NativeKeyKind
        switch event.type {
        case .keyDown: kind = .down
        case .keyUp: kind = .up
        case .flagsChanged: kind = .flags
        case .leftMouseDown, .rightMouseDown, .otherMouseDown:
            // Only events delivered by InputMethodKit. No global event monitor.
            _ = dispatch(.reset)
            return false
        default: return false
        }
        let input = NativeKey(kind: kind, code: event.keyCode,
            characters: kind == .flags ? nil : event.charactersIgnoringModifiers,
            modifiers: modifiers(event.modifierFlags), isRepeat: kind == .down && event.isARepeat)
        guard let key = translator.translate(input) else { return false }
        return dispatch(.key(key))
    }

    override func activateServer(_ sender: Any!) {
        state.invalidate()
        candidateData = []
        translator.reset(modifiers: modifiers(NSEvent.modifierFlags))
        if engine == nil { createEngine() }
        _ = dispatch(.activate)
    }

    override func deactivateServer(_ sender: Any!) {
        _ = dispatch(.deactivate)
        state.invalidate()
        candidateData = []
        panel?.hide()
        translator.reset()
    }

    // Host termination of a composition is cancellation, not a hidden Enter.
    override func commitComposition(_ sender: Any!) { _ = dispatch(.reset) }
    override func cancelComposition() { _ = dispatch(.reset) }

    override func inputControllerWillClose() {
        _ = dispatch(.focusLost)
        state.invalidate()
        panel?.hide()
        engine = nil
        super.inputControllerWillClose()
    }

    override func composedString(_ sender: Any!) -> Any! { state.snapshot?.preedit ?? "" }
    override func candidates(_ sender: Any!) -> [Any]! { candidateData }

    override func candidateSelectionChanged(_ candidateString: NSAttributedString!) {
        // Native highlight is not a final choice. Never commit or select on hover.
    }

    override func candidateSelected(_ candidateString: NSAttributedString!) {
        guard !handling, let candidateString,
              let token = CandidateAttributes.token(candidateString),
              let event = state.select(token, text: candidateString.string) else { return }
        _ = dispatch(event)
    }

    private func modifiers(_ flags: NSEvent.ModifierFlags) -> NativeModifiers {
        NativeModifiers(ctrl: flags.contains(.control), alt: flags.contains(.option),
            meta: flags.contains(.command), shift: flags.contains(.shift), capsLock: flags.contains(.capsLock),
            function: flags.contains(.function))
    }

    private func createEngine() {
        do {
            engine = try CoreEngine(lexicon: LexiconResource.result.get())
        } catch { reportError(error) }
    }

    @discardableResult private func dispatch(_ event: EngineEvent) -> Bool {
        // IMK callbacks for one context are serialized on the main run loop.
        guard Thread.isMainThread, !handling, let engine else { return false }
        handling = true
        defer { handling = false }
        do {
            delivery += 1
            let output = try engine.send(event)
            if let presentation = try state.accept(output, delivery: delivery) { apply(presentation) }
            return output.consumed
        } catch {
            reportError(error)
            // Never retry an event: it might already have produced a commit.
            self.engine = nil
            candidateData = []
            panel?.hide()
            applyTextEffects(state.cancel())
            return false
        }
    }

    private func apply(_ presentation: Presentation) {
        let snapshot = presentation.snapshot
        applyTextEffects(presentation.effects)
        candidateData = presentation.candidates.map(CandidateAttributes.string)
        if !candidateData.isEmpty {
            panel?.update()
            if let localIndex = presentation.candidates.firstIndex(where: { $0.token.index == snapshot.candidateIndex }),
               let identifier = panel?.candidateStringIdentifier(candidateData[localIndex]) {
                _ = panel?.selectCandidate(withIdentifier: identifier)
            }
            panel?.show(kIMKLocateCandidatesBelowHint)
        } else { panel?.hide() }
    }

    private func applyTextEffects(_ effects: [TextEffect]) {
        let replacement = NSRange(location: NSNotFound, length: NSNotFound)
        for effect in effects {
            switch effect {
            case .commit(let text):
                client()?.insertText(text, replacementRange: replacement)
            case .mark(let text, let selection):
                let marked = NSAttributedString(string: text,
                    attributes: [.underlineStyle: NSUnderlineStyle.single.rawValue])
                client()?.setMarkedText(marked, selectionRange: selection, replacementRange: replacement)
            }
        }
    }
}
