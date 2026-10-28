use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const LEXICON: &str = "# bopomofo-lexicon-v1\nㄇㄚ\t媽\t20\nㄅㄚ\t巴\t10\n";
const CANONICAL: &str = "# bopomofo-lexicon-v1\nㄅㄚ\t巴\t10\nㄇㄚ\t媽\t20\n";
const RESET: &str = "{\"event\":{\"type\":\"reset\"}}\n";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        for _ in 0..128 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "ime-cli-test-{}-{}-{sequence}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    let fixture = Self { directory };
                    fixture.write("lexicon.tsv", LEXICON);
                    return fixture;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        }
        panic!("cannot allocate a unique test directory");
    }

    fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    fn write(&self, name: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, contents).unwrap();
        path
    }

    fn replay(&self, contents: &str) -> Output {
        let case = self.write("case.jsonl", contents);
        replay(&self.path("lexicon.tsv"), &case)
    }

    fn assert_no_staged_files(&self) {
        for entry in fs::read_dir(&self.directory).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(!name.to_string_lossy().starts_with(".ime-cli-"));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ime-cli"))
}

fn replay(lexicon: &Path, case: &Path) -> Output {
    cli().arg("replay").arg(lexicon).arg(case).output().unwrap()
}

fn build(input: &Path, output: &Path) -> Output {
    cli()
        .arg("build-lexicon")
        .arg(input)
        .arg(output)
        .output()
        .unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn failure(output: &Output, kind: &str) {
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(kind), "unexpected diagnostic: {stderr}");
    assert!(!stderr.contains("PRIVATE_INPUT_SENTINEL"));
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn arguments_are_required_and_unknown_commands_are_rejected() {
    for args in [
        vec![],
        vec!["replay"],
        vec!["replay", "one"],
        vec!["validate-lexicon"],
        vec!["build-lexicon", "one"],
        vec!["benchmark", "one", "two"],
        vec!["replay", "one", "two", "extra"],
        vec!["PRIVATE_INPUT_SENTINEL"],
    ] {
        let output = cli().args(args).output().unwrap();
        failure(&output, "Usage:");
        assert!(output.stdout.is_empty());
    }
    let output = cli().arg("--help").output().unwrap();
    success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("fixture"));
}

#[test]
fn malformed_records_are_preparsed_without_partial_output_or_input_diagnostics() {
    let fixture = Fixture::new();
    for malformed in [
        "PRIVATE_INPUT_SENTINEL",
        "{\"event\":{\"type\":\"PRIVATE_INPUT_SENTINEL\"}}",
        "{\"event\":{\"type\":\"reset\"},\"PRIVATE_INPUT_SENTINEL\":1}",
        "{\"event\":{\"type\":\"reset\",\"PRIVATE_INPUT_SENTINEL\":1}}",
        "{\"event\":{\"type\":\"key\",\"key\":\"1\",\"PRIVATE_INPUT_SENTINEL\":1}}",
        "{\"expect\":{}}",
        "{\"event\":{\"type\":\"reset\"},\"event\":{\"type\":\"reset\"}}",
        "{\"event\":{\"type\":\"reset\"}} {}",
        "[]",
        "",
    ] {
        let output = fixture.replay(&format!("{RESET}{malformed}\n"));
        failure(&output, "line 2: invalid_record");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn empty_invalid_encoding_and_deeply_nested_corpora_are_rejected() {
    let fixture = Fixture::new();
    let output = fixture.replay("");
    failure(&output, "empty_corpus");
    let path = fixture.write("invalid.jsonl", [0xff, 0xfe]);
    let output = replay(&fixture.path("lexicon.tsv"), &path);
    failure(&output, "invalid_corpus_encoding");
    assert!(output.stdout.is_empty());

    let nested = format!(
        "{{\"event\":{{\"type\":\"reset\"}},\"expect\":{}0{}}}\n",
        "[".repeat(256),
        "]".repeat(256)
    );
    let output = fixture.replay(&nested);
    failure(&output, "line 1: invalid_record");
    assert!(output.stdout.is_empty());
}

#[test]
fn malformed_lexicons_do_not_print_source_text() {
    let fixture = Fixture::new();
    for invalid in [
        "PRIVATE_INPUT_SENTINEL\n",
        "# bopomofo-lexicon-v1\nㄅㄚ\tPRIVATE_INPUT_SENTINEL\t0\n",
        "# bopomofo-lexicon-v1\nㄅㄚ\tPRIVATE_INPUT_SENTINEL\t-1\n",
        "# bopomofo-lexicon-v1\nㄅㄚ\tPRIVATE_INPUT_SENTINEL\tnot-a-frequency\n",
        "# bopomofo-lexicon-v1\nPRIVATE_INPUT_SENTINEL\t字\t1\n",
        "# bopomofo-lexicon-v1\nㄅㄚ\tPRIVATE_INPUT_SENTINEL\n",
        "# bopomofo-lexicon-v1\nㄅㄚ\t巴\t10\nㄅㄚ\t巴\t20\n",
    ] {
        let path = fixture.write("invalid.tsv", invalid);
        let output = cli().arg("validate-lexicon").arg(&path).output().unwrap();
        failure(&output, "invalid_lexicon");
        assert!(output.stdout.is_empty());
        let case = fixture.write("case.jsonl", RESET);
        let output = replay(&path, &case);
        failure(&output, "invalid_lexicon");
        assert!(output.stdout.is_empty());
    }
    let path = fixture.write("invalid.tsv", [0xff]);
    let output = cli().arg("validate-lexicon").arg(path).output().unwrap();
    failure(&output, "invalid_lexicon_encoding");
}

#[test]
fn replay_checks_recursive_snapshot_subsets_and_exact_primitives() {
    let fixture = Fixture::new();
    let output = fixture.replay(
        "{\"event\":{\"type\":\"reset\"},\"expect\":{\"consumed\":true,\"snapshot\":{\"preedit\":\"\",\"cursor\":0,\"candidates\":[],\"selecting\":false},\"commit\":null,\"status\":null}}\n",
    );
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["snapshot"]["preedit"], "");
    assert_eq!(value["commit"], Value::Null);

    for expected in [
        json!({"snapshot": {"cursor": 1}}),
        json!({"snapshot": {"candidates": [{}]}}),
        json!({"consumed": false}),
        json!({"PRIVATE_INPUT_SENTINEL": null}),
        Value::Null,
        json!([]),
    ] {
        let record = json!({"event": {"type": "reset"}, "expect": expected});
        let output = fixture.replay(&format!("{record}\n"));
        failure(&output, "line 1: expectation_mismatch");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn repeated_replay_is_byte_for_byte_stable_and_compact() {
    let fixture = Fixture::new();
    let case = fixture.write(
        "case.jsonl",
        format!(
            "{RESET}{{\"event\":{{\"type\":\"key\",\"key\":\"f24\"}}}}\n{{\"event\":{{\"type\":\"activate\"}}}}\n"
        ),
    );
    let first = replay(&fixture.path("lexicon.tsv"), &case);
    let second = replay(&fixture.path("lexicon.tsv"), &case);
    success(&first);
    success(&second);
    assert_eq!(first.stdout, second.stdout);
    let text = String::from_utf8(first.stdout).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(text.ends_with('\n'));
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        assert!(value["snapshot"].is_object());
        assert!(value["consumed"].is_boolean());
        assert!(!line.contains(": "));
    }
}

#[test]
fn lexicon_exports_are_canonical_and_checksums_are_repeatable() {
    let fixture = Fixture::new();
    let input = fixture.path("lexicon.tsv");
    let destination = fixture.path("built.tsv");
    let first = build(&input, &destination);
    success(&first);
    let exported = fs::read(&destination).unwrap();
    assert_eq!(exported, CANONICAL.as_bytes());
    let summary: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(summary["sha256"], digest(&exported));
    assert_eq!(summary["entries"], 2);
    assert_eq!(summary["bytes"], exported.len());

    let second = build(&input, &destination);
    success(&second);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(fs::read(&destination).unwrap(), exported);
    let third = build(&destination, &fixture.path("rebuilt.tsv"));
    success(&third);
    assert_eq!(first.stdout, third.stdout);

    let validated = cli()
        .arg("validate-lexicon")
        .arg(&destination)
        .output()
        .unwrap();
    success(&validated);
    assert_eq!(validated.stdout, first.stdout);
    fixture.assert_no_staged_files();
}

#[test]
fn import_can_replace_its_own_input_safely() {
    let fixture = Fixture::new();
    let input = fixture.path("lexicon.tsv");
    let first = build(&input, &input);
    success(&first);
    assert_eq!(fs::read(&input).unwrap(), CANONICAL.as_bytes());
    let second = build(&input, &input);
    success(&second);
    assert_eq!(first.stdout, second.stdout);
    fixture.assert_no_staged_files();
}

#[test]
fn invalid_import_never_overwrites_existing_destination_or_input() {
    let fixture = Fixture::new();
    let contents = "# bopomofo-lexicon-v1\nㄅㄚ\tPRIVATE_INPUT_SENTINEL\t0\n";
    let input = fixture.write("invalid.tsv", contents);
    let destination = fixture.write("existing.tsv", "existing destination\n");
    let output = build(&input, &destination);
    failure(&output, "invalid_lexicon");
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&destination).unwrap(), b"existing destination\n");
    assert_eq!(fs::read(&input).unwrap(), contents.as_bytes());
    let output = build(&input, &input);
    failure(&output, "invalid_lexicon");
    assert_eq!(fs::read(&input).unwrap(), contents.as_bytes());
    fixture.assert_no_staged_files();
}

#[test]
fn failed_atomic_replace_preserves_destination_and_removes_staging_file() {
    let fixture = Fixture::new();
    let destination = fixture.path("destination-directory");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("existing"), "unchanged").unwrap();
    let output = build(&fixture.path("lexicon.tsv"), &destination);
    failure(&output, "output_replace_failed");
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read(destination.join("existing")).unwrap(),
        b"unchanged"
    );
    fixture.assert_no_staged_files();
}

#[test]
fn file_and_event_limits_fail_before_replay_output() {
    let fixture = Fixture::new();
    let oversized = fixture.path("oversized");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(32 * 1024 * 1024 + 1)
        .unwrap();
    let output = cli()
        .arg("validate-lexicon")
        .arg(&oversized)
        .output()
        .unwrap();
    failure(&output, "input_byte_limit");
    assert!(output.stdout.is_empty());
    let output = replay(&fixture.path("lexicon.tsv"), &oversized);
    failure(&output, "input_byte_limit");
    assert!(output.stdout.is_empty());
    let output = fixture.replay(&RESET.repeat(100_001));
    failure(&output, "line 100001: event_limit");
    assert!(output.stdout.is_empty());
}

#[test]
fn benchmark_iterations_are_positive_and_bounded() {
    for iterations in ["0", "-1", "10001", "99999999999999999999999", "text"] {
        let output = cli()
            .args(["benchmark", "unused.tsv", "unused.jsonl", iterations])
            .output()
            .unwrap();
        failure(&output, "invalid_iterations");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn benchmark_requires_release_and_reports_only_measured_fixture_data() {
    let fixture = Fixture::new();
    let events = [
        json!({"event": {"type": "reset"}}),
        json!({"event": {"type": "key", "key": "f24"}}),
        json!({"event": {"type": "key", "key": "1"}}),
    ];
    let case = fixture.write(
        "benchmark.jsonl",
        events
            .iter()
            .map(|event| format!("{event}\n"))
            .collect::<String>(),
    );
    // Use the current core's output as expectations. This tests context reset
    // and CLI checking without hard-coding core language rules.
    let baseline = replay(&fixture.path("lexicon.tsv"), &case);
    success(&baseline);
    let baseline = String::from_utf8(baseline.stdout).unwrap();
    let checked = events
        .iter()
        .zip(baseline.lines())
        .map(|(record, output)| {
            let expected: Value = serde_json::from_str(output).unwrap();
            let record = json!({"event": record["event"], "expect": expected});
            format!("{record}\n")
        })
        .collect::<String>();
    fixture.write("benchmark.jsonl", &checked);
    let output = cli()
        .arg("benchmark")
        .arg(fixture.path("lexicon.tsv"))
        .arg(&case)
        .arg("3")
        .output()
        .unwrap();
    if cfg!(debug_assertions) {
        failure(&output, "benchmark_requires_release_build");
        assert!(output.stdout.is_empty());
        return;
    }
    success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scope"], "fixture_only");
    assert_eq!(report["iterations"], 3);
    assert_eq!(report["corpus"]["sha256"], digest(checked.as_bytes()));
    assert_eq!(report["corpus"]["events"], 3);
    assert_eq!(report["corpus"]["key_events"], 2);
    assert_eq!(report["corpus"]["total_events"], 9);
    assert_eq!(report["lexicon"]["sha256"], digest(LEXICON.as_bytes()));
    assert_eq!(report["lexicon"]["entries"], 2);
    assert_eq!(report["per_key_latency"]["samples"], 6);
    let latency = &report["per_key_latency"];
    assert!(latency["p50"].as_u64().unwrap() <= latency["p95"].as_u64().unwrap());
    assert!(latency["p95"].as_u64().unwrap() <= latency["p99"].as_u64().unwrap());
    let initialization = &report["initialization"];
    assert_eq!(
        initialization["total_ns"].as_u64().unwrap(),
        initialization["lexicon_parse_ns"].as_u64().unwrap()
            + initialization["engine_new_ns"].as_u64().unwrap()
    );
    assert_eq!(report["environment"]["os"], std::env::consts::OS);
    assert_eq!(report["environment"]["arch"], std::env::consts::ARCH);
    assert_eq!(report["build"]["debug_assertions"], false);
    assert!(report["build"]["rustc_version"].is_null());
    assert!(report["memory"]["status"].is_string());
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        assert!(report["environment"]["cpu"].is_string());
        assert!(report["memory"]["resident_bytes"].as_u64().unwrap() > 0);
    } else {
        assert_eq!(report["memory"]["status"], "unavailable");
    }
}

#[test]
fn release_benchmark_bounds_total_work_and_allows_zero_key_samples() {
    if cfg!(debug_assertions) {
        return;
    }
    let fixture = Fixture::new();
    let case = fixture.write("benchmark.jsonl", RESET.repeat(101));
    let output = cli()
        .arg("benchmark")
        .arg(fixture.path("lexicon.tsv"))
        .arg(&case)
        .arg("10000")
        .output()
        .unwrap();
    failure(&output, "benchmark_event_limit");
    assert!(output.stdout.is_empty());

    fixture.write("benchmark.jsonl", RESET);
    let output = cli()
        .arg("benchmark")
        .arg(fixture.path("lexicon.tsv"))
        .arg(&case)
        .arg("2")
        .output()
        .unwrap();
    success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["per_key_latency"]["samples"], 0);
    for name in ["p50", "p95", "p99"] {
        assert!(report["per_key_latency"][name].is_null());
    }
}
