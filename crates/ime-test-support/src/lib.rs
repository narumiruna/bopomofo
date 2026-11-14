//! Test-only dictionary compilation. Never used by a native runtime.
use chewing::dictionary::{DictionaryBuilder, DictionaryUsage, TrieBuilder};
use ime_core::Assets;
use ime_core::assets::{
    ASSET_FORMAT, AssetBuffer, DICTIONARY_NAME, ENGINE_VERSION, FileManifest, Manifest, checksum,
};
use std::sync::Arc;

pub fn parts(source: &str) -> (Vec<u8>, Vec<u8>) {
    let mut builder = TrieBuilder::new();
    builder.set_usage(DictionaryUsage::BuiltIn);
    for line in source
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        let reading = fields[0]
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect::<Vec<_>>();
        builder
            .insert(
                &reading,
                (fields[1], fields[2].parse::<u32>().unwrap()).into(),
            )
            .unwrap();
    }
    let mut data = Vec::new();
    builder.write(&mut data).unwrap();
    let manifest = Manifest {
        format: ASSET_FORMAT.into(),
        engine: ENGINE_VERSION.into(),
        builder: ASSET_FORMAT.into(),
        data_version: "test".into(),
        source_sha256: checksum(source.as_bytes()),
        files: vec![FileManifest {
            name: DICTIONARY_NAME.into(),
            bytes: data.len(),
            sha256: checksum(&data),
        }],
    };
    (data, serde_json::to_vec(&manifest).unwrap())
}
pub fn assets(source: &str) -> Arc<Assets> {
    let (data, manifest) = parts(source);
    Arc::new(
        Assets::verify(
            &manifest,
            &[AssetBuffer {
                name: DICTIONARY_NAME,
                data: &data,
            }],
        )
        .unwrap(),
    )
}
pub fn fixture() -> Arc<Assets> {
    assets(include_str!("../../../fixtures/lexicon.tsv"))
}
