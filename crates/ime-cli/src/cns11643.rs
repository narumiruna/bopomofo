//! Offline import of the pinned CNS11643 Taiwan common/less-common subset.
//! Source preparation and data rights are recorded in data/cns11643/README.md.

use crate::{Error, Result, files};
use ime_core::parser::Syllable;
use ime_core::{Lexicon, lexicon::HEADER};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    version: String,
    phonetic_sha256: String,
    unicode_sha256: String,
}

#[derive(Serialize)]
pub(crate) struct ImportSummary {
    source: Manifest,
    seed_sha256: String,
    characters: usize,
    character_readings: usize,
    excluded_readings: usize,
    phrase_entries: usize,
    ignored_seed_characters: usize,
    #[serde(flatten)]
    lexicon: files::LexiconSummary,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Code {
    plane: u32,
    position: u32,
}

impl Code {
    fn parse(text: &str) -> Option<Self> {
        let (plane, position) = text.split_once('-')?;
        if !plane.bytes().all(|c| c.is_ascii_digit())
            || position.len() != 4
            || !position.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return None;
        }
        let code = Self {
            plane: plane.parse().ok()?,
            position: u32::from_str_radix(position, 16).ok()?,
        };
        if !(0x21..=0x7e).contains(&(code.position >> 8))
            || !(0x21..=0x7e).contains(&(code.position & 0xff))
        {
            return None;
        }
        Some(code)
    }

    fn in_scope(self) -> bool {
        match self.plane {
            1 => (0x4421..=0x7d4b).contains(&self.position),
            2 => (0x2121..=0x7245).contains(&self.position),
            _ => false,
        }
    }

    fn score(self) -> u32 {
        // Prefer the common plane, then CNS stroke/radical order. These are
        // deterministic fallback scores, not corpus occurrence frequencies.
        let offset = (self.position >> 8) - 0x21;
        let column = (self.position & 0xff) - 0x21;
        30_000 - (self.plane * 94 * 94 + offset * 94 + column)
    }
}

fn record(line: &str, number: usize) -> Result<(Code, &str)> {
    let (key, value) = line
        .split_once('\t')
        .filter(|(_, value)| !value.is_empty() && !value.contains('\t'))
        .ok_or_else(|| Error::at_line("invalid_cns_record", number))?;
    let code = Code::parse(key).ok_or_else(|| Error::at_line("invalid_cns_code", number))?;
    if !code.in_scope() {
        return Err(Error::at_line("cns_code_out_of_scope", number));
    }
    Ok((code, value))
}

fn unicode_map(source: &str) -> Result<BTreeMap<Code, char>> {
    let mut mappings = BTreeMap::new();
    let mut scalars = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let number = index + 1;
        let (code, value) = record(line, number)?;
        let scalar = value
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
            .then(|| u32::from_str_radix(value, 16).ok())
            .flatten()
            .filter(|value| (0x4e00..=0x9fff).contains(value))
            .and_then(char::from_u32)
            .ok_or_else(|| Error::at_line("invalid_cns_han_mapping", number))?;
        if mappings.insert(code, scalar).is_some() || !scalars.insert(scalar) {
            return Err(Error::at_line("duplicate_cns_mapping", number));
        }
    }
    if mappings.is_empty() {
        return Err(Error::new("empty_cns_mapping"));
    }
    Ok(mappings)
}

fn convert(
    phonetic: &str,
    unicode: &str,
    seed: &Lexicon,
) -> Result<(Lexicon, usize, usize, usize, usize)> {
    let mappings = unicode_map(unicode)?;
    let mut pairs = BTreeMap::new();
    let mut covered = BTreeSet::new();
    let mut excluded = 0;
    for (index, line) in phonetic.lines().enumerate() {
        let number = index + 1;
        let (code, value) = record(line, number)?;
        let text = mappings
            .get(&code)
            .ok_or_else(|| Error::at_line("missing_cns_unicode", number))?;
        // Published 20261028 row 2-3378 / 啐 has an incomplete neutral-tone
        // initial. Keep it in the source snapshot, report its exclusion, and
        // never guess a replacement reading. Other invalid rows remain fatal.
        if code
            == (Code {
                plane: 2,
                position: 0x3378,
            })
            && value == "˙ㄑ"
        {
            excluded += 1;
            if excluded > 1 {
                return Err(Error::at_line("duplicate_cns_exclusion", number));
            }
            continue;
        }
        let reading =
            Syllable::parse(value).map_err(|_| Error::at_line("invalid_cns_syllable", number))?;
        if pairs
            .insert((reading, text.to_string()), code.score())
            .is_some()
        {
            return Err(Error::at_line("duplicate_cns_reading", number));
        }
        covered.insert(code);
    }
    if covered.len() != mappings.len() {
        return Err(Error::new("missing_cns_reading"));
    }
    let character_readings = pairs.len();
    let mut output = format!("{HEADER}\n");
    for ((reading, text), score) in &pairs {
        writeln!(output, "{}\t{text}\t{score}", reading.as_str()).unwrap();
    }
    let mut phrases = 0;
    let mut ignored = 0;
    for entry in seed.entries() {
        if entry.reading.len() == 1 {
            // Fixture-only character readings/scores are not source evidence.
            ignored += 1;
            continue;
        }
        let characters: Vec<_> = entry.text.chars().collect();
        if characters.len() != entry.reading.len()
            || !entry
                .reading
                .iter()
                .zip(&characters)
                .all(|(reading, text)| pairs.contains_key(&(reading.clone(), text.to_string())))
        {
            return Err(Error::new("cns_seed_reading_mismatch"));
        }
        for (index, reading) in entry.reading.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push_str(reading.as_str());
        }
        writeln!(output, "\t{}\t{}", entry.text, entry.frequency).unwrap();
        phrases += 1;
    }
    let lexicon = Lexicon::parse(&output).map_err(|_| Error::new("invalid_cns_lexicon"))?;
    Ok((lexicon, character_readings, phrases, ignored, excluded))
}

pub(crate) fn import(source: &Path, seed: &Path, destination: &Path) -> Result<ImportSummary> {
    if let Ok(destination) = destination.canonicalize() {
        for input in [
            source.join("manifest.json"),
            source.join("phonetic.tsv"),
            source.join("unicode.tsv"),
            seed.to_path_buf(),
        ] {
            if input.canonicalize().ok().as_ref() == Some(&destination) {
                return Err(Error::new("cns_output_is_input"));
            }
        }
    }
    let manifest: Manifest =
        serde_json::from_slice(&files::read_input(&source.join("manifest.json"))?)
            .map_err(|_| Error::new("invalid_cns_manifest"))?;
    if manifest.format != "cns11643-tw-v1"
        || manifest.version.len() != 8
        || !manifest.version.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(Error::new("invalid_cns_manifest"));
    }
    let phonetic = files::read_input(&source.join("phonetic.tsv"))?;
    let unicode = files::read_input(&source.join("unicode.tsv"))?;
    if files::checksum(&phonetic) != manifest.phonetic_sha256
        || files::checksum(&unicode) != manifest.unicode_sha256
    {
        return Err(Error::new("cns_source_checksum_mismatch"));
    }
    let phonetic =
        std::str::from_utf8(&phonetic).map_err(|_| Error::new("invalid_cns_encoding"))?;
    let unicode = std::str::from_utf8(&unicode).map_err(|_| Error::new("invalid_cns_encoding"))?;
    let seed = files::load_lexicon(seed)?;
    let (lexicon, character_readings, phrase_entries, ignored_seed_characters, excluded_readings) =
        convert(phonetic, unicode, &seed.data)?;
    let summary = ImportSummary {
        source: manifest,
        seed_sha256: seed.summary.sha256,
        characters: lexicon
            .entries()
            .iter()
            .filter(|entry| entry.reading.len() == 1)
            .map(|entry| &entry.text)
            .collect::<BTreeSet<_>>()
            .len(),
        character_readings,
        excluded_readings,
        phrase_entries,
        ignored_seed_characters,
        lexicon: files::write_lexicon(destination, &lexicon)?,
    };
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_seed() -> Lexicon {
        Lexicon::parse(&format!("{HEADER}\n")).unwrap()
    }

    #[test]
    fn only_official_common_and_less_common_character_regions_are_in_scope() {
        for code in ["1-4421", "1-7D4B", "2-2121", "2-7245"] {
            assert!(Code::parse(code).unwrap().in_scope());
        }
        for code in ["1-4420", "1-4243", "1-7D4C", "2-7246", "3-2121", "15-2121"] {
            assert!(Code::parse(code).is_none_or(|code| !code.in_scope()));
        }
        for code in ["", "1-212121", "1-217F", "1-2021", "x-4421", "1-44GG"] {
            assert!(Code::parse(code).is_none());
        }
        assert!(Code::parse("1-7D4B").unwrap().score() > Code::parse("2-2121").unwrap().score());
    }

    #[test]
    fn tones_are_canonical_and_source_order_does_not_change_output() {
        let unicode = "1-4421\t4E00\n1-4427\t4E86\n";
        let a = "1-4421\tㄧ\n1-4421\tㄧˊ\n1-4421\tㄧˋ\n1-4427\t˙ㄌㄜ\n";
        let b = "1-4427\t˙ㄌㄜ\n1-4421\tㄧˋ\n1-4421\tㄧ\n1-4421\tㄧˊ\n";
        let (first, count, _, _, _) = convert(a, unicode, &empty_seed()).unwrap();
        let (second, _, _, _, _) = convert(b, unicode, &empty_seed()).unwrap();
        assert_eq!(count, 4);
        assert_eq!(first.to_text(), second.to_text());
        assert_eq!(
            first.lookup(&[Syllable::parse("ㄌㄜ˙").unwrap()])[0].text,
            "了"
        );
    }

    #[test]
    fn malformed_out_of_scope_missing_and_duplicate_source_records_fail_closed() {
        let valid = "1-4421\tㄧ\n";
        let unicode = "1-4421\t4E00\n";
        for invalid in [
            "1-4421\tㄅ\n",
            "1-4421\tPRIVATE_INPUT_SENTINEL\n",
            "1-4421\tㄧ\textra\n",
            "1-4421\t\n",
            "\n",
            "1-4421\tㄧ\n1-4421\tㄧˉ\n",
            "3-2121\tㄧ\n",
            "1-4422\tㄧ\n",
            "",
        ] {
            assert!(convert(invalid, unicode, &empty_seed()).is_err());
        }
        for invalid in [
            "1-4421\tD800\n",
            "1-4421\t110000\n",
            "1-4421\tE000\n",
            "1-4421\t4E00 4E01\n",
            "1-4421\t3042\n",
            "3-2121\t6C49\n",
            "1-4421\t4E00\n1-4421\t4E00\n",
            "1-4421\t4E00\n1-4422\t4E00\n",
            "",
        ] {
            assert!(convert(valid, invalid, &empty_seed()).is_err());
        }
    }

    #[test]
    fn known_incomplete_reading_is_counted_but_never_guessed() {
        let unicode = "2-3378\t5550\n";
        let valid = "2-3378\tㄘㄨㄟˋ\n";
        let (lexicon, count, _, _, excluded) =
            convert(&format!("{valid}2-3378\t˙ㄑ\n"), unicode, &empty_seed()).unwrap();
        assert_eq!((count, excluded), (1, 1));
        assert!(
            lexicon
                .lookup(&[Syllable::parse("ㄑㄧ˙").unwrap()])
                .is_empty()
        );
        assert!(convert("2-3378\t˙ㄑ\n", unicode, &empty_seed()).is_err());
        assert!(
            convert(
                &format!("{valid}2-3378\t˙ㄑ\n2-3378\t˙ㄑ\n"),
                unicode,
                &empty_seed()
            )
            .is_err()
        );
        assert!(convert(&format!("{valid}2-3378\t˙ㄐ\n"), unicode, &empty_seed()).is_err());
    }

    #[test]
    fn seed_characters_are_not_imported_and_phrases_require_source_readings() {
        let unicode = "1-4421\t4E00\n1-4422\t4E59\n";
        let phonetic = "1-4421\tㄧ\n1-4422\tㄧˇ\n";
        let seed =
            Lexicon::parse(&format!("{HEADER}\nㄧ\t汉\t100\nㄧ ㄧˇ\t一乙\t100000\n")).unwrap();
        let (lexicon, _, phrases, ignored, _) = convert(phonetic, unicode, &seed).unwrap();
        assert_eq!((phrases, ignored), (1, 1));
        assert!(!lexicon.to_text().contains('汉'));
        for phrase in ["ㄧ ㄧ\t一乙\t1", "ㄧ ㄧˇ\t一汉\t1", "ㄧ ㄧˇ\t一\t1"] {
            let seed = Lexicon::parse(&format!("{HEADER}\n{phrase}\n")).unwrap();
            assert!(convert(phonetic, unicode, &seed).is_err());
        }
    }
}
