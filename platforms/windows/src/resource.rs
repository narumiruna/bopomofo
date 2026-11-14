//! Bounded activation-time reads of one explicit v2 asset package.
use ime_core::{
    Assets,
    assets::{AssetBuffer, DICTIONARY_NAME, MANIFEST_NAME, MAX_ASSET_BYTES, MAX_MANIFEST_BYTES},
};
use std::{io::Read, path::Path, sync::Arc};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResourceError {
    Read,
    TooLarge,
    Assets,
}
fn read_bounded(reader: impl Read, max: usize) -> Result<Vec<u8>, ResourceError> {
    let mut bytes = Vec::new();
    reader
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ResourceError::Read)?;
    if bytes.len() > max {
        return Err(ResourceError::TooLarge);
    }
    Ok(bytes)
}
fn validate(data: &[u8], manifest: &[u8]) -> Result<Arc<Assets>, ResourceError> {
    Assets::verify(
        manifest,
        &[AssetBuffer {
            name: DICTIONARY_NAME,
            data,
        }],
    )
    .map(Arc::new)
    .map_err(|_| ResourceError::Assets)
}
pub(crate) fn load(directory: &Path) -> Result<Arc<Assets>, ResourceError> {
    let data = read_bounded(
        std::fs::File::open(directory.join(DICTIONARY_NAME)).map_err(|_| ResourceError::Read)?,
        MAX_ASSET_BYTES,
    )?;
    let manifest = read_bounded(
        std::fs::File::open(directory.join(MANIFEST_NAME)).map_err(|_| ResourceError::Read)?,
        MAX_MANIFEST_BYTES,
    )?;
    validate(&data, &manifest)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_binary_reads() {
        assert_eq!(read_bounded([0xff].as_slice(), 1), Ok(vec![0xff]));
        assert_eq!(
            read_bounded(b"abcd".as_slice(), 3),
            Err(ResourceError::TooLarge)
        );
    }
    #[test]
    fn missing_corrupt_and_mixed_version_assets_fail_closed() {
        assert!(matches!(
            load(Path::new("no-such-assets-directory")),
            Err(ResourceError::Read)
        ));
        let (mut data, manifest) = ime_test_support::parts("ㄧ\t一\t1\n");
        assert_eq!(validate(&data, &manifest).unwrap().entries, 1);
        data[0] ^= 1;
        assert!(matches!(
            validate(&data, &manifest),
            Err(ResourceError::Assets)
        ));
        assert!(matches!(validate(&data, b"{}"), Err(ResourceError::Assets)));
    }
}
