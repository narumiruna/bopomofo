use crate::{Error, Result};
use ime_core::Lexicon;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub(crate) const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
pub(crate) struct LexiconSummary {
    pub(crate) format: &'static str,
    pub(crate) sha256: String,
    pub(crate) entries: usize,
    pub(crate) bytes: usize,
}

pub(crate) struct LoadedLexicon {
    pub(crate) data: Arc<Lexicon>,
    pub(crate) summary: LexiconSummary,
    pub(crate) initialization_ns: u64,
}

pub(crate) fn read_input(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| Error::new("input_open_failed"))?;
    let mut bytes = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("input_read_failed"))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::new("input_byte_limit"));
    }
    Ok(bytes)
}

pub(crate) fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

pub(crate) fn load_lexicon(path: &Path) -> Result<LoadedLexicon> {
    let bytes = read_input(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::new("invalid_lexicon_encoding"))?;
    let sha256 = checksum(&bytes);
    let start = Instant::now();
    // Core owns all schema and reading validation. Never display its source error.
    let data = Arc::new(Lexicon::parse(text).map_err(|_| Error::new("invalid_lexicon"))?);
    let initialization_ns = elapsed_ns(start);
    let summary = LexiconSummary {
        format: "bopomofo-lexicon-v1",
        sha256,
        entries: data.entries().len(),
        bytes: bytes.len(),
    };
    Ok(LoadedLexicon {
        data,
        summary,
        initialization_ns,
    })
}

pub(crate) fn build_lexicon(input: &Path, destination: &Path) -> Result<LexiconSummary> {
    // Finish reading and validation before opening a destination-side file. This
    // also permits input and destination to name the same file.
    let lexicon = load_lexicon(input)?;
    let canonical = lexicon.data.to_text();
    if canonical.len() > MAX_INPUT_BYTES {
        return Err(Error::new("output_byte_limit"));
    }
    let summary = LexiconSummary {
        format: "bopomofo-lexicon-v1",
        sha256: checksum(canonical.as_bytes()),
        entries: lexicon.data.entries().len(),
        bytes: canonical.len(),
    };
    StagedFile::create(destination)?.commit(destination, canonical.as_bytes())?;
    Ok(summary)
}

struct StagedFile {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl StagedFile {
    fn create(destination: &Path) -> Result<Self> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        for _ in 0..128 {
            let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".ime-cli-{}-{sequence}.tmp", std::process::id()));
            if path.file_name() == destination.file_name() {
                continue;
            }
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(Error::new("output_stage_failed")),
            }
        }
        Err(Error::new("output_stage_failed"))
    }

    fn commit(mut self, destination: &Path, bytes: &[u8]) -> Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| Error::new("output_stage_failed"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::new("output_write_failed"))?;
        drop(self.file.take());
        // Same-directory rename is the only operation that changes destination.
        fs::rename(&self.path, destination).map_err(|_| Error::new("output_replace_failed"))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}
