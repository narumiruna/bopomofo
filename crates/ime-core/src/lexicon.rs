//! Validated, versioned TSV lexicon and exact-reading index.
//!
//! The first line is `# bopomofo-lexicon-v1`. Every remaining line contains
//! `canonical reading<TAB>text<TAB>positive u32 frequency`. Reading syllables
//! have exactly one ASCII space between them. LF and CRLF are accepted.
//! Blank records, comments after the header, controls in text, and duplicate
//! (reading, text) records are errors. No external data is included here.
//!
//! Resource limits: 64 MiB source, 250,000 records, 16 syllables per phrase,
//! 1,024 UTF-8 bytes per text, and 256 alternatives per exact reading. These
//! limits also bound decoder edges and candidate lists. Export sorts by
//! reading, then UTF-8 text. Lookup sorts by decreasing frequency, then text.

use crate::parser::Syllable;
use std::{collections::BTreeMap, error::Error, fmt};

pub const HEADER: &str = "# bopomofo-lexicon-v1";
pub const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 250_000;
pub const MAX_PHRASE_SYLLABLES: usize = 16;
pub const MAX_TEXT_BYTES: usize = 1024;
pub const MAX_CANDIDATES_PER_READING: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub text: String,
    pub reading: Vec<Syllable>,
    pub frequency: u32,
}

/// Entries are immutable after validation; cloning preserves index validity.
#[derive(Clone, Debug, Default)]
pub struct Lexicon {
    entries: Vec<Entry>,
    index: BTreeMap<Vec<Syllable>, Vec<usize>>,
    total_frequency: u64,
}

impl Lexicon {
    pub fn parse(source: &str) -> Result<Self, LexiconError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(LexiconError::new(0, LexiconErrorKind::SourceTooLarge));
        }
        let mut lines = source.lines();
        if lines.next() != Some(HEADER) {
            return Err(LexiconError::new(1, LexiconErrorKind::MissingHeader));
        }
        let mut records = Vec::new();
        for (index, line) in lines.enumerate() {
            let line_number = index + 2;
            let error = |kind| LexiconError::new(line_number, kind);
            if records.len() >= MAX_ENTRIES {
                return Err(error(LexiconErrorKind::TooManyEntries));
            }
            let mut fields = line.split('\t');
            let reading_text = fields.next().unwrap_or_default();
            let text = fields
                .next()
                .ok_or_else(|| error(LexiconErrorKind::InvalidRecord))?;
            let frequency_text = fields
                .next()
                .ok_or_else(|| error(LexiconErrorKind::InvalidRecord))?;
            if fields.next().is_some() {
                return Err(error(LexiconErrorKind::InvalidRecord));
            }
            if text.is_empty() || text.chars().any(char::is_control) {
                return Err(error(LexiconErrorKind::InvalidText));
            }
            if text.len() > MAX_TEXT_BYTES {
                return Err(error(LexiconErrorKind::TextTooLong));
            }
            if frequency_text.is_empty() || !frequency_text.bytes().all(|c| c.is_ascii_digit()) {
                return Err(error(LexiconErrorKind::InvalidFrequency));
            }
            let frequency = frequency_text
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| error(LexiconErrorKind::InvalidFrequency))?;
            let mut reading = Vec::new();
            for token in reading_text.split(' ') {
                if reading.len() >= MAX_PHRASE_SYLLABLES {
                    return Err(error(LexiconErrorKind::PhraseTooLong));
                }
                let syllable =
                    Syllable::parse(token).map_err(|_| error(LexiconErrorKind::InvalidReading))?;
                if syllable.as_str() != token {
                    return Err(error(LexiconErrorKind::NonCanonicalReading));
                }
                reading.push(syllable);
            }
            records.push((
                Entry {
                    text: text.into(),
                    reading,
                    frequency,
                },
                line_number,
            ));
        }
        records.sort_unstable_by(|(left, _), (right, _)| {
            left.reading
                .cmp(&right.reading)
                .then_with(|| left.text.cmp(&right.text))
        });
        for pair in records.windows(2) {
            if pair[0].0.reading == pair[1].0.reading && pair[0].0.text == pair[1].0.text {
                return Err(LexiconError::new(
                    pair[1].1,
                    LexiconErrorKind::DuplicateEntry,
                ));
            }
        }
        let mut index: BTreeMap<Vec<Syllable>, Vec<usize>> = BTreeMap::new();
        for (position, (entry, line_number)) in records.iter().enumerate() {
            let candidates = index.entry(entry.reading.clone()).or_default();
            if candidates.len() >= MAX_CANDIDATES_PER_READING {
                return Err(LexiconError::new(
                    *line_number,
                    LexiconErrorKind::TooManyCandidates,
                ));
            }
            candidates.push(position);
        }
        let entries: Vec<_> = records.into_iter().map(|(entry, _)| entry).collect();
        for candidates in index.values_mut() {
            candidates.sort_unstable_by(|&left, &right| {
                entries[right]
                    .frequency
                    .cmp(&entries[left].frequency)
                    .then_with(|| entries[left].text.cmp(&entries[right].text))
            });
        }
        // MAX_ENTRIES * u32::MAX fits u64.
        let total_frequency = entries.iter().map(|entry| u64::from(entry.frequency)).sum();
        Ok(Self {
            entries,
            index,
            total_frequency,
        })
    }

    /// Stable canonical export, including the header and final LF.
    pub fn to_text(&self) -> String {
        use fmt::Write;
        let mut output = String::from(HEADER);
        output.push('\n');
        for entry in &self.entries {
            for (index, syllable) in entry.reading.iter().enumerate() {
                if index > 0 {
                    output.push(' ');
                }
                output.push_str(syllable.as_str());
            }
            // Writing into String cannot fail.
            writeln!(output, "\t{}\t{}", entry.text, entry.frequency).unwrap();
        }
        output
    }

    /// Exact phrase match; no prefix, substring, or toneless matching.
    pub fn lookup(&self, reading: &[Syllable]) -> Vec<&Entry> {
        self.index.get(reading).map_or_else(Vec::new, |positions| {
            positions
                .iter()
                .map(|&position| &self.entries[position])
                .collect()
        })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn total_frequency(&self) -> u64 {
        self.total_frequency
    }
}

/// A line number and an error category, never source text.
/// Line zero identifies a file-wide resource error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LexiconError {
    pub line: usize,
    pub kind: LexiconErrorKind,
}

impl LexiconError {
    fn new(line: usize, kind: LexiconErrorKind) -> Self {
        Self { line, kind }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexiconErrorKind {
    MissingHeader,
    InvalidRecord,
    InvalidReading,
    NonCanonicalReading,
    InvalidText,
    InvalidFrequency,
    DuplicateEntry,
    SourceTooLarge,
    TooManyEntries,
    PhraseTooLong,
    TextTooLong,
    TooManyCandidates,
}

impl fmt::Display for LexiconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            LexiconErrorKind::MissingHeader => "missing or unsupported lexicon header",
            LexiconErrorKind::InvalidRecord => "expected three TSV fields",
            LexiconErrorKind::InvalidReading => "invalid syllable reading",
            LexiconErrorKind::NonCanonicalReading => "reading is not canonical",
            LexiconErrorKind::InvalidText => "empty text or control character",
            LexiconErrorKind::InvalidFrequency => "frequency must be a positive u32",
            LexiconErrorKind::DuplicateEntry => "duplicate reading and text",
            LexiconErrorKind::SourceTooLarge => "lexicon source byte limit exceeded",
            LexiconErrorKind::TooManyEntries => "lexicon record limit exceeded",
            LexiconErrorKind::PhraseTooLong => "phrase syllable limit exceeded",
            LexiconErrorKind::TextTooLong => "text byte limit exceeded",
            LexiconErrorKind::TooManyCandidates => "reading candidate limit exceeded",
        };
        write!(f, "lexicon line {}: {message}", self.line)
    }
}

impl Error for LexiconError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(text: &str) -> Vec<Syllable> {
        text.split(' ')
            .map(|part| Syllable::parse(part).unwrap())
            .collect()
    }

    fn parse_records(records: &str) -> Result<Lexicon, LexiconError> {
        Lexicon::parse(&format!("{HEADER}\n{records}"))
    }

    #[test]
    fn exact_reading_index_and_frequency_order() {
        let lexicon = parse_records(
            "ㄋㄧˇ\t妳\t9\nㄋㄧˇ ㄏㄠˇ\t你好\t5\nㄋㄧˇ\t你\t10\nㄋㄧˇ\t擬\t9\nㄋㄧˊ\t泥\t30\n",
        )
        .unwrap();
        let found = lexicon.lookup(&reading("ㄋㄧˇ"));
        assert_eq!(
            found
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>(),
            ["你", "妳", "擬"]
        );
        assert_eq!(lexicon.lookup(&reading("ㄋㄧˇ ㄏㄠˇ"))[0].text, "你好");
        assert!(lexicon.lookup(&reading("ㄏㄠˇ")).is_empty());
        assert!(lexicon.lookup(&reading("ㄋㄧ")).is_empty());
        assert!(lexicon.lookup(&[]).is_empty());
    }

    #[test]
    fn export_is_stable_sorted_and_round_trips() {
        let first = parse_records("ㄋㄧˇ\t妳\t0009\nㄏㄠˇ\t好\t3\nㄋㄧˇ\t你\t10\n").unwrap();
        let second = parse_records("ㄋㄧˇ\t你\t10\nㄏㄠˇ\t好\t3\nㄋㄧˇ\t妳\t9\n").unwrap();
        assert_eq!(first.to_text(), second.to_text());
        assert_eq!(
            Lexicon::parse(&first.to_text()).unwrap().to_text(),
            first.to_text()
        );
        assert_eq!(first.entries().len(), 3);
        assert!(
            first
                .entries()
                .windows(2)
                .all(|pair| pair[0].reading <= pair[1].reading)
        );
    }

    #[test]
    fn malformed_records_and_missing_headers() {
        for (source, expected) in [
            ("\n", LexiconErrorKind::InvalidRecord),
            ("ㄇㄚ\t媽", LexiconErrorKind::InvalidRecord),
            ("ㄇㄚ\t媽\t1\textra", LexiconErrorKind::InvalidRecord),
            ("\t媽\t1", LexiconErrorKind::InvalidReading),
            ("ㄅㄩ\t媽\t1", LexiconErrorKind::InvalidReading),
            ("ㄇㄚ  ㄇㄚ\t媽媽\t1", LexiconErrorKind::InvalidReading),
            ("ㄇㄚ \t媽\t1", LexiconErrorKind::InvalidReading),
            (" ㄇㄚ\t媽\t1", LexiconErrorKind::InvalidReading),
            ("ㄇㄚˉ\t媽\t1", LexiconErrorKind::NonCanonicalReading),
            ("˙ㄇㄚ\t嗎\t1", LexiconErrorKind::NonCanonicalReading),
            ("ㄇㄚ\t\t1", LexiconErrorKind::InvalidText),
            ("ㄇㄚ\t媽\0\t1", LexiconErrorKind::InvalidText),
            ("ㄇㄚ\t媽\u{0085}\t1", LexiconErrorKind::InvalidText),
            ("ㄇㄚ\t媽\r\t1", LexiconErrorKind::InvalidText),
            ("ㄇㄚ\t媽\t", LexiconErrorKind::InvalidFrequency),
            ("ㄇㄚ\t媽\t0", LexiconErrorKind::InvalidFrequency),
            ("ㄇㄚ\t媽\t-1", LexiconErrorKind::InvalidFrequency),
            ("ㄇㄚ\t媽\t+1", LexiconErrorKind::InvalidFrequency),
            ("ㄇㄚ\t媽\t1.0", LexiconErrorKind::InvalidFrequency),
            ("ㄇㄚ\t媽\t 1", LexiconErrorKind::InvalidFrequency),
            ("ㄇㄚ\t媽\t4294967296", LexiconErrorKind::InvalidFrequency),
        ] {
            let error = parse_records(source).unwrap_err();
            assert_eq!(error.kind, expected, "{source:?}");
            assert_eq!(error.line, 2);
        }
        for invalid_header in [
            "",
            "# bopomofo-lexicon-v2\n",
            "ㄇㄚ\t媽\t1\n",
            "\u{feff}# bopomofo-lexicon-v1\n",
        ] {
            assert_eq!(
                Lexicon::parse(invalid_header).unwrap_err().kind,
                LexiconErrorKind::MissingHeader
            );
        }
    }

    #[test]
    fn duplicate_reading_text_is_rejected_but_heteronyms_are_valid() {
        let error = parse_records("ㄏㄠˇ\t好\t1\nㄏㄠˇ\t好\t2").unwrap_err();
        assert_eq!(error.kind, LexiconErrorKind::DuplicateEntry);
        assert!([2, 3].contains(&error.line));
        assert!(parse_records("ㄏㄠˇ\t好\t1\nㄏㄠˋ\t好\t2").is_ok());
    }

    #[test]
    fn text_need_not_match_syllable_or_scalar_count() {
        let lexicon = parse_records("ㄧ\t𠀀e\u{0301}😀\t4294967295").unwrap();
        assert_eq!(lexicon.entries()[0].text, "𠀀e\u{0301}😀");
        assert_eq!(lexicon.total_frequency(), u64::from(u32::MAX));
        assert_eq!(
            Lexicon::parse(&lexicon.to_text()).unwrap().entries(),
            lexicon.entries()
        );
    }

    #[test]
    fn crlf_and_empty_lexicons() {
        assert_eq!(Lexicon::parse(HEADER).unwrap().entries().len(), 0);
        assert_eq!(Lexicon::default().to_text(), format!("{HEADER}\n"));
        let lexicon = Lexicon::parse(&format!("{HEADER}\r\nㄧ\t一\t1\r\n")).unwrap();
        assert_eq!(lexicon.to_text(), format!("{HEADER}\nㄧ\t一\t1\n"));
    }

    #[test]
    fn phrase_text_and_candidate_resource_caps() {
        let at_limit = vec!["ㄧ"; MAX_PHRASE_SYLLABLES].join(" ");
        assert!(parse_records(&format!("{at_limit}\t一\t1")).is_ok());
        let over_limit = format!("{at_limit} ㄧ\t一\t1");
        assert_eq!(
            parse_records(&over_limit).unwrap_err().kind,
            LexiconErrorKind::PhraseTooLong
        );
        assert!(parse_records(&format!("ㄧ\t{}\t1", "a".repeat(MAX_TEXT_BYTES))).is_ok());
        assert_eq!(
            parse_records(&format!("ㄧ\t{}\t1", "a".repeat(MAX_TEXT_BYTES + 1)))
                .unwrap_err()
                .kind,
            LexiconErrorKind::TextTooLong
        );
        let mut records = String::new();
        for candidate in 0..MAX_CANDIDATES_PER_READING {
            records.push_str(&format!("ㄧ\t字{candidate}\t1\n"));
        }
        assert_eq!(
            parse_records(&records)
                .unwrap()
                .lookup(&reading("ㄧ"))
                .len(),
            MAX_CANDIDATES_PER_READING
        );
        records.push_str("ㄧ\t超限\t1\n");
        assert_eq!(
            parse_records(&records).unwrap_err().kind,
            LexiconErrorKind::TooManyCandidates
        );
    }
}
