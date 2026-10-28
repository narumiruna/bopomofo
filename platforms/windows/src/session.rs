//! Context-local ordering and panic isolation, independent of COM.
use ime_core::{Engine, Event, KeyEvent, Lexicon, Output, Snapshot};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ticket {
    pub epoch: u64,
    pub sequence: u64,
}

pub(crate) struct Session {
    engine: Engine,
    epoch: u64,
    sequence: u64,
    applied: u64,
}

/// Do not allow a Rust panic to unwind through a COM vtable or DLL export.
/// Errors and panics must not be retried: a native call may already have committed.
pub(crate) fn boundary<T>(call: impl FnOnce() -> T) -> Result<T, ()> {
    catch_unwind(AssertUnwindSafe(call)).map_err(|_| ())
}

impl Session {
    pub fn new(lexicon: Arc<Lexicon>) -> Self {
        let mut engine = Engine::new(lexicon);
        engine.handle(Event::Activate);
        Self {
            engine,
            epoch: 1,
            sequence: 0,
            applied: 0,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.engine.snapshot()
    }

    pub fn probe(&self, key: &KeyEvent) -> bool {
        self.engine.probe(key)
    }

    /// TSF test callbacks request delivery, not final consumption. Core tracks
    /// held keys even on pass-through events, so every actual event is needed.
    pub fn test_delivery(&self, key: &KeyEvent) -> bool {
        let _would_consume = self.probe(key);
        true
    }

    pub fn event(&mut self, event: Event) -> (Ticket, Output) {
        let output = self.engine.handle(event);
        self.sequence += 1;
        (
            Ticket {
                epoch: self.epoch,
                sequence: self.sequence,
            },
            output,
        )
    }

    /// Invalidate pending edits and candidate callbacks before a lifecycle change.
    pub fn invalidate(&mut self, event: Event) -> (Ticket, Output) {
        self.epoch += 1;
        self.sequence = 0;
        self.applied = 0;
        self.event(event)
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Reserve an output once, before calling the host. Never retry partial edits.
    pub fn accept(&mut self, ticket: Ticket) -> bool {
        if ticket.epoch != self.epoch || ticket.sequence != self.applied + 1 {
            return false;
        }
        self.applied = ticket.sequence;
        true
    }

    pub fn selection_is_current(&self, epoch: u64, generation: u64) -> bool {
        epoch == self.epoch && generation == self.engine.snapshot().generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_order_prevents_duplicate_or_reordered_commit() {
        // Ordering does not depend on a lexicon or engine state.
        let mut order = Session::new(Arc::new(Lexicon::parse("# bopomofo-lexicon-v1\n").unwrap()));
        let (first, _) = order.event(Event::Reset);
        let (second, _) = order.event(Event::Reset);
        assert!(!order.accept(second));
        assert!(order.accept(first));
        assert!(!order.accept(first));
        assert!(order.accept(second));
    }

    fn fixture() -> Arc<Lexicon> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lexicon.tsv");
        Arc::new(Lexicon::parse(&std::fs::read_to_string(path).unwrap()).unwrap())
    }

    fn key(name: &str) -> Event {
        Event::Key(KeyEvent {
            key: name.into(),
            ..KeyEvent::default()
        })
    }

    #[test]
    fn probe_is_read_only_and_contexts_are_independent() {
        let lexicon = fixture();
        let mut first = Session::new(lexicon.clone());
        let second = Session::new(lexicon);
        for name in ["1", "8", "space", "down"] {
            first.event(key(name));
        }
        let before = first.snapshot();
        let event = KeyEvent {
            key: "page_down".into(),
            ..KeyEvent::default()
        };
        for _ in 0..20 {
            assert!(first.probe(&event));
        }
        assert_eq!(first.snapshot(), before);
        assert!(second.snapshot().preedit.is_empty());
        assert!(first.selection_is_current(first.epoch(), before.generation));
        first.event(Event::Key(event));
        assert!(!first.selection_is_current(first.epoch(), before.generation));
    }

    fn deliver(session: &mut Session, event: KeyEvent) -> bool {
        let before = session.snapshot();
        for _ in 0..2 {
            assert!(session.test_delivery(&event));
            assert_eq!(session.snapshot(), before);
        }
        let expected = session.probe(&event);
        let (_, output) = session.event(Event::Key(event));
        assert_eq!(output.consumed, expected);
        output.consumed
    }

    fn native(vk: u16, release: bool, shift: bool, ctrl: bool) -> KeyEvent {
        crate::keys::normalize(
            vk,
            0,
            release,
            KeyEvent {
                shift,
                ctrl,
                ..KeyEvent::default()
            },
        )
    }

    #[test]
    fn test_actual_dispatch_gate_observes_passthrough_and_release_events() {
        use ime_core::InputMode;
        let mut session = Session::new(fixture());
        assert!(!deliver(&mut session, native(0x10, false, true, false)));
        assert!(deliver(&mut session, native(0x10, true, false, false)));
        assert_eq!(session.snapshot().mode, InputMode::Latin);
        assert!(!deliver(&mut session, native(0x41, false, false, false)));
        assert!(!deliver(&mut session, native(0x41, true, false, false)));
        assert!(!deliver(&mut session, native(0x10, false, true, false)));
        assert!(deliver(&mut session, native(0x10, true, false, false)));
        assert_eq!(session.snapshot().mode, InputMode::Chinese);
        for event in [
            native(0x10, false, true, false),
            native(0x11, false, true, true),
            native(0x41, false, true, true),
            native(0x41, true, true, true),
            native(0x11, true, true, false),
            native(0x10, true, false, false),
        ] {
            assert!(!deliver(&mut session, event));
        }
        assert_eq!(session.snapshot().mode, InputMode::Chinese);
    }

    #[test]
    fn focus_reset_and_deactivation_reject_stale_edits_and_candidates() {
        let mut session = Session::new(fixture());
        for name in ["1", "8", "space", "down"] {
            session.event(key(name));
        }
        let generation = session.snapshot().generation;
        let epoch = session.epoch();
        let (queued, _) = session.event(key("enter"));
        let (cancel, output) = session.invalidate(Event::FocusLost);
        assert!(output.commit.is_none());
        assert!(output.snapshot.preedit.is_empty());
        assert!(!session.accept(queued));
        assert!(session.accept(cancel));
        assert!(!session.selection_is_current(epoch, generation));
        for event in [Event::Reset, Event::Reset, Event::Deactivate] {
            let (ticket, output) = session.invalidate(event);
            assert!(output.commit.is_none());
            assert!(output.snapshot.preedit.is_empty());
            assert!(session.accept(ticket));
        }
        assert!(!session.snapshot().active);
        session.event(Event::Activate);
        assert!(session.snapshot().active);
    }

    fn assert_subset(actual: &serde_json::Value, expected: &serde_json::Value) {
        match expected {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    assert_subset(&actual[key], value);
                }
            }
            _ => assert_eq!(actual, expected),
        }
    }

    #[test]
    fn shared_replay_matches_core_and_all_offsets_are_valid() {
        let lexicon = fixture();
        let mut session = Session::new(lexicon.clone());
        let mut core = Engine::new(lexicon);
        core.handle(Event::Activate);
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/basic.jsonl");
        for (line_number, line) in std::fs::read_to_string(path).unwrap().lines().enumerate() {
            let record: serde_json::Value = serde_json::from_str(line).unwrap();
            let event: Event = serde_json::from_value(record["event"].clone()).unwrap();
            if let Event::Key(ref key) = event {
                let before = session.snapshot();
                assert_eq!(session.probe(key), core.probe(key));
                assert_eq!(session.snapshot(), before);
            }
            let (ticket, output) = session.event(event.clone());
            assert_eq!(output, core.handle(event));
            assert_subset(&serde_json::to_value(&output).unwrap(), &record["expect"]);
            assert!(
                crate::text::TextSnapshot::checked(&output.snapshot).is_ok(),
                "invalid output offsets at fixture line {}",
                line_number + 1
            );
            assert!(session.accept(ticket));
            assert!(!session.accept(ticket));
        }
    }

    #[test]
    fn panic_recovery_invalidates_partial_state_and_can_accept_new_input() {
        let mut session = Session::new(fixture());
        let epoch = session.epoch();
        let result = boundary(|| {
            session.event(key("1"));
            panic!("partial event test");
        });
        assert_eq!(result, Err(()));
        let (ticket, output) = session.invalidate(Event::Reset);
        assert_ne!(epoch, session.epoch());
        assert!(output.commit.is_none());
        assert!(output.snapshot.preedit.is_empty());
        assert!(session.accept(ticket));
        let (_, output) = session.event(key("1"));
        assert!(output.consumed);
        assert_eq!(output.snapshot.preedit, "ㄅ");
    }

    #[test]
    fn panic_is_contained_and_not_retried() {
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            boundary(|| {
                calls.set(calls.get() + 1);
                panic!("test boundary")
            }),
            Err(())
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(boundary(|| 42), Ok(42));
    }
}
