use ime_core::Assets;
use ime_core::assets::{
    AssetBuffer, DICTIONARY_NAME, MAX_ASSET_BYTES, MAX_MANIFEST_BYTES, checksum,
};
use ime_test_support::parts;
use serde_json::{Value, json};
#[test]
fn valid_copied_bytes_and_metadata() {
    let (mut data, manifest) = parts("ㄧ\t一\t1\n");
    let a = Assets::verify(
        &manifest,
        &[AssetBuffer {
            name: DICTIONARY_NAME,
            data: &data,
        }],
    )
    .unwrap();
    data.fill(0);
    assert_eq!(a.entries, 1);
}
#[test]
fn missing_extra_duplicate_and_traversal_files_are_rejected() {
    let (data, m) = parts("ㄧ\t一\t1\n");
    let f = AssetBuffer {
        name: DICTIONARY_NAME,
        data: &data,
    };
    assert!(Assets::verify(&m, &[]).is_err());
    assert!(Assets::verify(&m, &[f, f]).is_err());
    assert!(
        Assets::verify(
            &m,
            &[AssetBuffer {
                name: "../dictionary.dat",
                data: &data
            }]
        )
        .is_err()
    );
}
#[test]
fn manifest_identity_hash_bounds_and_schema_are_checked() {
    let (data, m) = parts("ㄧ\t一\t1\n");
    let original: Value = serde_json::from_slice(&m).unwrap();
    for (path, value) in [
        ("format", json!("wrong")),
        ("engine", json!("chewing-0.14")),
        ("builder", json!("wrong")),
        ("source_sha256", json!("bad")),
        ("data_version", json!("")),
    ] {
        let mut m = original.clone();
        m[path] = value;
        assert!(
            Assets::verify(
                &serde_json::to_vec(&m).unwrap(),
                &[AssetBuffer {
                    name: DICTIONARY_NAME,
                    data: &data
                }]
            )
            .is_err()
        );
    }
    for (field, value) in [
        ("bytes", json!(0)),
        ("bytes", json!(MAX_ASSET_BYTES + 1)),
        ("bytes", json!(u64::MAX)),
        ("sha256", json!("0".repeat(64))),
        ("name", json!("../dictionary.dat")),
    ] {
        let mut m = original.clone();
        m["files"][0][field] = value;
        assert!(
            Assets::verify(
                &serde_json::to_vec(&m).unwrap(),
                &[AssetBuffer {
                    name: DICTIONARY_NAME,
                    data: &data
                }]
            )
            .is_err()
        );
    }
    assert!(
        Assets::verify(
            &vec![b' '; MAX_MANIFEST_BYTES + 1],
            &[AssetBuffer {
                name: DICTIONARY_NAME,
                data: &data
            }]
        )
        .is_err()
    );
    let mut m = original.clone();
    m["extra"] = json!(true);
    assert!(
        Assets::verify(
            &serde_json::to_vec(&m).unwrap(),
            &[AssetBuffer {
                name: DICTIONARY_NAME,
                data: &data
            }]
        )
        .is_err()
    );
    for bad in [vec![], vec![0, 1, 2], b"garbage".to_vec()] {
        let mut m = original.clone();
        m["files"][0]["bytes"] = json!(bad.len());
        m["files"][0]["sha256"] = json!(checksum(&bad));
        assert!(
            Assets::verify(
                &serde_json::to_vec(&m).unwrap(),
                &[AssetBuffer {
                    name: DICTIONARY_NAME,
                    data: &bad
                }]
            )
            .is_err()
        );
    }
}
#[test]
fn checksum_valid_cycles_empty_nodes_and_bad_leaf_ranges_are_rejected() {
    use der::{
        Decode, Reader, SliceReader,
        asn1::{AnyRef, OctetStringRef},
    };
    let (original, manifest) = parts("ㄧ\t一\t1\n");
    let root = AnyRef::from_der(&original).unwrap();
    let mut r = SliceReader::new(root.value()).unwrap();
    for _ in 0..3 {
        let _: AnyRef<'_> = r.decode().unwrap();
    }
    let index: OctetStringRef<'_> = r.decode().unwrap();
    let offset = index.as_bytes().as_ptr() as usize - original.as_ptr() as usize;
    for (position, value) in [
        (offset + 8, 1u32.to_be_bytes().to_vec()),
        (offset + 12, vec![0, 0]),
        (offset + 16, u32::MAX.to_be_bytes().to_vec()),
    ] {
        let mut data = original.clone();
        data[position..position + value.len()].copy_from_slice(&value);
        let mut m: Value = serde_json::from_slice(&manifest).unwrap();
        m["files"][0]["sha256"] = json!(checksum(&data));
        assert!(
            Assets::verify(
                &serde_json::to_vec(&m).unwrap(),
                &[AssetBuffer {
                    name: DICTIONARY_NAME,
                    data: &data
                }]
            )
            .is_err()
        );
    }
}

#[test]
fn incompatible_text_mapping_is_rejected() {
    let (data, m) = parts("ㄧ\t𠀀é😀\t1\n");
    assert!(
        Assets::verify(
            &m,
            &[AssetBuffer {
                name: DICTIONARY_NAME,
                data: &data
            }]
        )
        .is_err()
    );
}
