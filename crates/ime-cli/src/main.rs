mod benchmark;
mod files;
mod replay;

use serde::Serialize;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "Usage:
  ime-cli replay <lexicon.tsv> <case.jsonl>
  ime-cli validate-lexicon <file>
  ime-cli build-lexicon <input.tsv> <output.tsv>
  ime-cli benchmark <lexicon.tsv> <case.jsonl> <iterations>

Replay expectations: object fields are subsets; arrays have equal lengths;
primitive values match exactly. Each JSONL record has event and optional expect.
Inputs are limited to 32 MiB each and 100000 events. Benchmark requires a release
build, 1..=10000 iterations, and at most 1000000 total fixture events.
Benchmark results apply only to the supplied fixture, not to product performance.";

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub(crate) struct Error {
    kind: &'static str,
    line: Option<usize>,
    usage: bool,
}

impl Error {
    pub(crate) fn new(kind: &'static str) -> Self {
        Self {
            kind,
            line: None,
            usage: false,
        }
    }

    pub(crate) fn at_line(kind: &'static str, line: usize) -> Self {
        Self {
            kind,
            line: Some(line),
            usage: false,
        }
    }

    fn arguments(kind: &'static str) -> Self {
        Self {
            kind,
            line: None,
            usage: true,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {line}: ")?;
        }
        f.write_str(self.kind)
    }
}

enum Command {
    Help,
    Replay(PathBuf, PathBuf),
    ValidateLexicon(PathBuf),
    BuildLexicon(PathBuf, PathBuf),
    Benchmark(PathBuf, PathBuf, usize),
}

impl Command {
    fn parse(args: Vec<OsString>) -> Result<Self> {
        let Some(name) = args.first().and_then(|arg| arg.to_str()) else {
            return Err(Error::arguments("missing_or_invalid_command"));
        };
        let command = match (name, args.len()) {
            ("--help" | "-h", 1) => Self::Help,
            ("replay", 3) => Self::Replay(args[1].clone().into(), args[2].clone().into()),
            ("validate-lexicon", 2) => Self::ValidateLexicon(args[1].clone().into()),
            ("build-lexicon", 3) => {
                Self::BuildLexicon(args[1].clone().into(), args[2].clone().into())
            }
            ("benchmark", 4) => {
                let iterations = args[3]
                    .to_str()
                    .and_then(|arg| arg.parse::<usize>().ok())
                    .filter(|count| (1..=benchmark::MAX_ITERATIONS).contains(count))
                    .ok_or_else(|| Error::arguments("invalid_iterations"))?;
                Self::Benchmark(args[1].clone().into(), args[2].clone().into(), iterations)
            }
            (
                "--help" | "-h" | "replay" | "validate-lexicon" | "build-lexicon" | "benchmark",
                _,
            ) => {
                return Err(Error::arguments("invalid_argument_count"));
            }
            _ => return Err(Error::arguments("unknown_command")),
        };
        Ok(command)
    }

    fn run(self, output: &mut impl Write) -> Result<()> {
        match self {
            Self::Help => {
                writeln!(output, "{USAGE}").map_err(|_| Error::new("output_write_failed"))
            }
            Self::Replay(lexicon_path, case_path) => {
                let lexicon = files::load_lexicon(&lexicon_path)?;
                let case = replay::Case::load(&case_path)?;
                replay::run(lexicon.data, &case, output)
            }
            Self::ValidateLexicon(path) => {
                let lexicon = files::load_lexicon(&path)?;
                write_json(output, &lexicon.summary)
            }
            Self::BuildLexicon(input, destination) => {
                let summary = files::build_lexicon(&input, &destination)?;
                write_json(output, &summary)
            }
            Self::Benchmark(lexicon, case, iterations) => {
                benchmark::run(&lexicon, &case, iterations, output)
            }
        }
    }
}

pub(crate) fn write_json(output: &mut impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *output, value).map_err(|_| Error::new("output_write_failed"))?;
    output
        .write_all(b"\n")
        .map_err(|_| Error::new("output_write_failed"))
}

fn main() -> ExitCode {
    let stdout = io::stdout();
    let mut output = io::BufWriter::new(stdout.lock());
    let result = Command::parse(std::env::args_os().skip(1).collect())
        .and_then(|command| command.run(&mut output))
        .and_then(|()| {
            output
                .flush()
                .map_err(|_| Error::new("output_write_failed"))
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            if error.usage {
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}
