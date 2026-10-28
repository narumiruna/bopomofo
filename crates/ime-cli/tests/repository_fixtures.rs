use std::path::Path;
use std::process::Command;

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
    ] {
        let run = || {
            Command::new(env!("CARGO_BIN_EXE_ime-cli"))
                .arg("replay")
                .arg(root.join("fixtures/lexicon.tsv"))
                .arg(root.join(format!("fixtures/{case}.jsonl")))
                .output()
                .unwrap()
        };
        let a = run();
        let b = run();
        assert!(
            a.status.success(),
            "case {case}: {}",
            String::from_utf8_lossy(&a.stderr)
        );
        assert!(
            b.status.success(),
            "case {case}: {}",
            String::from_utf8_lossy(&b.stderr)
        );
        assert_eq!(a.stdout, b.stdout, "case {case} must be deterministic");
        assert!(a.stderr.is_empty());
    }
}
