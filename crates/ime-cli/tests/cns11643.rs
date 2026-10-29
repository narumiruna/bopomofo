use ime_core::{Lexicon, parser::Syllable};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ime-cns-test-{}-{}-{id}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        fixture.sources("1-4421\tㄧ\n".as_bytes(), b"1-4421\t4E00\n");
        fs::write(fixture.path("seed.tsv"), "# bopomofo-lexicon-v1\n").unwrap();
        fixture
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn sources(&self, phonetic: &[u8], unicode: &[u8]) {
        fs::write(self.path("phonetic.tsv"), phonetic).unwrap();
        fs::write(self.path("unicode.tsv"), unicode).unwrap();
        fs::write(
            self.path("manifest.json"),
            json!({
                "format": "cns11643-tw-v1", "version": "20261028",
                "phonetic_sha256": digest(phonetic), "unicode_sha256": digest(unicode)
            })
            .to_string(),
        )
        .unwrap();
    }

    fn import(&self, destination: &Path) -> Output {
        import(&self.0, &self.path("seed.tsv"), destination)
    }

    fn inputs(&self) -> Vec<Vec<u8>> {
        ["manifest.json", "phonetic.tsv", "unicode.tsv", "seed.tsv"]
            .map(|name| fs::read(self.path(name)).unwrap())
            .into()
    }

    fn no_staging_files(&self) {
        assert!(fs::read_dir(&self.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".ime-cli-")
        }));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn import(source: &Path, seed: &Path, destination: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ime-cli"))
        .arg("import-cns11643")
        .arg(source)
        .arg(seed)
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

fn failure(output: &Output, kind: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains(kind), "{diagnostic}");
    assert!(!diagnostic.contains("PRIVATE_INPUT_SENTINEL"));
}

#[test]
fn pinned_source_rebuild_matches_artifact_report_and_character_scope() {
    let fixture = Fixture::new();
    let root = root();
    let build = || {
        import(
            &root.join("data/cns11643/source"),
            &root.join("fixtures/lexicon.tsv"),
            &fixture.path("built.tsv"),
        )
    };
    let first = build();
    success(&first);
    let expected = fs::read(root.join("data/cns11643/lexicon.tsv")).unwrap();
    assert_eq!(fs::read(fixture.path("built.tsv")).unwrap(), expected);
    let second = build();
    success(&second);
    assert_eq!(first.stdout, second.stdout);
    let summary: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        summary,
        serde_json::from_slice::<Value>(&fs::read(root.join("data/cns11643/build.json")).unwrap())
            .unwrap()
    );
    assert_eq!(summary["sha256"], digest(&expected));
    assert_eq!(summary["characters"], 13_052);
    assert_eq!(summary["character_readings"], 16_496);
    assert_eq!(summary["excluded_readings"], 1);
    assert_eq!(summary["phrase_entries"], 5);
    assert_eq!(summary["ignored_seed_characters"], 34);
    assert_eq!(summary["entries"], 16_501);
    let lexicon = Lexicon::parse(std::str::from_utf8(&expected).unwrap()).unwrap();
    for (reading, text) in [
        ("ㄨㄛˇ", "我"),
        ("ㄍㄨㄛˊ", "國"),
        ("ㄆㄧㄚ", "啪"),
        ("ㄈㄧㄠˋ", "覅"),
        ("ㄋㄨㄟˊ", "捼"),
        ("ㄌㄩㄢˊ", "孿"),
        ("ㄌㄩㄢˇ", "孌"),
        ("ㄙㄟ", "塞"),
        ("ㄅㄛˊ", "礴"),
        ("ㄌㄜ˙", "了"),
    ] {
        assert!(
            lexicon
                .lookup(&[Syllable::parse(reading).unwrap()])
                .iter()
                .any(|entry| entry.text == text)
        );
    }
    for text in ["汉", "国", "学", "湾", "龙", "语", "发"] {
        assert!(!lexicon.entries().iter().any(|entry| entry.text == text));
    }
    assert_eq!(
        lexicon.lookup(&[Syllable::parse("ㄧˋ").unwrap()]).len(),
        154
    );
    assert_eq!(
        lexicon
            .entries()
            .iter()
            .map(|entry| entry.reading.len())
            .max(),
        Some(2)
    );
    fixture.no_staging_files();
}

#[test]
fn cns_replay_covers_new_rimes_and_selection_beyond_the_old_128_candidate_limit() {
    let root = root();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_ime-cli"))
            .arg("replay")
            .arg(root.join("data/cns11643/lexicon.tsv"))
            .arg(root.join("fixtures/cns11643.jsonl"))
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    success(&first);
    success(&second);
    assert_eq!(first.stdout, second.stdout);
    let lines: Vec<Value> = String::from_utf8(first.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 81);
    assert_eq!(
        lines
            .iter()
            .filter(|output| output["commit"] == "我")
            .count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|output| output["commit"] == "齸")
            .count(),
        1
    );
    assert!(
        lines
            .iter()
            .any(|output| output["snapshot"]["candidates"].as_array().unwrap().len() == 154)
    );
}

#[test]
fn source_or_seed_outputs_are_rejected_without_modifying_inputs() {
    let fixture = Fixture::new();
    let before = fixture.inputs();
    for path in ["manifest.json", "phonetic.tsv", "unicode.tsv", "seed.tsv"] {
        failure(&fixture.import(&fixture.path(path)), "cns_output_is_input");
        assert_eq!(fixture.inputs(), before);
    }
    fixture.no_staging_files();
}

#[test]
fn invalid_manifest_checksum_encoding_or_records_never_replace_output() {
    let fixture = Fixture::new();
    let destination = fixture.path("output.tsv");
    fs::write(&destination, "previous artifact").unwrap();
    for invalid in [
        "PRIVATE_INPUT_SENTINEL",
        "{}",
        "{\"format\":\"cns11643-tw-v1\",\"version\":\"20261028\",\"phonetic_sha256\":\"wrong\",\"unicode_sha256\":\"wrong\",\"extra\":1}",
    ] {
        fs::write(fixture.path("manifest.json"), invalid).unwrap();
        let before = fixture.inputs();
        failure(&fixture.import(&destination), "invalid_cns_manifest");
        assert_eq!(fs::read(&destination).unwrap(), b"previous artifact");
        assert_eq!(fixture.inputs(), before);
    }
    for (phonetic, unicode, kind) in [
        (
            vec![0xff],
            b"1-4421\t4E00\n".to_vec(),
            "invalid_cns_encoding",
        ),
        (
            b"1-4421\tPRIVATE_INPUT_SENTINEL\n".to_vec(),
            b"1-4421\t4E00\n".to_vec(),
            "invalid_cns_syllable",
        ),
        (
            "3-2121\tㄏㄢˋ\n".as_bytes().to_vec(),
            b"3-2121\t6C49\n".to_vec(),
            "cns_code_out_of_scope",
        ),
        (
            "1-4421\tㄧ\n".as_bytes().to_vec(),
            b"1-4421\tD800\n".to_vec(),
            "invalid_cns_han_mapping",
        ),
    ] {
        fixture.sources(&phonetic, &unicode);
        let before = fixture.inputs();
        failure(&fixture.import(&destination), kind);
        assert_eq!(fs::read(&destination).unwrap(), b"previous artifact");
        assert_eq!(fixture.inputs(), before);
    }
    fixture.sources("1-4421\tㄧ\n".as_bytes(), b"1-4421\t4E00\n");
    fs::write(fixture.path("phonetic.tsv"), "changed source").unwrap();
    failure(
        &fixture.import(&destination),
        "cns_source_checksum_mismatch",
    );
    assert_eq!(fs::read(&destination).unwrap(), b"previous artifact");
    fixture.no_staging_files();
}

#[test]
fn failed_atomic_replace_keeps_existing_directory_and_cleans_stage() {
    let fixture = Fixture::new();
    let destination = fixture.path("output.tsv");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("sentinel"), "previous artifact").unwrap();
    failure(&fixture.import(&destination), "output_replace_failed");
    assert_eq!(
        fs::read(destination.join("sentinel")).unwrap(),
        b"previous artifact"
    );
    fixture.no_staging_files();
}
