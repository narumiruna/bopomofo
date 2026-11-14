use std::path::Path;
use std::process::Command;

fn assert_replays_identically(lexicon: &Path, case: &Path) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let assets = std::env::temp_dir().join(format!(
        "bopomofo-replay-assets-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let built = Command::new(env!("CARGO_BIN_EXE_ime-cli"))
        .arg("build-assets")
        .arg(lexicon)
        .arg(&assets)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_ime-cli"))
            .arg("replay")
            .arg(&assets)
            .arg(case)
            .output()
            .unwrap()
    };
    let a = run();
    let b = run();
    for output in [&a, &b] {
        assert!(
            output.status.success(),
            "case {}: {}",
            case.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
    }
    assert_eq!(
        a.stdout,
        b.stdout,
        "case {} must be deterministic",
        case.display()
    );
    std::fs::remove_dir_all(assets).unwrap();
}

#[test]
fn shared_repository_fixtures_match_and_replay_identically() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for case in [
        "basic",
        "tones",
        "modes",
        "editing",
        "limit-64",
        "gestures",
        "reselection",
        "aliases",
        "ui-abort",
    ] {
        assert_replays_identically(
            &root.join("fixtures/lexicon.tsv"),
            &root.join(format!("fixtures/{case}.jsonl")),
        );
    }
}

#[test]
fn lexicon_quality_fixtures_match_and_replay_identically() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lexicon-quality");
    assert_replays_identically(&root.join("lexicon.tsv"), &root.join("cases.jsonl"));
}

#[test]
fn unicode_repository_fixtures_match_and_replay_identically() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/unicode");
    assert_replays_identically(&root.join("lexicon.tsv"), &root.join("cases.jsonl"));
}
