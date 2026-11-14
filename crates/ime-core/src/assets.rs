//! Versioned, bounded libchewing assets. No filesystem or environment lookup.
use chewing::dictionary::{
    Dictionary, DictionaryInfo, DictionaryUsage, Entries, LookupStrategy, Phrase, Trie,
};
use chewing::zhuyin::Syllable;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path, sync::Arc};

pub const ENGINE_VERSION: &str = "chewing-0.13.1";
pub const ASSET_FORMAT: &str = "bopomofo-assets-v1";
pub const DICTIONARY_NAME: &str = "dictionary.dat";
pub const MANIFEST_NAME: &str = "assets.json";
pub const MAX_ASSET_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
pub const MAX_ENTRIES: usize = 250_000;
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_PHRASE_SYLLABLES: usize = 16;
pub const MAX_TEXT_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileManifest {
    pub name: String,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub engine: String,
    pub builder: String,
    pub data_version: String,
    pub source_sha256: String,
    pub files: Vec<FileManifest>,
}

#[derive(Clone, Copy)]
pub struct AssetBuffer<'a> {
    pub name: &'a str,
    pub data: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct Assets {
    pub(crate) dictionary: Arc<Trie>,
    pub manifest: Manifest,
    pub entries: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetError(pub &'static str);
impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for AssetError {}

pub fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Assets {
    /// Copies and validates every supplied byte before a context can use it.
    pub fn verify(manifest: &[u8], files: &[AssetBuffer<'_>]) -> Result<Self, AssetError> {
        std::panic::catch_unwind(|| Self::verify_inner(manifest, files))
            .map_err(|_| AssetError("invalid_assets"))?
    }
    fn verify_inner(manifest: &[u8], files: &[AssetBuffer<'_>]) -> Result<Self, AssetError> {
        let fail = || AssetError("invalid_assets");
        if manifest.len() > MAX_MANIFEST_BYTES || files.len() != 1 {
            return Err(fail());
        }
        let manifest: Manifest = serde_json::from_slice(manifest).map_err(|_| fail())?;
        if manifest.format != ASSET_FORMAT
            || manifest.engine != ENGINE_VERSION
            || manifest.builder != ASSET_FORMAT
            || manifest.data_version.is_empty()
            || manifest.data_version.len() > 128
            || manifest.data_version.chars().any(char::is_control)
            || !digest(&manifest.source_sha256)
            || manifest.files.len() != 1
        {
            return Err(fail());
        }
        let file = &manifest.files[0];
        let buffer = &files[0];
        if file.name != DICTIONARY_NAME
            || buffer.name != DICTIONARY_NAME
            || file.bytes == 0
            || file.bytes > MAX_ASSET_BYTES
            || buffer.data.len() != file.bytes
            || !digest(&file.sha256)
            || checksum(buffer.data) != file.sha256
        {
            return Err(fail());
        }
        let entries = validate_dictionary(buffer.data).map_err(|_| fail())?;
        let mut dictionary = Trie::new(buffer.data).map_err(|_| fail())?;
        dictionary.set_usage(DictionaryUsage::BuiltIn);
        Ok(Self {
            dictionary: Arc::new(dictionary),
            manifest,
            entries,
        })
    }
}

// Validate the pinned upstream DER/BFS format before using its lazy iterator.
// In particular, a checksum-valid index must not contain cycles, overlaps, empty
// internal nodes or unbounded phrase lists. This does not implement lookups.
fn validate_dictionary(bytes: &[u8]) -> Result<usize, ()> {
    use der::{
        Decode, Reader, SliceReader, Tag, Tagged,
        asn1::{AnyRef, OctetStringRef, Utf8StringRef},
    };
    let file = AnyRef::from_der(bytes).map_err(|_| ())?;
    if file.tag() != Tag::Sequence {
        return Err(());
    }
    let mut reader = SliceReader::new(file.value()).map_err(|_| ())?;
    let magic: Utf8StringRef<'_> = reader.decode().map_err(|_| ())?;
    let version: u8 = reader.decode().map_err(|_| ())?;
    let info: AnyRef<'_> = reader.decode().map_err(|_| ())?;
    let index: OctetStringRef<'_> = reader.decode().map_err(|_| ())?;
    let phrases: AnyRef<'_> = reader.decode().map_err(|_| ())?;
    reader.finish(()).map_err(|_| ())?;
    if magic.as_str() != "CHEW"
        || version != 0
        || info.tag() != Tag::Sequence
        || phrases.tag() != Tag::Sequence
    {
        return Err(());
    }
    let index = index.as_bytes();
    let phrases = phrases.value();
    if index.len() < 16 || !index.len().is_multiple_of(8) {
        return Err(());
    }
    let nodes = index.len() / 8;
    let mut depths = vec![0u8; nodes];
    let mut next_child = 1;
    let mut next_data = 0;
    let mut entries = 0;
    for (i, node) in index.chunks_exact(8).enumerate() {
        let begin = u32::from_be_bytes(node[..4].try_into().map_err(|_| ())?) as usize;
        let len = u16::from_be_bytes(node[4..6].try_into().map_err(|_| ())?) as usize;
        let syllable = u16::from_be_bytes(node[6..].try_into().map_err(|_| ())?);
        let end = begin.checked_add(len).ok_or(())?;
        if i == 0 || syllable != 0 {
            if (i == 0 && syllable != 0)
                || begin != next_child
                || begin <= i
                || len == 0
                || end > nodes
            {
                return Err(());
            }
            let mut previous = 0;
            for child in begin..end {
                let value = u16::from_be_bytes(
                    index[child * 8 + 6..child * 8 + 8]
                        .try_into()
                        .map_err(|_| ())?,
                );
                if child == begin && value == 0 {
                    if depths[i] == 0 {
                        return Err(());
                    }
                    depths[child] = depths[i];
                } else {
                    if value == 0 || value <= previous {
                        return Err(());
                    }
                    let reading = Syllable::try_from(value).map_err(|_| ())?;
                    if reading.is_empty()
                        || !reading
                            .to_string()
                            .parse::<Syllable>()
                            .is_ok_and(|v| v == reading)
                    {
                        return Err(());
                    }
                    let depth = usize::from(depths[i]) + 1;
                    if depth > MAX_PHRASE_SYLLABLES {
                        return Err(());
                    }
                    depths[child] = depth as u8;
                    previous = value;
                }
            }
            next_child = end;
        } else {
            if begin != next_data || len == 0 || end > phrases.len() {
                return Err(());
            }
            let mut list = SliceReader::new(&phrases[begin..end]).map_err(|_| ())?;
            let mut count = 0;
            let mut texts = std::collections::BTreeSet::new();
            while !list.is_finished() {
                let record: AnyRef<'_> = list.decode().map_err(|_| ())?;
                if record.tag() != Tag::Sequence {
                    return Err(());
                }
                let mut fields = SliceReader::new(record.value()).map_err(|_| ())?;
                let text: Utf8StringRef<'_> = fields.decode().map_err(|_| ())?;
                let _: u32 = fields.decode().map_err(|_| ())?;
                fields.finish(()).map_err(|_| ())?; // No persisted user-history extension.
                let text = text.as_str();
                if text.is_empty()
                    || text.len() > MAX_TEXT_BYTES
                    || text.chars().any(char::is_control)
                    || text.chars().count() != usize::from(depths[i])
                    || !texts.insert(text)
                {
                    return Err(());
                }
                count += 1;
                entries += 1;
                if count > MAX_CANDIDATES || entries > MAX_ENTRIES {
                    return Err(());
                }
            }
            next_data = end;
        }
    }
    if next_child != nodes || next_data != phrases.len() || entries == 0 {
        return Err(());
    }
    Ok(entries)
}

/// Shares only an immutable upstream dictionary, never an Editor or user state.
#[derive(Debug)]
pub(crate) struct ReadOnlyDictionary(pub Arc<Trie>);
impl Dictionary for ReadOnlyDictionary {
    fn lookup(&self, s: &[Syllable], strategy: LookupStrategy) -> Vec<Phrase> {
        self.0.lookup(s, strategy)
    }
    fn entries(&self) -> Entries<'_> {
        self.0.entries()
    }
    fn about(&self) -> DictionaryInfo {
        self.0.about()
    }
    fn path(&self) -> Option<&Path> {
        None
    }
    fn set_usage(&mut self, _: DictionaryUsage) {}
}
