use ime_core::{Engine, Event, InputMode, KeyEvent, Lexicon, MAX_SYLLABLES, Output, Status};
use std::sync::Arc;

fn engine() -> Engine {
    Engine::new(Arc::new(
        Lexicon::parse(include_str!("../../../fixtures/lexicon.tsv")).unwrap(),
    ))
}
fn key(value: &str) -> Event {
    Event::Key(KeyEvent {
        key: value.into(),
        ..Default::default()
    })
}
fn send(engine: &mut Engine, value: &str) -> Output {
    engine.handle(key(value))
}
fn type_keys(engine: &mut Engine, keys: &str) {
    for character in keys.chars() {
        let value = character.to_string();
        let name = if character == ' ' { "space" } else { &value };
        send(engine, name);
        engine.handle(Event::Key(KeyEvent {
            key: name.into(),
            release: true,
            ..Default::default()
        }));
    }
}
fn choose(engine: &mut Engine, text: &str) {
    send(engine, "down");
    let snapshot = engine.snapshot();
    let index = snapshot
        .candidates
        .iter()
        .position(|candidate| candidate.text == text)
        .unwrap();
    let out = engine.handle(Event::Select {
        generation: snapshot.generation,
        index,
    });
    assert_eq!(out.status, None);
    assert_eq!(out.commit, None);
}

#[test]
fn tones_and_enter_commit_exactly_once() {
    for (tone, expected) in [
        ("space", "八"),
        ("6", "拔"),
        ("3", "把"),
        ("4", "爸"),
        ("7", "罷"),
    ] {
        let mut engine = engine();
        type_keys(&mut engine, "18");
        let out = send(&mut engine, tone);
        assert_eq!(out.snapshot.preedit, expected);
        assert_eq!(out.commit, None);
        let out = send(&mut engine, "enter");
        assert_eq!(out.commit.as_deref(), Some(expected));
        assert!(out.snapshot.preedit.is_empty());
        assert_eq!(send(&mut engine, "enter").commit, None);
        assert!(!send(&mut engine, "enter").consumed);
        assert!(engine.snapshot().preedit.is_empty());
        assert_eq!(engine.handle(Event::Reset).commit, None);
    }
}

#[test]
fn every_allowed_single_reading_candidate_remains_selectable() {
    let mut source = String::from("# bopomofo-lexicon-v1\n");
    for index in 0..ime_core::lexicon::MAX_CANDIDATES_PER_READING {
        source.push_str(&format!("ㄧˋ\t候選{index:03}\t1\n"));
    }
    let mut engine = Engine::new(Arc::new(Lexicon::parse(&source).unwrap()));
    type_keys(&mut engine, "u4");
    let open = send(&mut engine, "down");
    assert_eq!(open.snapshot.candidates.len(), 256);
    for _ in 0..28 {
        send(&mut engine, "page_down");
    }
    assert_eq!(engine.snapshot().page, 28);
    assert_eq!(send(&mut engine, "4").snapshot.preedit, "候選255");
    assert_eq!(
        send(&mut engine, "enter").commit.as_deref(),
        Some("候選255")
    );
    assert_eq!(send(&mut engine, "enter").commit, None);
}

#[test]
fn invalid_pending_unknown_reading_and_space_rules() {
    let mut engine = engine();
    assert!(!send(&mut engine, "space").consumed);
    send(&mut engine, "1");
    for value in ["3", "enter"] {
        let out = send(&mut engine, value);
        assert_eq!(out.status, Some(Status::InvalidSyllable));
        assert_eq!(out.snapshot.preedit, "ㄅ");
        assert_eq!(out.commit, None);
    }
    send(&mut engine, "8");
    send(&mut engine, "backspace");
    assert_eq!(engine.snapshot().preedit, "ㄅ");
    send(&mut engine, "escape");
    type_keys(&mut engine, "q8");
    let out = send(&mut engine, "space");
    assert_eq!(out.snapshot.preedit, "ㄆㄚ");
    assert_eq!(out.status, Some(Status::UnknownReading));
    assert_eq!(send(&mut engine, "space").commit.as_deref(), Some("ㄆㄚ"));
}

#[test]
fn phrase_decode_edit_cursor_and_deletion() {
    let mut engine = engine();
    type_keys(&mut engine, "su3cl3");
    assert_eq!(engine.snapshot().preedit, "你好");
    assert_eq!(engine.snapshot().cursor, 6);
    send(&mut engine, "home");
    send(&mut engine, "delete");
    assert_eq!(engine.snapshot().preedit, "好");
    send(&mut engine, "end");
    send(&mut engine, "backspace");
    assert_eq!(engine.snapshot().preedit, "");
    type_keys(&mut engine, "su3cl3");
    send(&mut engine, "home");
    send(&mut engine, "right");
    type_keys(&mut engine, "18 ");
    assert_eq!(engine.snapshot().preedit, "你八好");
    send(&mut engine, "backspace");
    assert_eq!(engine.snapshot().preedit, "你好");
    send(&mut engine, "end");
    send(&mut engine, "1");
    let before = engine.snapshot().preedit;
    for movement in ["left", "right", "home", "end"] {
        assert_eq!(send(&mut engine, movement).snapshot.preedit, before);
    }
}

#[test]
fn explicit_selection_resolves_digit_tone_conflict_and_pages() {
    let mut engine = engine();
    type_keys(&mut engine, "18 ");
    send(&mut engine, "down");
    assert!(engine.snapshot().selecting);
    let out = send(&mut engine, "2");
    assert_eq!(out.snapshot.preedit, "巴");
    assert!(!out.snapshot.selecting);
    assert_eq!(out.commit, None);
    send(&mut engine, "down");
    send(&mut engine, "page_down");
    assert_eq!(engine.snapshot().page, 1);
    send(&mut engine, "1");
    assert_eq!(engine.snapshot().preedit, "豝");
    send(&mut engine, "down");
    send(&mut engine, "escape");
    assert_eq!(engine.snapshot().preedit, "豝");
    send(&mut engine, "escape");
    assert!(engine.snapshot().preedit.is_empty());
    type_keys(&mut engine, "18 ");
    send(&mut engine, "down");
    send(&mut engine, "down");
    assert_eq!(send(&mut engine, "enter").snapshot.preedit, "巴");
    assert_eq!(send(&mut engine, "enter").commit.as_deref(), Some("巴"));
}

#[test]
fn locks_survive_unrelated_edits_and_unlock_on_overlap() {
    let mut engine = engine();
    type_keys(&mut engine, "su3cl3");
    send(&mut engine, "home");
    choose(&mut engine, "妳");
    send(&mut engine, "end");
    type_keys(&mut engine, "18 ");
    choose(&mut engine, "巴");
    assert_eq!(engine.snapshot().preedit, "妳好巴");
    send(&mut engine, "home");
    send(&mut engine, "delete");
    assert_eq!(engine.snapshot().preedit, "好巴");
    send(&mut engine, "home");
    type_keys(&mut engine, "su3");
    assert_eq!(engine.snapshot().preedit, "你好巴");
    engine.handle(Event::Reset);
    type_keys(&mut engine, "su3cl3");
    send(&mut engine, "home");
    choose(&mut engine, "你好");
    send(&mut engine, "home");
    send(&mut engine, "right");
    type_keys(&mut engine, "18 ");
    assert_eq!(engine.snapshot().preedit, "你八好");
}

#[test]
fn explicit_reselection_can_change_locked_span_length() {
    let mut engine = engine();
    type_keys(&mut engine, "su3cl318 ");
    choose(&mut engine, "巴");
    send(&mut engine, "home");
    choose(&mut engine, "妳");
    assert_eq!(engine.snapshot().preedit, "妳好巴");
    send(&mut engine, "home");
    choose(&mut engine, "你好");
    assert_eq!(engine.snapshot().preedit, "你好巴");
    send(&mut engine, "home");
    send(&mut engine, "right");
    choose(&mut engine, "好");
    assert_eq!(engine.snapshot().preedit, "你好巴");
    send(&mut engine, "home");
    choose(&mut engine, "妳");
    assert_eq!(engine.snapshot().preedit, "妳好巴");
}

#[test]
fn stale_selection_reset_and_context_isolation() {
    let mut a = engine();
    let mut b = a.clone();
    type_keys(&mut a, "18 ");
    assert_eq!(b.snapshot().preedit, "");
    send(&mut a, "down");
    let stale = a.snapshot().generation;
    send(&mut a, "escape");
    send(&mut a, "down");
    let before = a.snapshot();
    let out = a.handle(Event::Select {
        generation: stale,
        index: 0,
    });
    assert_eq!(out.status, Some(Status::StaleSelection));
    assert_eq!(a.snapshot(), before);
    for event in [
        Event::FocusLost,
        Event::Reset,
        Event::Reset,
        Event::Deactivate,
    ] {
        let out = a.handle(event);
        assert_eq!(out.commit, None);
        assert_eq!(out.snapshot.preedit, "");
    }
    assert!(!send(&mut a, "1").consumed);
    a.handle(Event::Activate);
    assert!(send(&mut a, "1").consumed);
    type_keys(&mut b, "su3");
    assert_eq!(b.snapshot().preedit, "你");
}

#[test]
fn shortcuts_modes_release_and_repeat() {
    let mut engine = engine();
    type_keys(&mut engine, "18 ");
    let before = engine.snapshot();
    for (ctrl, alt, meta) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let out = engine.handle(Event::Key(KeyEvent {
            key: "a".into(),
            ctrl,
            alt,
            meta,
            ..Default::default()
        }));
        assert!(!out.consumed);
        assert_eq!(out.snapshot, before);
        let released = engine.handle(Event::Key(KeyEvent {
            key: "a".into(),
            ctrl,
            alt,
            meta,
            release: true,
            ..Default::default()
        }));
        assert!(!released.consumed);
        assert_eq!(released.snapshot, before);
    }
    assert!(
        !engine
            .handle(Event::Key(KeyEvent {
                key: "1".into(),
                release: true,
                ..Default::default()
            }))
            .consumed
    );
    send(&mut engine, "shift");
    let out = engine.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert_eq!(out.snapshot.mode, InputMode::Latin);
    assert_eq!(out.snapshot.preedit, "");
    assert_eq!(out.commit, None);
    assert!(!send(&mut engine, "1").consumed);
    send(&mut engine, "caps_lock");
    assert_eq!(engine.snapshot().mode, InputMode::Chinese);
    engine.handle(Event::Key(KeyEvent {
        key: "caps_lock".into(),
        repeat: true,
        ..Default::default()
    }));
    assert_eq!(engine.snapshot().mode, InputMode::Chinese);
    send(&mut engine, "shift");
    send(&mut engine, "1");
    engine.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert_eq!(engine.snapshot().mode, InputMode::Chinese);
    assert_eq!(engine.snapshot().preedit, "ㄅ");
    let out = engine.handle(Event::Key(KeyEvent {
        key: "1".into(),
        repeat: true,
        ..Default::default()
    }));
    assert_eq!(out.snapshot.preedit, "ㄅ");
}

#[test]
fn shift_gestures_preserve_chords_and_held_keys() {
    let mut engine = engine();
    type_keys(&mut engine, "18 ");
    let shift = |release, aggregate| {
        Event::Key(KeyEvent {
            key: "shift".into(),
            shift: aggregate,
            release,
            ..Default::default()
        })
    };
    engine.handle(shift(false, true));
    engine.handle(Event::Key(KeyEvent {
        key: "a".into(),
        shift: true,
        ..Default::default()
    }));
    engine.handle(shift(false, true));
    engine.handle(shift(true, true));
    engine.handle(Event::Key(KeyEvent {
        key: "a".into(),
        shift: true,
        release: true,
        ..Default::default()
    }));
    engine.handle(shift(true, false));
    assert_eq!(engine.snapshot().mode, InputMode::Chinese);
    assert_eq!(engine.snapshot().preedit, "八ㄇ");
    engine.handle(Event::Reset);
    send(&mut engine, "1");
    engine.handle(shift(false, true));
    engine.handle(shift(true, false));
    assert_eq!(engine.snapshot().mode, InputMode::Chinese);
    assert_eq!(engine.snapshot().preedit, "ㄅ");
    engine.handle(Event::Key(KeyEvent {
        key: "1".into(),
        release: true,
        ..Default::default()
    }));
    engine.handle(shift(false, true));
    engine.handle(shift(true, false));
    assert_eq!(engine.snapshot().mode, InputMode::Latin);
}

#[test]
fn aliased_native_keys_remain_held_until_both_releases() {
    let mut engine = engine();
    send(&mut engine, "enter");
    send(&mut engine, "enter");
    engine.handle(Event::Key(KeyEvent {
        key: "enter".into(),
        release: true,
        ..Default::default()
    }));
    send(&mut engine, "shift");
    let out = engine.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert!(!out.consumed);
    assert_eq!(out.snapshot.mode, InputMode::Chinese);
    engine.handle(Event::Key(KeyEvent {
        key: "enter".into(),
        release: true,
        ..Default::default()
    }));
    send(&mut engine, "shift");
    let out = engine.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert!(out.consumed);
    assert_eq!(out.snapshot.mode, InputMode::Latin);
}

#[test]
fn repeat_observation_without_initial_down_still_tracks_a_held_key() {
    let mut engine = engine();
    engine.handle(Event::Key(KeyEvent {
        key: "1".into(),
        repeat: true,
        ..Default::default()
    }));
    send(&mut engine, "shift");
    let out = engine.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert!(!out.consumed);
    assert_eq!(out.snapshot.mode, InputMode::Chinese);
    engine.handle(Event::Key(KeyEvent {
        key: "1".into(),
        release: true,
        ..Default::default()
    }));
    send(&mut engine, "shift");
    let out = engine.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert!(out.consumed);
    assert_eq!(out.snapshot.mode, InputMode::Latin);
}

#[test]
fn composition_limit_never_loses_or_auto_commits() {
    let mut engine = engine();
    for _ in 0..MAX_SYLLABLES {
        type_keys(&mut engine, "18 ");
    }
    let before = engine.snapshot().preedit;
    assert_eq!(before, "八".repeat(64));
    let out = send(&mut engine, "1");
    assert_eq!(out.status, Some(Status::LimitReached));
    assert_eq!(out.commit, None);
    assert_eq!(out.snapshot.preedit, before);
    send(&mut engine, "backspace");
    type_keys(&mut engine, "184");
    assert_eq!(engine.snapshot().preedit, format!("{}爸", "八".repeat(63)));
}

#[test]
fn unicode_and_multi_scalar_span_offsets_are_valid() {
    let lexicon = Lexicon::parse(
        "# bopomofo-lexicon-v1\nㄅㄚ\t𠀀👩‍💻e\u{301}\t100\nㄅㄚ ㄅㄚ\t𠀁x\u{301}\t10000\n",
    )
    .unwrap();
    let mut engine = Engine::new(Arc::new(lexicon));
    type_keys(&mut engine, "18 ");
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.cursor, snapshot.preedit.len());
    assert!(snapshot.preedit.is_char_boundary(snapshot.cursor));
    type_keys(&mut engine, "18 ");
    send(&mut engine, "home");
    choose(&mut engine, "𠀁x\u{301}");
    send(&mut engine, "home");
    send(&mut engine, "right");
    assert_eq!(engine.snapshot().cursor, 0);
    send(&mut engine, "end");
    assert_eq!(engine.snapshot().cursor, "𠀁x\u{301}".len());
}

#[test]
fn bounded_state_machine_fuzz_and_probe_properties() {
    let mut a = engine();
    let mut b = a.clone();
    let mut random = 0x5eed_u64;
    let keys = [
        "1",
        "8",
        "3",
        "6",
        "4",
        "7",
        "s",
        "u",
        "c",
        "l",
        "space",
        "enter",
        "escape",
        "backspace",
        "delete",
        "left",
        "right",
        "home",
        "end",
        "down",
        "up",
        "page_up",
        "page_down",
        "shift",
        "caps_lock",
        "[",
        "",
    ];
    for index in 0..20_000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let key = KeyEvent {
            key: keys[(random >> 32) as usize % keys.len()].into(),
            ctrl: index % 79 == 0,
            release: index % 7 == 0,
            repeat: index % 11 == 0,
            ..Default::default()
        };
        let before = a.snapshot();
        let probe = a.probe(&key);
        assert_eq!(a.snapshot(), before);
        let event = Event::Key(key);
        let out = a.handle(event.clone());
        assert_eq!(probe, out.consumed);
        assert_eq!(out, b.handle(event));
        assert!(out.snapshot.preedit.is_char_boundary(out.snapshot.cursor));
        assert!(out.snapshot.candidates.len() <= ime_core::lexicon::MAX_CANDIDATES_PER_READING);
        assert!(
            out.snapshot
                .candidates
                .iter()
                .all(|candidate| candidate.start < candidate.end && candidate.end <= 64)
        );
        assert_ne!(out.status, Some(Status::InternalError));
        if index % 103 == 0 {
            assert_eq!(a.handle(Event::Reset), b.handle(Event::Reset));
        }
    }
}
