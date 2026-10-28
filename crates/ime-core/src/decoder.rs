//! Bounded unigram Viterbi decoder with exact locked spans.
//!
//! A lexicon edge scores log2(frequency / (total_frequency + 1)) in signed
//! Q32 fixed point. Logarithms use integer arithmetic only. A raw singleton
//! scores log2(1 / (total_frequency + 1)) minus 16 bits. Every unlocked
//! position has this raw Zhuyin edge, so unknown readings are never lost.
//!
//! Ties prefer fewer raw syllables, then fewer segments, then the longer
//! leftmost segment, then ascending UTF-8 text. Same-reading alternatives
//! need only their highest-frequency entry (text breaks frequency ties).
//! Search has at most 64 * 16 phrase lookups and does not enumerate paths.
//!
//! Every segment maps its interior syllable positions to its starting byte
//! offset. Its end maps to the byte after its complete text. This applies
//! to lexicon phrases and locks, regardless of Unicode scalar count.

use crate::{
    lexicon::{Lexicon, MAX_PHRASE_SYLLABLES, MAX_TEXT_BYTES},
    parser::Syllable,
};
use std::{error::Error, fmt};

pub use crate::MAX_SYLLABLES;
const SCORE_ONE: i64 = 1_i64 << 32;
const RAW_PENALTY: i64 = 16 * SCORE_ONE;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    /// Inclusive syllable index.
    pub start: usize,
    /// Exclusive syllable index.
    pub end: usize,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decoded {
    pub text: String,
    /// Monotone UTF-8 byte boundaries; length is syllable count plus one.
    pub offsets: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecoderError {
    TooManySyllables,
    TooManyLocks,
    InvalidLockRange,
    InvalidLockText,
    OverlappingLocks,
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooManySyllables => "decoder accepts at most 64 syllables",
            Self::TooManyLocks => "more locks than syllables",
            Self::InvalidLockRange => "lock must cover a nonempty valid syllable range",
            Self::InvalidLockText => "lock text is empty, contains controls, or exceeds 1024 bytes",
            Self::OverlappingLocks => "lock ranges overlap",
        })
    }
}

impl Error for DecoderError {}

/// Decode without I/O or input mutation.
///
/// Locks can be supplied in any order. They must be nonempty, disjoint,
/// within the input, and contain nonempty control-free text of at most
/// MAX_TEXT_BYTES bytes. Invalid inputs return an error before decoding.
/// Errors do not mutate the inputs and must not produce a partial commit.
pub fn decode(
    lexicon: &Lexicon,
    syllables: &[Syllable],
    locks: &[Lock],
) -> Result<Decoded, DecoderError> {
    let count = syllables.len();
    if count > MAX_SYLLABLES {
        return Err(DecoderError::TooManySyllables);
    }
    if locks.len() > count {
        return Err(DecoderError::TooManyLocks);
    }
    let mut ordered_locks: Vec<_> = locks.iter().collect();
    ordered_locks.sort_unstable_by_key(|lock| (lock.start, lock.end));
    let mut previous_end = 0;
    for lock in &ordered_locks {
        if lock.start >= lock.end || lock.end > count {
            return Err(DecoderError::InvalidLockRange);
        }
        if lock.text.is_empty()
            || lock.text.len() > MAX_TEXT_BYTES
            || lock.text.chars().any(char::is_control)
        {
            return Err(DecoderError::InvalidLockText);
        }
        if lock.start < previous_end {
            return Err(DecoderError::OverlappingLocks);
        }
        previous_end = lock.end;
    }
    let mut lock_at = vec![None; count];
    let mut inside_lock = vec![false; count];
    for lock in ordered_locks {
        lock_at[lock.start] = Some(lock);
        inside_lock[lock.start + 1..lock.end].fill(true);
    }

    let normalizer = log2_fixed(lexicon.total_frequency() + 1);
    let raw_score = -normalizer - RAW_PENALTY;
    let mut paths = vec![None; count + 1];
    paths[count] = Some(Path {
        score: 0,
        raw_count: 0,
        segments: 0,
        end: count,
        text: "",
    });
    let mut next_lock_start = count;
    for start in (0..count).rev() {
        if let Some(lock) = lock_at[start] {
            let suffix = paths[lock.end].expect("validated lock has a reachable suffix");
            consider(&mut paths[start], suffix, lock.end, &lock.text, 0, false);
            next_lock_start = start;
            continue;
        }
        if inside_lock[start] {
            continue;
        }
        let suffix = paths[start + 1].expect("unlocked singleton has a reachable suffix");
        consider(
            &mut paths[start],
            suffix,
            start + 1,
            syllables[start].as_str(),
            raw_score,
            true,
        );
        let limit = (start + MAX_PHRASE_SYLLABLES).min(next_lock_start);
        for end in start + 1..=limit {
            let Some(entry) = lexicon.lookup(&syllables[start..end]).into_iter().next() else {
                continue;
            };
            let suffix = paths[end].expect("unlocked phrase has a reachable suffix");
            let score = log2_fixed(u64::from(entry.frequency)) - normalizer;
            consider(&mut paths[start], suffix, end, &entry.text, score, false);
        }
    }
    let mut text = String::new();
    let mut offsets = vec![0; count + 1];
    let mut position = 0;
    while position < count {
        let path = paths[position].expect("every unlocked start has a raw fallback");
        offsets[position..path.end].fill(text.len());
        text.push_str(path.text);
        offsets[path.end] = text.len();
        position = path.end;
    }
    Ok(Decoded { text, offsets })
}

#[derive(Clone, Copy)]
struct Path<'a> {
    score: i64,
    raw_count: usize,
    segments: usize,
    end: usize,
    text: &'a str,
}

fn consider<'a>(
    best: &mut Option<Path<'a>>,
    suffix: Path<'a>,
    end: usize,
    text: &'a str,
    score: i64,
    raw: bool,
) {
    let candidate = Path {
        score: score + suffix.score,
        raw_count: usize::from(raw) + suffix.raw_count,
        segments: 1 + suffix.segments,
        end,
        text,
    };
    let better = best.is_none_or(|current| {
        candidate
            .score
            .cmp(&current.score)
            .then_with(|| current.raw_count.cmp(&candidate.raw_count))
            .then_with(|| current.segments.cmp(&candidate.segments))
            .then_with(|| candidate.end.cmp(&current.end))
            .then_with(|| current.text.cmp(candidate.text))
            .is_gt()
    });
    if better {
        *best = Some(candidate);
    }
}

/// Floor(log2(value) * 2^32), for nonzero values.
/// Normalize to unsigned Q63, then obtain 32 fractional bits by squaring.
/// Each normalized value is below 2^64, so its square fits u128.
fn log2_fixed(value: u64) -> i64 {
    debug_assert!(value > 0);
    let exponent = 63 - value.leading_zeros();
    let mut normalized = u128::from(value) << (63 - exponent);
    let mut fraction = 0_i64;
    for bit in (0..32).rev() {
        normalized = (normalized * normalized) >> 63;
        if normalized >= (1_u128 << 64) {
            normalized >>= 1;
            fraction |= 1_i64 << bit;
        }
    }
    i64::from(exponent) * SCORE_ONE + fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::HEADER;

    fn reading(text: &str) -> Vec<Syllable> {
        text.split(' ')
            .map(|part| Syllable::parse(part).unwrap())
            .collect()
    }

    fn lexicon(records: &str) -> Lexicon {
        Lexicon::parse(&format!("{HEADER}\n{records}")).unwrap()
    }

    fn assert_offsets(decoded: &Decoded, count: usize) {
        assert_eq!(decoded.offsets.len(), count + 1);
        assert_eq!(decoded.offsets[0], 0);
        assert_eq!(decoded.offsets[count], decoded.text.len());
        assert!(decoded.offsets.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(
            decoded
                .offsets
                .iter()
                .all(|&offset| decoded.text.is_char_boundary(offset))
        );
    }

    #[test]
    fn optimal_segmentation_is_not_greedy() {
        let lexicon = lexicon("ㄧ\t甲\t1\nㄨ\t乙\t1\nㄩ\t丙\t1\nㄧ ㄨ\t前詞\t10\nㄨ ㄩ\t後詞\t100");
        let decoded = decode(&lexicon, &reading("ㄧ ㄨ ㄩ"), &[]).unwrap();
        assert_eq!(decoded.text, "甲後詞");
        assert_eq!(decoded.offsets, [0, 3, 3, 9]);
    }

    #[test]
    fn unigram_normalization_can_prefer_frequent_singletons() {
        let lexicon = lexicon("ㄧ\t甲\t1000\nㄨ\t乙\t1000\nㄧ ㄨ\t罕見詞\t1");
        assert_eq!(
            decode(&lexicon, &reading("ㄧ ㄨ"), &[]).unwrap().text,
            "甲乙"
        );
    }

    #[test]
    fn frequency_then_text_and_segmentation_ties_are_deterministic() {
        let ranked = lexicon("ㄧ\t甲\t1\nㄧ\t乙\t2\nㄨ\t甲\t4\nㄨ\t乙\t4");
        assert_eq!(
            decode(&ranked, &reading("ㄧ ㄨ"), &[]).unwrap().text,
            "乙乙"
        );
        let tied = lexicon_from_tie_fixture(false);
        let reversed = lexicon_from_tie_fixture(true);
        for lexicon in [&tied, &reversed] {
            assert_eq!(
                decode(lexicon, &reading("ㄧ ㄨ ㄩ"), &[]).unwrap().text,
                "前丙"
            );
        }
        // Adjacent u32 frequencies remain distinct, even at the upper limit.
        let high = lexicon("ㄧ\t乙\t4294967294\nㄧ\t甲\t4294967295");
        assert_eq!(decode(&high, &reading("ㄧ"), &[]).unwrap().text, "甲");
        assert!(log2_fixed(u64::from(u32::MAX)) > log2_fixed(u64::from(u32::MAX - 1)));
    }

    fn lexicon_from_tie_fixture(reverse: bool) -> Lexicon {
        let mut records = [
            "ㄧ\t甲\t1",
            "ㄨ\t乙\t1",
            "ㄩ\t丙\t1",
            "ㄧ ㄨ\t前\t1",
            "ㄨ ㄩ\t後\t1",
        ];
        if reverse {
            records.reverse();
        }
        lexicon(&records.join("\n"))
    }

    #[test]
    fn fallback_keeps_all_unknown_zhuyin_and_tones() {
        let lexicon = lexicon("ㄇㄚ\t媽\t1");
        let decoded = decode(&lexicon, &reading("ㄇㄚ ㄋㄩˇ ㄦˊ ㄓ˙"), &[]).unwrap();
        assert_eq!(decoded.text, "媽ㄋㄩˇㄦˊㄓ˙");
        assert_offsets(&decoded, 4);
        let empty = decode(&Lexicon::default(), &[], &[]).unwrap();
        assert_eq!(
            empty,
            Decoded {
                text: String::new(),
                offsets: vec![0]
            }
        );
    }

    #[test]
    fn locks_block_crossing_phrases_and_preserve_unicode_text() {
        let lexicon =
            lexicon("ㄧ\t甲\t1\nㄧ ㄨ\t跨界\t4000000000\nㄨ ㄩ ㄦ\t跨界尾\t4000000000\nㄦ\t丁\t1");
        let locks = [Lock {
            start: 1,
            end: 3,
            text: "𠀀e\u{0301}😀".into(),
        }];
        let decoded = decode(&lexicon, &reading("ㄧ ㄨ ㄩ ㄦ"), &locks).unwrap();
        assert_eq!(decoded.text, "甲𠀀e\u{0301}😀丁");
        assert_eq!(decoded.offsets, [0, 3, 3, 14, 17]);
        assert_offsets(&decoded, 4);
        assert_eq!(locks[0].text, "𠀀e\u{0301}😀");
    }

    #[test]
    fn adjacent_and_unsorted_locks_and_arbitrary_scalar_counts() {
        let syllables = reading("ㄧ ㄨ ㄩ ㄦ");
        let locks = [
            Lock {
                start: 2,
                end: 4,
                text: "e\u{0301}".into(),
            },
            Lock {
                start: 0,
                end: 2,
                text: "𠀀😀多字".into(),
            },
        ];
        let decoded = decode(&Lexicon::default(), &syllables, &locks).unwrap();
        assert_eq!(decoded.text, "𠀀😀多字e\u{0301}");
        assert_eq!(decoded.offsets, [0, 0, 14, 14, 17]);
        assert_offsets(&decoded, 4);
        let phrase = lexicon("ㄧ ㄨ\t𠀀e\u{0301}😀\t1");
        assert_eq!(
            decode(&phrase, &syllables[..2], &[]).unwrap().offsets,
            [0, 0, 11]
        );
    }

    #[test]
    fn rejects_bad_locks_and_over_limit_without_mutating_input() {
        let syllables = reading("ㄧ ㄨ");
        let lexicon = Lexicon::default();
        for lock in [
            Lock {
                start: 0,
                end: 0,
                text: "甲".into(),
            },
            Lock {
                start: 1,
                end: 0,
                text: "甲".into(),
            },
            Lock {
                start: 0,
                end: 3,
                text: "甲".into(),
            },
            Lock {
                start: usize::MAX,
                end: usize::MAX,
                text: "甲".into(),
            },
        ] {
            assert_eq!(
                decode(&lexicon, &syllables, &[lock]),
                Err(DecoderError::InvalidLockRange)
            );
        }
        for text in [
            String::new(),
            "\0".into(),
            "甲\n".into(),
            "x".repeat(MAX_TEXT_BYTES + 1),
        ] {
            assert_eq!(
                decode(
                    &lexicon,
                    &syllables,
                    &[Lock {
                        start: 0,
                        end: 1,
                        text
                    }]
                ),
                Err(DecoderError::InvalidLockText)
            );
        }
        let locks = [
            Lock {
                start: 0,
                end: 2,
                text: "甲".into(),
            },
            Lock {
                start: 1,
                end: 2,
                text: "乙".into(),
            },
        ];
        assert_eq!(
            decode(&lexicon, &syllables, &locks),
            Err(DecoderError::OverlappingLocks)
        );
        let locks = vec![
            Lock {
                start: 0,
                end: 1,
                text: "甲".into()
            };
            3
        ];
        assert_eq!(
            decode(&lexicon, &syllables, &locks),
            Err(DecoderError::TooManyLocks)
        );
        let over_limit = vec![syllables[0].clone(); MAX_SYLLABLES + 1];
        assert_eq!(
            decode(&lexicon, &over_limit, &[]),
            Err(DecoderError::TooManySyllables)
        );
        assert_eq!(over_limit.len(), MAX_SYLLABLES + 1);
    }

    #[test]
    fn full_composition_has_bounded_fallback_and_phrase_search() {
        let syllable = Syllable::parse("ㄧ").unwrap();
        let syllables = vec![syllable; MAX_SYLLABLES];
        let decoded = decode(&Lexicon::default(), &syllables, &[]).unwrap();
        assert_eq!(decoded.text, "ㄧ".repeat(MAX_SYLLABLES));
        assert_offsets(&decoded, MAX_SYLLABLES);
        let records = (1..=MAX_PHRASE_SYLLABLES)
            .map(|length| format!("{}\t詞{length}\t1", vec!["ㄧ"; length].join(" ")))
            .collect::<Vec<_>>()
            .join("\n");
        let decoded = decode(&lexicon(&records), &syllables, &[]).unwrap();
        assert_eq!(decoded.text, "詞16".repeat(4));
        assert_offsets(&decoded, MAX_SYLLABLES);
    }

    #[test]
    fn all_small_lock_layouts_preserve_text_and_boundaries() {
        let lexicon = lexicon("ㄧ\t甲\t10\nㄧ ㄧ\t乙\t20\nㄧ ㄧ ㄧ\t𠀀e\u{0301}😀\t30");
        let syllable = Syllable::parse("ㄧ").unwrap();
        for count in 0..=8 {
            let syllables = vec![syllable.clone(); count];
            for mask in 0_usize..1 << count {
                let mut locks = Vec::new();
                let mut position = 0;
                while position < count {
                    if mask & (1 << position) == 0 {
                        position += 1;
                        continue;
                    }
                    let start = position;
                    while position < count && mask & (1 << position) != 0 {
                        position += 1;
                    }
                    locks.push(Lock {
                        start,
                        end: position,
                        text: format!("𠀀e\u{0301}😀{start}"),
                    });
                }
                if mask % 2 == 1 {
                    locks.reverse();
                }
                let decoded = decode(&lexicon, &syllables, &locks).unwrap();
                assert_offsets(&decoded, count);
                for lock in locks {
                    assert_eq!(
                        &decoded.text[decoded.offsets[lock.start]..decoded.offsets[lock.end]],
                        lock.text
                    );
                    assert!(
                        decoded.offsets[lock.start..lock.end]
                            .iter()
                            .all(|&offset| offset == decoded.offsets[lock.start])
                    );
                }
            }
        }
    }

    #[test]
    fn fixed_logarithm_is_monotone_and_exact_at_powers_of_two() {
        for exponent in 0..64 {
            assert_eq!(
                log2_fixed(1_u64 << exponent),
                i64::from(exponent) * SCORE_ONE
            );
        }
        for value in 1..10_000 {
            assert!(log2_fixed(value) < log2_fixed(value + 1));
        }
        assert!(log2_fixed(u64::MAX) > log2_fixed(u64::MAX / 2));
    }
}
