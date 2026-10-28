use crate::{Error, Result, files, replay, write_json};
use ime_core::{Engine, Event};
use serde::Serialize;
use std::hint::black_box;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub(crate) const MAX_ITERATIONS: usize = 10_000;
const MAX_TOTAL_EVENTS: usize = 1_000_000;

#[derive(Serialize)]
struct CorpusSummary {
    sha256: String,
    bytes: usize,
    events: usize,
    key_events: usize,
    total_events: usize,
}

#[derive(Serialize)]
struct Initialization {
    lexicon_parse_ns: u64,
    engine_new_ns: u64,
    total_ns: u64,
}

#[derive(Serialize)]
struct Latency {
    unit: &'static str,
    percentile_method: &'static str,
    samples: usize,
    p50: Option<u64>,
    p95: Option<u64>,
    p99: Option<u64>,
}

#[derive(Serialize)]
struct Environment {
    os: &'static str,
    os_version: Option<String>,
    arch: &'static str,
    cpu: Option<String>,
}

#[derive(Serialize)]
struct Build {
    package: &'static str,
    package_version: &'static str,
    debug_assertions: bool,
    rustc_version: Option<&'static str>,
    rustc_version_status: &'static str,
}

#[derive(Serialize)]
struct Memory {
    measurement: &'static str,
    source: &'static str,
    resident_bytes: Option<u64>,
    status: &'static str,
}

#[derive(Serialize)]
struct Method {
    timed_call: &'static str,
    timed_events: &'static str,
    repetition_reset: &'static str,
    excluded: [&'static str; 5],
    warmup_iterations: usize,
}

#[derive(Serialize)]
struct Report {
    scope: &'static str,
    limitations: [&'static str; 3],
    iterations: usize,
    lexicon: files::LexiconSummary,
    corpus: CorpusSummary,
    environment: Environment,
    build: Build,
    initialization: Initialization,
    per_key_latency: Latency,
    memory: Memory,
    method: Method,
}

pub(crate) fn run(
    lexicon_path: &Path,
    case_path: &Path,
    iterations: usize,
    output: &mut impl Write,
) -> Result<()> {
    if cfg!(debug_assertions) {
        return Err(Error::new("benchmark_requires_release_build"));
    }
    let lexicon = files::load_lexicon(lexicon_path)?;
    let case = replay::Case::load(case_path)?;
    let total_events = case
        .records
        .len()
        .checked_mul(iterations)
        .filter(|count| *count <= MAX_TOTAL_EVENTS)
        .ok_or_else(|| Error::new("benchmark_event_limit"))?;
    let start = Instant::now();
    let initial_engine = black_box(Engine::new(Arc::clone(&lexicon.data)));
    let engine_new_ns = files::elapsed_ns(start);
    drop(initial_engine);

    let mut latencies = Vec::with_capacity(case.keys * iterations);
    for _ in 0..iterations {
        // A fresh context resets composition, mode, and generation. No reset
        // event is injected into the fixture or included in a latency sample.
        let mut engine = Engine::new(Arc::clone(&lexicon.data));
        for (index, record) in case.records.iter().enumerate() {
            let event = record.event.clone();
            let result = if matches!(&event, Event::Key(_)) {
                let event = black_box(event);
                let start = Instant::now();
                let result = black_box(engine.handle(event));
                let elapsed = files::elapsed_ns(start);
                latencies.push(elapsed);
                result
            } else {
                black_box(engine.handle(event))
            };
            record.check(&result, index + 1)?;
        }
    }
    latencies.sort_unstable();
    let report = Report {
        scope: "fixture_only",
        limitations: [
            "No product performance or representative-corpus claim.",
            "No native adapter or UI rendering is measured.",
            "Latency includes timer and compiler-barrier overhead; memory is a process RSS sample, not peak or allocation cost.",
        ],
        iterations,
        lexicon: lexicon.summary,
        corpus: CorpusSummary {
            sha256: case.sha256,
            bytes: case.bytes,
            events: case.records.len(),
            key_events: case.keys,
            total_events,
        },
        environment: environment(),
        build: Build {
            package: env!("CARGO_PKG_NAME"),
            package_version: env!("CARGO_PKG_VERSION"),
            debug_assertions: cfg!(debug_assertions),
            rustc_version: None,
            rustc_version_status: "unavailable: compiler version was not embedded at build time",
        },
        initialization: Initialization {
            lexicon_parse_ns: lexicon.initialization_ns,
            engine_new_ns,
            total_ns: lexicon.initialization_ns.saturating_add(engine_new_ns),
        },
        per_key_latency: Latency {
            unit: "nanoseconds",
            percentile_method: "nearest_rank",
            samples: latencies.len(),
            p50: percentile(&latencies, 50),
            p95: percentile(&latencies, 95),
            p99: percentile(&latencies, 99),
        },
        memory: memory(),
        method: Method {
            timed_call: "Engine::handle including the complete returned snapshot",
            timed_events: "key events only; other fixture events still run",
            repetition_reset: "fresh Engine::new before each repetition",
            excluded: [
                "file I/O and corpus parsing",
                "per-repetition engine initialization",
                "event cloning",
                "expectation checks and output serialization",
                "returned output destruction",
            ],
            warmup_iterations: 0,
        },
    };
    write_json(output, &report)
}

fn percentile(sorted: &[u64], percent: usize) -> Option<u64> {
    if sorted.is_empty() {
        None
    } else {
        Some(sorted[(sorted.len() * percent).div_ceil(100) - 1])
    }
}

fn environment() -> Environment {
    Environment {
        os: std::env::consts::OS,
        os_version: os_version(),
        arch: std::env::consts::ARCH,
        cpu: cpu(),
    }
}

#[cfg(target_os = "macos")]
fn command_text(program: &str, args: &[&str]) -> Option<String> {
    let result = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !result.status.success() || result.stdout.len() > 128 * 1024 {
        return None;
    }
    let text = String::from_utf8(result.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(target_os = "macos")]
fn os_version() -> Option<String> {
    command_text("/usr/bin/sw_vers", &["-productVersion"])
}

#[cfg(target_os = "linux")]
fn os_version() -> Option<String> {
    system_text_file("/proc/sys/kernel/osrelease").map(|text| text.trim().to_owned())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn os_version() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn cpu() -> Option<String> {
    command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"])
}

#[cfg(target_os = "linux")]
fn cpu() -> Option<String> {
    let text = system_text_file("/proc/cpuinfo")?;
    for field in ["model name", "Hardware", "Processor"] {
        if let Some(value) = text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == field).then(|| value.trim().to_owned())
        }) {
            return Some(value);
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn cpu() -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn system_text_file(path: &str) -> Option<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(128 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 128 * 1024 {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(target_os = "macos")]
fn resident_memory() -> (Option<u64>, &'static str) {
    let pid = std::process::id().to_string();
    let resident_bytes = command_text("/bin/ps", &["-o", "rss=", "-p", &pid])
        .and_then(|text| text.parse::<u64>().ok())
        .and_then(|kibibytes| kibibytes.checked_mul(1024));
    (resident_bytes, "macos_ps_rss_kib")
}

#[cfg(target_os = "linux")]
fn resident_memory() -> (Option<u64>, &'static str) {
    let resident_bytes = system_text_file("/proc/self/status").and_then(|text| {
        text.lines().find_map(|line| {
            let value = line.strip_prefix("VmRSS:")?;
            let mut fields = value.split_whitespace();
            let kibibytes = fields.next()?.parse::<u64>().ok()?;
            if fields.next()? != "kB" {
                return None;
            }
            kibibytes.checked_mul(1024)
        })
    });
    (resident_bytes, "linux_proc_self_status_vmrss_kib")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn resident_memory() -> (Option<u64>, &'static str) {
    (None, "unavailable")
}

fn memory() -> Memory {
    let (resident_bytes, source) = resident_memory();
    Memory {
        measurement: "process_resident_set_after_benchmark",
        source,
        resident_bytes,
        status: if resident_bytes.is_some() {
            "available"
        } else {
            "unavailable"
        },
    }
}

#[cfg(test)]
mod tests {
    use super::percentile;

    #[test]
    fn percentiles_use_nearest_rank() {
        assert_eq!(percentile(&[], 50), None);
        assert_eq!(percentile(&[9], 99), Some(9));
        let samples: Vec<_> = (1..=100).collect();
        assert_eq!(percentile(&samples, 50), Some(50));
        assert_eq!(percentile(&samples, 95), Some(95));
        assert_eq!(percentile(&samples, 99), Some(99));
        assert_eq!(percentile(&[10, 20, 30], 50), Some(20));
    }
}
