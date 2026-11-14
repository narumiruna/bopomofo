mod common;
use common::*;
use ime_core::{Engine, Event, InputMode, KeyEvent, Status};

#[test]
fn phrase_and_transient_commit() {
    let mut e = engine();
    type_keys(&mut e, "su3cl3");
    assert_eq!(e.snapshot().preedit, "你好");
    assert_eq!(key(&mut e, "enter").commit.as_deref(), Some("你好"));
    assert_eq!(e.snapshot().preedit, "");
    assert_eq!(key(&mut e, "enter").commit, None);
    assert_eq!(e.handle(Event::Reset).commit, None);
}
#[test]
fn tones() {
    for (tone, text) in [
        ("space", "八"),
        ("6", "拔"),
        ("3", "把"),
        ("4", "爸"),
        ("7", "罷"),
    ] {
        let mut e = engine();
        type_keys(&mut e, "18");
        key(&mut e, tone);
        assert_eq!(e.snapshot().preedit, text);
    }
}
#[test]
fn readonly_probe_matches_actual_on_all_routes() {
    let mut e = engine();
    let mut seed = 17u64;
    let keys = [
        "s",
        "u",
        "3",
        "c",
        "l",
        "1",
        "8",
        "space",
        "enter",
        "escape",
        "backspace",
        "delete",
        "left",
        "right",
        "up",
        "down",
        "page_down",
        "page_up",
        "tab",
        "caps_lock",
        "shift",
        "control",
        "not-a-key",
    ];
    for i in 0..20_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let k = KeyEvent {
            key: keys[(seed as usize) % keys.len()].into(),
            ctrl: i % 17 == 0,
            alt: i % 31 == 0,
            meta: i % 41 == 0,
            shift: i % 13 == 0,
            repeat: i % 7 == 0,
            release: i % 3 == 0,
        };
        let before = e.snapshot();
        let predicted = e.probe(&k);
        for _ in 0..3 {
            assert_eq!(e.probe(&k), predicted);
            assert_eq!(e.snapshot(), before);
        }
        let result = e.handle(Event::Key(k));
        assert_eq!(result.consumed, predicted);
        let s = result.snapshot;
        assert!(s.preedit.is_char_boundary(s.cursor));
        assert!(s.candidates.len() <= 256);
        assert!(!s.selecting || !s.candidates.is_empty());
        assert!(s.candidates.iter().all(|c| c.start < c.end && c.end <= 64));
        if i % 251 == 0 {
            e.handle(Event::Reset);
        }
    }
}
#[test]
fn candidates_pages_and_stale_events() {
    let mut e = engine();
    type_keys(&mut e, "18 ");
    assert!(e.snapshot().candidates.is_empty());
    key(&mut e, "down");
    assert_eq!(e.snapshot().candidates[0].text, "八");
    key(&mut e, "page_down");
    assert_eq!(e.snapshot().page, 1);
    key(&mut e, "1");
    assert_eq!(e.snapshot().preedit, "豝");
    let before = e.snapshot();
    let stale = e.handle(Event::Select {
        generation: before.generation.wrapping_sub(1),
        index: 0,
    });
    assert_eq!(stale.status, Some(Status::StaleSelection));
    assert_eq!(stale.snapshot, before);
    assert_eq!(stale.commit, None);
}
#[test]
fn absolute_ui_selection_and_cancel() {
    let mut e = engine();
    type_keys(&mut e, "18 ");
    key(&mut e, "down");
    let g = e.snapshot().generation;
    e.handle(Event::Select {
        generation: g,
        index: 10,
    });
    assert_eq!(e.snapshot().preedit, "仈");
    key(&mut e, "down");
    key(&mut e, "escape");
    assert_eq!(e.snapshot().preedit, "仈");
    assert!(!e.snapshot().selecting);
    key(&mut e, "escape");
    assert_eq!(e.snapshot().preedit, "");
}
#[test]
fn pending_unknown_invalid_and_navigation_never_lose_text() {
    let mut e = engine();
    type_keys(&mut e, "q8");
    assert_eq!(key(&mut e, "space").status, Some(Status::UnknownReading));
    assert_eq!(e.snapshot().preedit, "ㄆㄚ");
    for k in ["down", "left", "right", "home", "delete"] {
        assert_eq!(key(&mut e, k).snapshot.preedit, "ㄆㄚ");
    }
    assert_eq!(key(&mut e, "enter").status, Some(Status::InvalidSyllable));
    assert_eq!(e.snapshot().preedit, "ㄆㄚ");
    key(&mut e, "backspace");
    assert_eq!(e.snapshot().preedit, "ㄆ");
    key(&mut e, "escape");
    assert_eq!(e.snapshot().preedit, "");
}
#[test]
fn context_isolation_and_lifecycle() {
    let mut a = engine();
    let b = engine();
    type_keys(&mut a, "su3");
    assert!(b.snapshot().preedit.is_empty());
    for event in [Event::FocusLost, Event::Reset, Event::Deactivate] {
        let out = a.handle(event);
        assert_eq!(out.commit, None);
        assert!(out.snapshot.preedit.is_empty());
    }
    assert!(!key(&mut a, "s").consumed);
    a.handle(Event::Activate);
    assert!(key(&mut a, "s").consumed);
}
#[test]
fn shortcuts_modes_and_shift_chords() {
    let mut e = engine();
    type_keys(&mut e, "su3");
    let before = e.snapshot();
    for k in [
        KeyEvent {
            key: "2".into(),
            ctrl: true,
            ..Default::default()
        },
        KeyEvent {
            key: "a".into(),
            alt: true,
            ..Default::default()
        },
        KeyEvent {
            key: "s".into(),
            meta: true,
            ..Default::default()
        },
        KeyEvent {
            key: "caps_lock".into(),
            ctrl: true,
            ..Default::default()
        },
        KeyEvent {
            key: "caps_lock".into(),
            alt: true,
            ..Default::default()
        },
        KeyEvent {
            key: "caps_lock".into(),
            meta: true,
            ..Default::default()
        },
    ] {
        assert!(!e.handle(Event::Key(k)).consumed);
        assert_eq!(e.snapshot(), before);
    }
    key(&mut e, "caps_lock");
    assert_eq!(e.snapshot().mode, InputMode::Latin);
    assert!(e.snapshot().preedit.is_empty());
    assert!(!key(&mut e, "s").consumed);
    e.handle(Event::Reset);
    e.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        shift: true,
        ..Default::default()
    }));
    e.handle(Event::Key(KeyEvent {
        key: "shift".into(),
        release: true,
        ..Default::default()
    }));
    assert_eq!(e.snapshot().mode, InputMode::Chinese);
}
#[test]
fn limits_do_not_autocommit() {
    let mut e = engine();
    for _ in 0..64 {
        type_keys(&mut e, "su3");
    }
    let before = e.snapshot();
    let o = key(&mut e, "s");
    assert_eq!(o.status, Some(Status::LimitReached));
    assert_eq!(o.commit, None);
    assert_eq!(o.snapshot.preedit, before.preedit);
    assert_eq!(key(&mut e, "enter").commit.unwrap().chars().count(), 64);
}
#[test]
fn non_bmp_cursor_and_multi_syllable_span() {
    let mut e = Engine::new(assets(
        "ㄧ\t𠀀\t1\nㄋㄧˇ\t你\t2\nㄏㄠˇ\t好\t2\nㄋㄧˇ ㄏㄠˇ\t你好\t100\n",
    ));
    type_keys(&mut e, "u ");
    assert_eq!(e.snapshot().cursor, 4);
    assert_eq!(key(&mut e, "enter").commit.as_deref(), Some("𠀀"));
    type_keys(&mut e, "su3cl3");
    key(&mut e, "home");
    key(&mut e, "down");
    let s = e.snapshot();
    assert_eq!((s.candidates[0].start, s.candidates[0].end), (0, 2));
    key(&mut e, "tab");
    assert!(e.snapshot().candidates.iter().all(|c| c.end == 1));
    let mut unicode = Engine::new(assets("ㄧ\t𠀀\t1\nㄧ ㄧ ㄧ ㄧ\t𠀀é😀\t1000000\n"));
    type_keys(&mut unicode, "u u u u ");
    assert_eq!(unicode.snapshot().preedit, "𠀀é😀");
    assert_eq!(unicode.snapshot().cursor, 11);
    key(&mut unicode, "home");
    key(&mut unicode, "right");
    key(&mut unicode, "right");
    assert_eq!(unicode.snapshot().cursor, 5); // Before the combining scalar, not a grapheme count.
    key(&mut unicode, "home");
    key(&mut unicode, "down");
    assert_eq!(
        (
            unicode.snapshot().candidates[0].start,
            unicode.snapshot().candidates[0].end
        ),
        (0, 4)
    );
    key(&mut unicode, "1");
    assert_eq!(unicode.snapshot().cursor, 0);
    key(&mut unicode, "end");
    assert_eq!(unicode.snapshot().cursor, 11);
    assert_eq!(key(&mut unicode, "enter").commit.as_deref(), Some("𠀀é😀"));
}
#[test]
fn learning_and_logging_are_disabled() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Capture(AtomicUsize);
    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, _: &log::Record<'_>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn flush(&self) {}
    }
    static LOGGER: Capture = Capture(AtomicUsize::new(0));
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Off);
    eprintln!("BOPOMOFO_IO_BEGIN");
    let mut a = engine();
    let mut b = engine();
    type_keys(&mut a, "18 ");
    key(&mut a, "down");
    key(&mut a, "2");
    key(&mut a, "enter");
    for _ in 0..3 {
        type_keys(&mut a, "18 ");
        type_keys(&mut b, "18 ");
        assert_eq!(a.snapshot().preedit, b.snapshot().preedit);
        key(&mut a, "escape");
        key(&mut b, "escape");
    }
    // Include probe, selection, cancellation, learning shortcuts, and teardown.
    let shortcut = KeyEvent {
        key: "2".into(),
        ctrl: true,
        ..Default::default()
    };
    assert!(!a.probe(&shortcut));
    assert!(!a.handle(Event::Key(shortcut)).consumed);
    a.handle(Event::Reset);
    b.handle(Event::Deactivate);
    drop(a);
    drop(b);
    assert_eq!(LOGGER.0.load(Ordering::Relaxed), 0);
    eprintln!("BOPOMOFO_IO_END");
}
