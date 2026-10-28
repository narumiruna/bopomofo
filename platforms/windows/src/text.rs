//! Checked conversion from core UTF-8 byte offsets to TSF UTF-16 units.
use ime_core::Snapshot;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetError {
    InvalidBoundary,
    ReversedRange,
    InvalidCandidateSpan,
    TooLong,
}

pub fn utf16_offset(text: &str, byte: usize) -> Result<i32, OffsetError> {
    let prefix = text.get(..byte).ok_or(OffsetError::InvalidBoundary)?;
    i32::try_from(prefix.encode_utf16().count()).map_err(|_| OffsetError::TooLong)
}

pub fn utf16_range(text: &str, range: Range<usize>) -> Result<Range<i32>, OffsetError> {
    if range.start > range.end {
        return Err(OffsetError::ReversedRange);
    }
    Ok(utf16_offset(text, range.start)?..utf16_offset(text, range.end)?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    pub text: Vec<u16>,
    pub cursor: i32,
}

impl TextSnapshot {
    pub fn checked(snapshot: &Snapshot) -> Result<Self, OffsetError> {
        utf16_offset(&snapshot.preedit, snapshot.preedit.len())?;
        // Candidate spans use syllable indices, not text offsets. Do not pass
        // them to a UTF-16 range API or assume one syllable is one character.
        if snapshot.candidates.iter().any(|candidate| {
            candidate.start >= candidate.end || candidate.end > ime_core::MAX_SYLLABLES
        }) {
            return Err(OffsetError::InvalidCandidateSpan);
        }
        Ok(Self {
            text: snapshot.preedit.encode_utf16().collect(),
            cursor: utf16_offset(&snapshot.preedit, snapshot.cursor)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ime_core::{Candidate, InputMode};

    #[test]
    fn converts_bmp_non_bmp_emoji_and_combining_sequence() {
        let text = "中𠀀😀e\u{301}";
        for (byte, units) in [(0, 0), (3, 1), (7, 3), (11, 5), (12, 6), (14, 7)] {
            assert_eq!(utf16_offset(text, byte), Ok(units));
        }
        assert_eq!(utf16_range(text, 3..11), Ok(1..5));
        assert_eq!(utf16_range(text, 11..14), Ok(5..7));
        assert_eq!(utf16_range(text, 14..14), Ok(7..7));
    }

    #[test]
    fn rejects_interior_out_of_bounds_and_reversed_offsets() {
        for byte in [1, 2, 4, 5, 6, 8, 9, 10, 13, 15, usize::MAX] {
            assert_eq!(
                utf16_offset("中𠀀😀e\u{301}", byte),
                Err(OffsetError::InvalidBoundary)
            );
        }
        assert_eq!(
            utf16_range("中", Range { start: 3, end: 0 }),
            Err(OffsetError::ReversedRange)
        );
        assert_eq!(utf16_offset("", 0), Ok(0));
    }

    #[test]
    fn candidate_syllable_spans_are_not_converted_as_text_bytes() {
        let mut snapshot = Snapshot {
            preedit: "𠀀e\u{301}".into(),
            cursor: 4,
            candidates: vec![Candidate {
                text: "替換".into(),
                start: 0,
                end: 2,
            }],
            page: 0,
            candidate_index: 0,
            selecting: true,
            generation: 1,
            mode: InputMode::Chinese,
            active: true,
        };
        let converted = TextSnapshot::checked(&snapshot).unwrap();
        assert_eq!(converted.cursor, 2);
        assert_eq!(converted.text.len(), 4);
        snapshot.candidates[0].end = ime_core::MAX_SYLLABLES + 1;
        assert_eq!(
            TextSnapshot::checked(&snapshot),
            Err(OffsetError::InvalidCandidateSpan)
        );
        snapshot.candidates[0].end = 0;
        assert_eq!(
            TextSnapshot::checked(&snapshot),
            Err(OffsetError::InvalidCandidateSpan)
        );
    }
}
