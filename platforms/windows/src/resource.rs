//! Bounded activation-time loading of an installation's lexicon artifact.
use ime_core::{Lexicon, lexicon::MAX_SOURCE_BYTES};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResourceError {
    Read,
    TooLarge,
    Checksum,
    Lexicon,
}

fn read_bounded(reader: impl Read, maximum: usize) -> Result<String, ResourceError> {
    let mut text = String::new();
    reader
        .take(maximum as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| ResourceError::Read)?;
    if text.len() > maximum {
        return Err(ResourceError::TooLarge);
    }
    Ok(text)
}

fn validate(source: &str, checksum: &str) -> Result<Arc<Lexicon>, ResourceError> {
    if checksum.trim().len() != 64
        || checksum.trim() != format!("{:x}", Sha256::digest(source.as_bytes()))
    {
        return Err(ResourceError::Checksum);
    }
    let lexicon = Lexicon::parse(source).map_err(|_| ResourceError::Lexicon)?;
    if lexicon.entries().is_empty() {
        return Err(ResourceError::Lexicon);
    }
    Ok(Arc::new(lexicon))
}

pub(crate) fn load(directory: &Path) -> Result<Arc<Lexicon>, ResourceError> {
    let source = read_bounded(
        std::fs::File::open(directory.join("lexicon.tsv")).map_err(|_| ResourceError::Read)?,
        MAX_SOURCE_BYTES,
    )?;
    let checksum = read_bounded(
        std::fs::File::open(directory.join("lexicon.sha256")).map_err(|_| ResourceError::Read)?,
        66,
    )?;
    validate(&source, &checksum)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_is_bounded_and_invalid_encoding_fails() {
        assert_eq!(read_bounded("abc".as_bytes(), 3), Ok("abc".into()));
        assert_eq!(
            read_bounded("abcd".as_bytes(), 3),
            Err(ResourceError::TooLarge)
        );
        assert_eq!(read_bounded([0xff].as_slice(), 3), Err(ResourceError::Read));
    }

    #[test]
    fn missing_artifact_checksum_mismatch_and_invalid_lexicon_fail_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(matches!(
            load(&root.join("no-such-installation")),
            Err(ResourceError::Read)
        ));
        let source = std::fs::read_to_string(root.join("../../fixtures/lexicon.tsv")).unwrap();
        let checksum = format!("{:x}", Sha256::digest(source.as_bytes()));
        assert!(!validate(&source, &checksum).unwrap().entries().is_empty());
        assert!(matches!(
            validate(&source, &"0".repeat(64)),
            Err(ResourceError::Checksum)
        ));
        for source in ["wrong header\n", "# bopomofo-lexicon-v1\n"] {
            let checksum = format!("{:x}", Sha256::digest(source.as_bytes()));
            assert!(matches!(
                validate(source, &checksum),
                Err(ResourceError::Lexicon)
            ));
        }
    }
}
