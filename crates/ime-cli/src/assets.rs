//! Atomic, offline construction of the single runtime asset package.
use crate::{Error, Result, files, source::Source};
use ime_core::{
    Assets,
    assets::{
        ASSET_FORMAT, AssetBuffer, DICTIONARY_NAME, ENGINE_VERSION, FileManifest, MANIFEST_NAME,
        Manifest,
    },
};
use std::{fs, path::Path, sync::Arc, time::Instant};
const NOTICE: &str = include_str!("../../../data/libchewing/NOTICE.md");
const LGPL: &str = include_str!("../../../data/libchewing/COPYING");
const AUTHORS: &str = include_str!("../../../data/libchewing/AUTHORS");
const AGPL: &str = include_str!("../../../LICENSE");

pub fn build(input: &Path, destination: &Path) -> Result<files::LexiconSummary> {
    if destination.exists() {
        return Err(Error::new("asset_destination_exists"));
    }
    let (source, source_hash, version, license, notice, data_license) = if input.is_dir() {
        let word = files::read_input(&input.join("word.csv"))?;
        let tsi = files::read_input(&input.join("tsi.csv"))?;
        if files::checksum(&word)
            != "da55b8e599c1389bc486453554f3410cf9c621d0ffff0ce38855698d26b3892a"
            || files::checksum(&tsi)
                != "c889a1ac3ae1901b3f8f62748bc41b958f010bf995f7f88dbaf9e3494f341428"
        {
            return Err(Error::new("upstream_source_checksum"));
        }
        let mut identity = b"word.csv\0".to_vec();
        identity.extend_from_slice(&word);
        identity.extend_from_slice(b"\0tsi.csv\0");
        identity.extend_from_slice(&tsi);
        let source = Source::upstream(
            std::str::from_utf8(&word).map_err(|_| Error::new("source_encoding"))?,
            std::str::from_utf8(&tsi).map_err(|_| Error::new("source_encoding"))?,
        )
        .map_err(|_| Error::new("invalid_upstream_source"))?;
        (
            source,
            files::checksum(&identity),
            "bfba418ea8a4bd2fe10b9bf80b4bf81ec01af5f4".to_owned(),
            "LGPL-2.1-or-later",
            NOTICE.to_owned(),
            LGPL,
        )
    } else {
        let loaded = files::load_lexicon(input)?;
        let hash = loaded.summary.sha256;
        let cns = hash == "f279ff2faa8bc40814e300253cda97c162003a0b6650fdfa0484f4075d0b300a";
        let fixture = [
            include_bytes!("../../../fixtures/lexicon.tsv").as_slice(),
            include_bytes!("../../../fixtures/lexicon-quality/lexicon.tsv").as_slice(),
            include_bytes!("../../../fixtures/unicode/lexicon.tsv").as_slice(),
        ]
        .into_iter()
        .any(|bytes| files::checksum(bytes) == hash);
        let notice = if cns {
            include_str!("../../../data/cns11643/NOTICE.txt").to_owned()
                + "\nThe five self-authored seed phrases remain AGPL-3.0-only.\n"
        } else {
            "Caller-supplied authoring data. Verify its source and license before distribution. Repository fixtures are AGPL-3.0-only.\n".into()
        };
        (
            (*loaded.data).clone(),
            hash.clone(),
            hash,
            if cns {
                "OGDL-Taiwan-1.0 AND AGPL-3.0-only"
            } else if fixture {
                "AGPL-3.0-only"
            } else {
                "LicenseRef-Caller-Supplied"
            },
            notice,
            if cns {
                include_str!("../../../data/cns11643/LICENSE.txt")
            } else if fixture {
                AGPL
            } else {
                "This command does not establish the source license or grant redistribution rights. Supply and preserve the actual data license and notices before distribution. Do not treat the project AGPL license as a data license.\n"
            },
        )
    };
    let data = source
        .dictionary(license)
        .map_err(|_| Error::new("incompatible_dictionary_source"))?;
    let manifest = Manifest {
        format: ASSET_FORMAT.into(),
        engine: ENGINE_VERSION.into(),
        builder: ASSET_FORMAT.into(),
        data_version: version,
        source_sha256: source_hash,
        files: vec![FileManifest {
            name: DICTIONARY_NAME.into(),
            bytes: data.len(),
            sha256: files::checksum(&data),
        }],
    };
    let mut manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|_| Error::new("manifest_encoding"))?;
    manifest.push(b'\n');
    let verified = Assets::verify(
        &manifest,
        &[AssetBuffer {
            name: DICTIONARY_NAME,
            data: &data,
        }],
    )
    .map_err(|_| Error::new("invalid_built_assets"))?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = parent.join(format!(".bopomofo-assets-{}", std::process::id()));
    fs::create_dir(&staging).map_err(|_| Error::new("asset_stage_failed"))?;
    let result = (|| {
        for (name, bytes) in [
            (DICTIONARY_NAME, data.as_slice()),
            (MANIFEST_NAME, manifest.as_slice()),
            ("Project-LICENSE.txt", AGPL.as_bytes()),
            ("Engine-LICENSE.txt", LGPL.as_bytes()),
            ("Engine-AUTHORS.txt", AUTHORS.as_bytes()),
            ("Engine-NOTICE.txt", NOTICE.as_bytes()),
            ("Data-NOTICE.txt", notice.as_bytes()),
            ("Data-LICENSE.txt", data_license.as_bytes()),
        ] {
            fs::write(staging.join(name), bytes).map_err(|_| Error::new("asset_write_failed"))?;
        }
        fs::rename(&staging, destination).map_err(|_| Error::new("asset_replace_failed"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    Ok(files::LexiconSummary {
        format: ASSET_FORMAT,
        sha256: files::checksum(&manifest),
        entries: verified.entries,
        bytes: data.len(),
    })
}

pub fn load(path: &Path) -> Result<files::LoadedAssets> {
    let data = files::read_input(&path.join(DICTIONARY_NAME))?;
    let manifest = files::read_bounded(
        &path.join(MANIFEST_NAME),
        ime_core::assets::MAX_MANIFEST_BYTES,
    )?;
    let start = Instant::now();
    let data = Arc::new(
        Assets::verify(
            &manifest,
            &[AssetBuffer {
                name: DICTIONARY_NAME,
                data: &data,
            }],
        )
        .map_err(|_| Error::new("invalid_assets"))?,
    );
    Ok(files::LoadedAssets {
        summary: files::LexiconSummary {
            format: ASSET_FORMAT,
            sha256: files::checksum(&manifest),
            entries: data.entries,
            bytes: data.manifest.files[0].bytes,
        },
        data,
        initialization_ns: files::elapsed_ns(start),
    })
}
