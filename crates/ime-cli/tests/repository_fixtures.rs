use std::path::{Path, PathBuf};
use std::process::Command;

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "bopomofo-replay-assets-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Cannot create test sandbox: {error}"),
            }
        }
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_replays_identically(lexicon: &Path, case: &Path) {
    let sandbox = Sandbox::new();
    let assets = sandbox.0.join("assets");
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
}

#[test]
fn replay_sandbox_is_removed_on_success_and_unwind() {
    let sandbox = Sandbox::new();
    let path = sandbox.0.clone();
    std::fs::write(path.join("asset"), b"synthetic").unwrap();
    drop(sandbox);
    assert!(!path.exists());

    let sandbox = Sandbox::new();
    let path = sandbox.0.clone();
    let result = std::panic::catch_unwind(move || {
        let _sandbox = sandbox;
        std::fs::write(_sandbox.0.join("asset"), b"synthetic").unwrap();
        panic!("Synthetic replay failure");
    });
    assert!(result.is_err());
    assert!(!path.exists());
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
