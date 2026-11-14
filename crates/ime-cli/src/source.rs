//! Offline authoring format. All phonetic parsing and dictionary compilation use libchewing.
use chewing::dictionary::{DictionaryBuilder, DictionaryInfo, DictionaryUsage, TrieBuilder};
use chewing::zhuyin::Syllable;
use ime_core::assets::{MAX_CANDIDATES, MAX_ENTRIES, MAX_PHRASE_SYLLABLES, MAX_TEXT_BYTES};
use std::{collections::BTreeMap, fmt::Write};

pub const HEADER: &str = "# bopomofo-lexicon-v1";
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reading {
    text: String,
    value: Syllable,
}
impl Reading {
    pub fn parse(text: &str) -> Result<Self, ()> {
        let normalized = if let Some(rest) = text.strip_prefix('˙') {
            format!("{rest}˙")
        } else {
            text.to_owned()
        };
        let mut value: Syllable = normalized.parse().map_err(|_| ())?;
        if value.tone() == Some(chewing::zhuyin::Bopomofo::TONE1) {
            value.remove_tone();
        }
        if value.is_empty() {
            return Err(());
        }
        Ok(Self {
            text: value.to_string(),
            value,
        })
    }
    pub fn as_str(&self) -> &str {
        &self.text
    }
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub reading: Vec<Reading>,
    pub text: String,
    pub frequency: u32,
}
#[derive(Clone, Debug, Default)]
pub struct Source {
    entries: Vec<Entry>,
}
impl Source {
    pub fn parse(text: &str) -> Result<Self, ()> {
        if text.len() > crate::files::MAX_INPUT_BYTES {
            return Err(());
        }
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Err(());
        }
        let mut entries = Vec::new();
        let mut seen = BTreeMap::new();
        for line in lines {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 3
                || entries.len() >= MAX_ENTRIES
                || fields[1].is_empty()
                || fields[1].len() > MAX_TEXT_BYTES
                || fields[1].chars().any(char::is_control)
                || fields[2].is_empty()
                || !fields[2].bytes().all(|c| c.is_ascii_digit())
            {
                return Err(());
            }
            let reading = fields[0]
                .split(' ')
                .map(Reading::parse)
                .collect::<Result<Vec<_>, _>>()?;
            if reading.is_empty()
                || reading.len() > MAX_PHRASE_SYLLABLES
                || reading
                    .iter()
                    .map(Reading::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
                    != fields[0]
            {
                return Err(());
            }
            let frequency = fields[2].parse::<u32>().map_err(|_| ())?;
            if frequency == 0 {
                return Err(());
            }
            let texts = seen
                .entry(reading.clone())
                .or_insert_with(std::collections::BTreeSet::new);
            if !texts.insert(fields[1].to_owned()) || texts.len() > MAX_CANDIDATES {
                return Err(());
            }
            entries.push(Entry {
                reading,
                text: fields[1].into(),
                frequency,
            });
        }
        entries.sort_by(|a, b| a.reading.cmp(&b.reading).then(a.text.cmp(&b.text)));
        Ok(Self { entries })
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn to_text(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for e in &self.entries {
            writeln!(
                out,
                "{}\t{}\t{}",
                e.reading
                    .iter()
                    .map(Reading::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
                e.text,
                e.frequency
            )
            .unwrap();
        }
        out
    }
    pub fn dictionary(&self, license: &str) -> Result<Vec<u8>, ()> {
        let mut b = TrieBuilder::new();
        b.set_info(DictionaryInfo {
            name: "bopomofo".into(),
            copyright: "See the package Data-NOTICE.txt".into(),
            license: license.into(),
            version: ime_core::assets::ASSET_FORMAT.into(),
            software: ime_core::assets::ENGINE_VERSION.into(),
            usage: DictionaryUsage::BuiltIn,
        })
        .map_err(|_| ())?;
        for e in &self.entries {
            if e.text.chars().count() != e.reading.len() {
                return Err(());
            }
            b.insert(
                &e.reading.iter().map(|r| r.value).collect::<Vec<_>>(),
                (e.text.as_str(), e.frequency).into(),
            )
            .map_err(|_| ())?;
        }
        let mut bytes = Vec::new();
        b.write(&mut bytes).map_err(|_| ())?;
        Ok(bytes)
    }
    pub fn upstream(word: &str, tsi: &str) -> Result<Self, ()> {
        let mut pairs = BTreeMap::<(Vec<Reading>, String), u32>::new();
        for text in [word, tsi] {
            if !text.starts_with("# dc:title,") || !text.contains("# dc:license,LGPL-2.1-or-later,")
            {
                return Err(());
            }
            for line in text.lines().filter(|l| !l.starts_with('#')) {
                let fields = line.split(',').collect::<Vec<_>>();
                if fields.len() != 3 {
                    return Err(());
                }
                let reading = fields[2]
                    .split_whitespace()
                    .map(Reading::parse)
                    .collect::<Result<Vec<_>, _>>()?;
                let score = fields[1].parse::<u32>().map_err(|_| ())?;
                let value = pairs.entry((reading, fields[0].into())).or_default();
                *value = (*value).max(score);
            }
        }
        Ok(Self {
            entries: pairs
                .into_iter()
                .map(|((reading, text), frequency)| Entry {
                    reading,
                    text,
                    frequency,
                })
                .collect(),
        })
    }
}
