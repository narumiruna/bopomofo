use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "bopomofo-assets-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ime-cli"))
}
fn build(source: &Path, destination: &Path) -> Output {
    cli()
        .arg("build-assets")
        .arg(source)
        .arg(destination)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
fn rejected(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_INPUT_SENTINEL"));
}
#[test]
fn fixture_assets_rebuild_and_validate_without_changing_sources() {
    let s = Sandbox::new();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lexicon.tsv");
    let original = fs::read(&source).unwrap();
    let a = build(&source, &s.path("a"));
    let b = build(&source, &s.path("b"));
    success(&a);
    success(&b);
    assert_eq!(a.stdout, b.stdout);
    for name in [
        "dictionary.dat",
        "assets.json",
        "Project-LICENSE.txt",
        "Engine-LICENSE.txt",
        "Engine-AUTHORS.txt",
        "Engine-NOTICE.txt",
        "Data-LICENSE.txt",
        "Data-NOTICE.txt",
    ] {
        assert_eq!(
            fs::read(s.path("a").join(name)).unwrap(),
            fs::read(s.path("b").join(name)).unwrap()
        );
    }
    success(
        &cli()
            .arg("validate-assets")
            .arg(s.path("a"))
            .output()
            .unwrap(),
    );
    rejected(&build(&source, &s.path("a")));
    assert_eq!(fs::read(source).unwrap(), original);
    assert!(fs::read_dir(&s.0).unwrap().all(|f| {
        !f.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".bopomofo-assets-")
    }));
}
#[test]
fn source_and_package_failures_are_closed_and_non_destructive() {
    let s = Sandbox::new();
    let source = s.path("source.tsv");
    fs::write(&source, "# bopomofo-lexicon-v1\nㄧ\t一\t1\n").unwrap();
    success(&build(&source, &s.path("valid")));
    let bytes = fs::read(s.path("valid/dictionary.dat")).unwrap();
    let manifest = fs::read(s.path("valid/assets.json")).unwrap();
    for bad in [
        "# bopomofo-lexicon-v1\nㄧ\t𠀀é😀\t1\n",
        "# bopomofo-lexicon-v1\n",
        "PRIVATE_INPUT_SENTINEL",
    ] {
        fs::write(&source, bad).unwrap();
        rejected(&build(&source, &s.path("bad")));
        assert!(!s.path("bad").exists());
        assert_eq!(fs::read(s.path("valid/dictionary.dat")).unwrap(), bytes);
    }
    fs::write(s.path("valid/dictionary.dat"), b"corrupt").unwrap();
    rejected(
        &cli()
            .arg("validate-assets")
            .arg(s.path("valid"))
            .output()
            .unwrap(),
    );
    fs::write(s.path("valid/dictionary.dat"), &bytes).unwrap();
    let mut m: Value = serde_json::from_slice(&manifest).unwrap();
    m["engine"] = json!("chewing-0.14");
    fs::write(s.path("valid/assets.json"), m.to_string()).unwrap();
    rejected(
        &cli()
            .arg("validate-assets")
            .arg(s.path("valid"))
            .output()
            .unwrap(),
    );
    fs::remove_file(s.path("valid/dictionary.dat")).unwrap();
    rejected(
        &cli()
            .arg("validate-assets")
            .arg(s.path("valid"))
            .output()
            .unwrap(),
    );
}
#[test]
fn pinned_default_contains_phrases_and_independent_notices() {
    let s = Sandbox::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let first = build(&root.join("data/libchewing/source"), &s.path("default"));
    success(&first);
    let summary: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(summary["entries"], 160440);
    let again = build(&root.join("data/libchewing/source"), &s.path("repeat"));
    success(&again);
    assert_eq!(first.stdout, again.stdout);
    assert_eq!(
        fs::read(s.path("default/dictionary.dat")).unwrap(),
        fs::read(s.path("repeat/dictionary.dat")).unwrap()
    );
    let source_notice = fs::read_to_string(s.path("default/Data-NOTICE.txt")).unwrap();
    assert!(source_notice.contains("LGPL-2.1-or-later") && source_notice.contains("bfba418"));
    assert_ne!(
        fs::read(s.path("default/Data-LICENSE.txt")).unwrap(),
        fs::read(s.path("default/Project-LICENSE.txt")).unwrap()
    );
    let mut records = String::new();
    for key in ["s", "u", "3", "c", "l", "3"] {
        records.push_str(
            &json!({"event":{"type":"key","key":key},"expect":{"consumed":true,"commit":null}})
                .to_string(),
        );
        records.push('\n');
    }
    records.push_str("{\"event\":{\"type\":\"key\",\"key\":\"enter\"},\"expect\":{\"commit\":\"你好\",\"snapshot\":{\"preedit\":\"\"}}}\n");
    fs::write(s.path("phrase.jsonl"), records).unwrap();
    success(
        &cli()
            .arg("replay")
            .arg(s.path("default"))
            .arg(s.path("phrase.jsonl"))
            .output()
            .unwrap(),
    );
}
#[test]
fn upstream_source_pin_is_not_inferred_from_directory_name() {
    let s = Sandbox::new();
    fs::write(s.path("word.csv"), "PRIVATE_INPUT_SENTINEL").unwrap();
    fs::write(s.path("tsi.csv"), "PRIVATE_INPUT_SENTINEL").unwrap();
    rejected(&build(&s.0, &s.path("out")));
    assert!(!s.path("out").exists());
}
