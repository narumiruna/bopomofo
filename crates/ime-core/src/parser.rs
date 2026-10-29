//! Standard Dachen keys and validated Mandarin Zhuyin syllables.
//!
//! Canonical syllables contain an initial, medial, and final in that order,
//! with absent slots omitted. First tone has no mark. Other tones use the
//! suffixes `ˊ`, `ˇ`, `ˋ`, and `˙`. Parsing also accepts an explicit `ˉ`
//! suffix and a leading neutral-tone mark, and normalizes both forms.
//! Syllable legality is independent of word frequency and lexical tone.

use serde::{Deserialize, Deserializer, Serialize};
use std::{error::Error, fmt};

/// A complete, canonical syllable. Deserialization validates the spelling.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Syllable(String);

impl Syllable {
    pub fn parse(input: &str) -> Result<Self, ParserError> {
        if input.is_empty() {
            return Err(ParserError::EmptySyllable);
        }
        // Three Zhuyin symbols and one tone mark need at most eleven bytes.
        if input.len() > 11 {
            return Err(ParserError::InvalidOrder);
        }
        let (body, tone) = if let Some(body) = input.strip_prefix('˙') {
            (body, 5)
        } else if let Some(last) = input.chars().next_back().and_then(tone_number) {
            let length = input.chars().next_back().unwrap().len_utf8();
            (&input[..input.len() - length], last)
        } else {
            (input, 1)
        };
        if body.is_empty() {
            return Err(ParserError::EmptySyllable);
        }
        let mut slots = Slots::default();
        let mut previous_slot = None;
        for symbol in body.chars() {
            let slot = symbol_slot(symbol).ok_or(ParserError::InvalidSymbol)?;
            if previous_slot.is_some_and(|previous| previous >= slot) {
                return Err(ParserError::InvalidOrder);
            }
            slots.set(slot, symbol);
            previous_slot = Some(slot);
        }
        syllable_from_slots(slots, tone)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Syllable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).map_err(serde::de::Error::custom)
    }
}

/// Error categories contain no input text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParserError {
    EmptySyllable,
    InvalidSymbol,
    InvalidOrder,
    InvalidCombination,
    InvalidTone,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptySyllable => "empty syllable",
            Self::InvalidSymbol => "invalid Zhuyin symbol",
            Self::InvalidOrder => "invalid Zhuyin slot order",
            Self::InvalidCombination => "invalid Mandarin syllable combination",
            Self::InvalidTone => "tone must be in 1..=5",
        })
    }
}

impl Error for ParserError {}

/// An unfinished syllable, stored in phonetic slot order.
///
/// Keys replace the same slot, but cannot discard another slot. A partial
/// spelling is accepted only if a legal syllable can complete it. Missing
/// slots can be entered out of order. Errors leave all slots unchanged.
/// Backspace removes the last occupied phonetic slot, not the last key event.
#[derive(Clone, Debug, Default)]
pub struct PhoneticBuffer {
    slots: Slots,
}

impl PhoneticBuffer {
    /// Returns false for a non-phonetic key, including all five tone keys.
    /// Letter keys must be lowercase; modifier handling belongs to Engine.
    pub fn push_key(&mut self, key: char) -> Result<bool, ParserError> {
        let Some(symbol) = dachen_symbol(key) else {
            return Ok(false);
        };
        let mut next = self.slots;
        // Every Dachen symbol has a phonetic slot.
        next.set(symbol_slot(symbol).unwrap(), symbol);
        if !next.can_complete() {
            return Err(ParserError::InvalidCombination);
        }
        self.slots = next;
        Ok(true)
    }

    /// Validates a complete syllable without clearing or changing the buffer.
    /// The caller clears it only after accepting the returned syllable.
    pub fn finish(&self, tone: u8) -> Result<Syllable, ParserError> {
        syllable_from_slots(self.slots, tone)
    }

    pub fn text(&self) -> String {
        self.slots.text()
    }

    pub fn backspace(&mut self) -> bool {
        for slot in [2, 1, 0] {
            if self.slots.get(slot) != '\0' {
                self.slots.set(slot, '\0');
                return true;
            }
        }
        false
    }

    pub fn is_empty(&self) -> bool {
        self.slots == Slots::default()
    }

    pub fn clear(&mut self) {
        self.slots = Slots::default();
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Slots {
    initial: char,
    medial: char,
    final_symbol: char,
}

impl Slots {
    fn get(self, slot: u8) -> char {
        match slot {
            0 => self.initial,
            1 => self.medial,
            _ => self.final_symbol,
        }
    }

    fn set(&mut self, slot: u8, value: char) {
        match slot {
            0 => self.initial = value,
            1 => self.medial = value,
            _ => self.final_symbol = value,
        }
    }

    fn text(self) -> String {
        [self.initial, self.medial, self.final_symbol]
            .into_iter()
            .filter(|symbol| *symbol != '\0')
            .collect()
    }

    fn is_complete(self) -> bool {
        let final_symbol = if self.final_symbol == '\0' {
            ' '
        } else {
            self.final_symbol
        };
        RIMES.iter().any(|&(initial, medial, finals)| {
            initial == self.initial && medial == self.medial && finals.contains(final_symbol)
        })
    }

    fn can_complete(self) -> bool {
        RIMES.iter().any(|&(initial, medial, finals)| {
            (self.initial == '\0' || self.initial == initial)
                && (self.medial == '\0' || self.medial == medial)
                && (self.final_symbol == '\0' || finals.contains(self.final_symbol))
        })
    }
}

fn syllable_from_slots(slots: Slots, tone: u8) -> Result<Syllable, ParserError> {
    let mark = match tone {
        1 => None,
        2 => Some('ˊ'),
        3 => Some('ˇ'),
        4 => Some('ˋ'),
        5 => Some('˙'),
        _ => return Err(ParserError::InvalidTone),
    };
    if slots == Slots::default() {
        return Err(ParserError::EmptySyllable);
    }
    if !slots.is_complete() {
        return Err(ParserError::InvalidCombination);
    }
    let mut text = slots.text();
    if let Some(mark) = mark {
        text.push(mark);
    }
    Ok(Syllable(text))
}

fn tone_number(mark: char) -> Option<u8> {
    match mark {
        'ˉ' => Some(1),
        'ˊ' => Some(2),
        'ˇ' => Some(3),
        'ˋ' => Some(4),
        '˙' => Some(5),
        _ => None,
    }
}

fn symbol_slot(symbol: char) -> Option<u8> {
    match symbol {
        'ㄅ'..='ㄙ' => Some(0),
        'ㄧ'..='ㄩ' => Some(1),
        'ㄚ'..='ㄦ' => Some(2),
        _ => None,
    }
}

fn dachen_symbol(key: char) -> Option<char> {
    Some(match key {
        '1' => 'ㄅ',
        'q' => 'ㄆ',
        'a' => 'ㄇ',
        'z' => 'ㄈ',
        '2' => 'ㄉ',
        'w' => 'ㄊ',
        's' => 'ㄋ',
        'x' => 'ㄌ',
        'e' => 'ㄍ',
        'd' => 'ㄎ',
        'c' => 'ㄏ',
        'r' => 'ㄐ',
        'f' => 'ㄑ',
        'v' => 'ㄒ',
        '5' => 'ㄓ',
        't' => 'ㄔ',
        'g' => 'ㄕ',
        'b' => 'ㄖ',
        'y' => 'ㄗ',
        'h' => 'ㄘ',
        'n' => 'ㄙ',
        'u' => 'ㄧ',
        'j' => 'ㄨ',
        'm' => 'ㄩ',
        '8' => 'ㄚ',
        'i' => 'ㄛ',
        'k' => 'ㄜ',
        ',' => 'ㄝ',
        '9' => 'ㄞ',
        'o' => 'ㄟ',
        'l' => 'ㄠ',
        '.' => 'ㄡ',
        '0' => 'ㄢ',
        'p' => 'ㄣ',
        ';' => 'ㄤ',
        '/' => 'ㄥ',
        '-' => 'ㄦ',
        _ => return None,
    })
}

// Each row is (initial, medial, allowed finals). NUL means no initial or
// medial; a space in the last column permits no final. The table includes
// rare standard syllables such as ㄉㄧㄚ, ㄋㄨㄣ, ㄖㄨㄚ, ㄌㄛ, and ㄧㄞ.
// CNS11643 common/less-common readings also require ㄆㄧㄚ, ㄈㄧㄠ,
// ㄋㄨㄟ, ㄌㄩㄢ, and ㄙㄟ; see data/cns11643/README.md.
// Apical vowels are implicit after ㄓ ㄔ ㄕ ㄖ ㄗ ㄘ ㄙ, never written ㄧ.
// ㄩ is never interchangeable with ㄨ, including after ㄐ ㄑ ㄒ.
const RIMES: &[(char, char, &str)] = &[
    ('\0', '\0', "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ"),
    ('\0', 'ㄧ', " ㄚㄛㄝㄞㄠㄡㄢㄣㄤㄥ"),
    ('\0', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤㄥ"),
    ('\0', 'ㄩ', " ㄝㄢㄣㄥ"),
    ('ㄅ', '\0', "ㄚㄛㄞㄟㄠㄢㄣㄤㄥ"),
    ('ㄅ', 'ㄧ', " ㄝㄠㄢㄣㄥ"),
    ('ㄅ', 'ㄨ', " "),
    ('ㄆ', '\0', "ㄚㄛㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄆ', 'ㄧ', " ㄚㄝㄠㄢㄣㄥ"),
    ('ㄆ', 'ㄨ', " "),
    ('ㄇ', '\0', "ㄚㄛㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄇ', 'ㄧ', " ㄝㄠㄡㄢㄣㄥ"),
    ('ㄇ', 'ㄨ', " "),
    ('ㄈ', '\0', "ㄚㄛㄟㄡㄢㄣㄤㄥ"),
    ('ㄈ', 'ㄧ', "ㄠ"),
    ('ㄈ', 'ㄨ', " "),
    ('ㄉ', '\0', "ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄉ', 'ㄧ', " ㄚㄝㄠㄡㄢㄥ"),
    ('ㄉ', 'ㄨ', " ㄛㄟㄢㄣㄥ"),
    ('ㄊ', '\0', "ㄚㄜㄞㄟㄠㄡㄢㄤㄥ"),
    ('ㄊ', 'ㄧ', " ㄝㄠㄢㄥ"),
    ('ㄊ', 'ㄨ', " ㄛㄟㄢㄣㄥ"),
    ('ㄋ', '\0', "ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄋ', 'ㄧ', " ㄝㄠㄡㄢㄣㄤㄥ"),
    ('ㄋ', 'ㄨ', " ㄛㄟㄢㄣㄥ"),
    ('ㄋ', 'ㄩ', " ㄝ"),
    ('ㄌ', '\0', "ㄚㄛㄜㄞㄟㄠㄡㄢㄤㄥ"),
    ('ㄌ', 'ㄧ', " ㄚㄝㄠㄡㄢㄣㄤㄥ"),
    ('ㄌ', 'ㄨ', " ㄛㄢㄣㄥ"),
    ('ㄌ', 'ㄩ', " ㄝㄢ"),
    ('ㄍ', '\0', "ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄍ', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤㄥ"),
    ('ㄎ', '\0', "ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄎ', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤㄥ"),
    ('ㄏ', '\0', "ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄏ', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤㄥ"),
    ('ㄐ', 'ㄧ', " ㄚㄝㄠㄡㄢㄣㄤㄥ"),
    ('ㄐ', 'ㄩ', " ㄝㄢㄣㄥ"),
    ('ㄑ', 'ㄧ', " ㄚㄝㄠㄡㄢㄣㄤㄥ"),
    ('ㄑ', 'ㄩ', " ㄝㄢㄣㄥ"),
    ('ㄒ', 'ㄧ', " ㄚㄝㄠㄡㄢㄣㄤㄥ"),
    ('ㄒ', 'ㄩ', " ㄝㄢㄣㄥ"),
    ('ㄓ', '\0', " ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄓ', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤㄥ"),
    ('ㄔ', '\0', " ㄚㄜㄞㄠㄡㄢㄣㄤㄥ"),
    ('ㄔ', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤㄥ"),
    ('ㄕ', '\0', " ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄕ', 'ㄨ', " ㄚㄛㄞㄟㄢㄣㄤ"),
    ('ㄖ', '\0', " ㄜㄠㄡㄢㄣㄤㄥ"),
    ('ㄖ', 'ㄨ', " ㄚㄛㄟㄢㄣㄥ"),
    ('ㄗ', '\0', " ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄗ', 'ㄨ', " ㄛㄟㄢㄣㄥ"),
    ('ㄘ', '\0', " ㄚㄜㄞㄠㄡㄢㄣㄤㄥ"),
    ('ㄘ', 'ㄨ', " ㄛㄟㄢㄣㄥ"),
    ('ㄙ', '\0', " ㄚㄜㄞㄟㄠㄡㄢㄣㄤㄥ"),
    ('ㄙ', 'ㄨ', " ㄛㄟㄢㄣㄥ"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::{IntoDeserializer, value::Error as DeserializeError};
    use std::collections::BTreeSet;

    fn buffer(keys: &str) -> PhoneticBuffer {
        let mut result = PhoneticBuffer::default();
        for key in keys.chars() {
            assert_eq!(result.push_key(key), Ok(true), "key {key}");
        }
        result
    }

    #[test]
    fn all_dachen_symbols_and_non_phonetic_keys() {
        let keys = "1qaz2wsxedcrfv5tgbyhnu jm8ik,9ol.0p;/-".replace(' ', "");
        let symbols = "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ";
        assert_eq!(keys.chars().count(), 37);
        for (key, expected) in keys.chars().zip(symbols.chars()) {
            let mut buffer = PhoneticBuffer::default();
            assert_eq!(buffer.push_key(key), Ok(true));
            assert_eq!(buffer.text(), expected.to_string());
        }
        let mut buffer = buffer("su");
        for key in ['6', '3', '4', '7', ' ', 'A', '!', 'ㄅ', '\n', '😀'] {
            assert_eq!(buffer.push_key(key), Ok(false));
            assert_eq!(buffer.text(), "ㄋㄧ");
        }
    }

    #[test]
    fn five_tones_and_normalization() {
        let buffer = buffer("a8");
        for (tone, expected) in [
            (1, "ㄇㄚ"),
            (2, "ㄇㄚˊ"),
            (3, "ㄇㄚˇ"),
            (4, "ㄇㄚˋ"),
            (5, "ㄇㄚ˙"),
        ] {
            assert_eq!(buffer.finish(tone).unwrap().as_str(), expected);
            assert_eq!(Syllable::parse(expected).unwrap().as_str(), expected);
        }
        assert_eq!(Syllable::parse("ㄇㄚˉ").unwrap().as_str(), "ㄇㄚ");
        assert_eq!(Syllable::parse("˙ㄇㄚ").unwrap().as_str(), "ㄇㄚ˙");
        assert_eq!(buffer.finish(0), Err(ParserError::InvalidTone));
        assert_eq!(buffer.finish(6), Err(ParserError::InvalidTone));
        assert_eq!(buffer.text(), "ㄇㄚ");
    }

    #[test]
    fn legal_and_illegal_combinations() {
        for input in [
            "ㄓ",
            "ㄔˊ",
            "ㄕˇ",
            "ㄖˋ",
            "ㄗ˙",
            "ㄘ",
            "ㄙ",
            "ㄦˊ",
            "ㄝˋ",
            "ㄋㄩˇ",
            "ㄌㄩㄝˋ",
            "ㄐㄩㄥ",
            "ㄑㄩㄢˊ",
            "ㄒㄩㄣˊ",
            "ㄧㄞˊ",
            "ㄧㄛ˙",
            "ㄨㄥ",
            "ㄩㄝˋ",
            "ㄉㄧㄚˇ",
            "ㄋㄨㄣˊ",
            "ㄖㄨㄚˊ",
            "ㄌㄛ˙",
            "ㄆㄧㄚ",
            "ㄈㄧㄠˋ",
            "ㄋㄨㄟˊ",
            "ㄌㄩㄢˊ",
            "ㄌㄩㄢˇ",
            "ㄙㄟ",
        ] {
            assert!(Syllable::parse(input).is_ok(), "{input}");
        }
        for input in [
            "",
            "ㄅ",
            "ㄐ",
            "ㄓㄧ",
            "ㄘㄧ",
            "ㄖㄧ",
            "ㄅㄩ",
            "ㄆㄩ",
            "ㄇㄩ",
            "ㄈㄧ",
            "ㄅㄧㄡ",
            "ㄉㄧㄣ",
            "ㄊㄧㄤ",
            "ㄋㄩㄢ",
            "ㄌㄩㄣ",
            "ㄍㄧ",
            "ㄎㄩ",
            "ㄏㄩ",
            "ㄐㄨ",
            "ㄑㄨㄢ",
            "ㄒㄨㄥ",
            "ㄓㄩ",
            "ㄕㄨㄥ",
            "ㄗㄨㄚ",
            "ㄧㄜ",
            "ㄨㄝ",
            "ㄩㄚ",
            "ㄩㄡ",
            "ㄅㄦ",
            "ㄧㄦ",
            "ㄦㄦ",
            "ㄇㄅㄚ",
            "ㄚㄇ",
            "ㄚㄧ",
            "ㄇㄚˊˋ",
            "ˊㄇㄚ",
            "˙ㄇㄚ˙",
            "ㄇˇㄚ",
            "ㄇㄚ1",
            "ma",
            " ㄇㄚ",
            "ㄇㄚ\n",
            "ˉ",
            "ㄫ",
        ] {
            assert!(Syllable::parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn same_slot_replacement_and_failed_edits_preserve_state() {
        let mut buffer = buffer("su0");
        assert_eq!(buffer.text(), "ㄋㄧㄢ");
        assert_eq!(buffer.push_key('x'), Ok(true));
        assert_eq!(buffer.text(), "ㄌㄧㄢ");
        assert_eq!(buffer.push_key(';'), Ok(true));
        assert_eq!(buffer.text(), "ㄌㄧㄤ");
        assert_eq!(buffer.push_key('e'), Err(ParserError::InvalidCombination));
        assert_eq!(buffer.text(), "ㄌㄧㄤ");
        assert_eq!(buffer.push_key('m'), Err(ParserError::InvalidCombination));
        assert_eq!(buffer.finish(3).unwrap().as_str(), "ㄌㄧㄤˇ");
        assert_eq!(buffer.text(), "ㄌㄧㄤ");
        let mut buffer = super::tests::buffer("sm");
        assert_eq!(buffer.push_key('1'), Err(ParserError::InvalidCombination));
        assert_eq!(buffer.text(), "ㄋㄩ");
    }

    #[test]
    fn out_of_order_slots_and_backspace() {
        let mut buffer = buffer("r8");
        assert_eq!(buffer.text(), "ㄐㄚ");
        assert_eq!(buffer.finish(1), Err(ParserError::InvalidCombination));
        assert_eq!(buffer.push_key('u'), Ok(true));
        assert_eq!(buffer.finish(1).unwrap().as_str(), "ㄐㄧㄚ");
        for expected in ["ㄐㄧ", "ㄐ", ""] {
            assert!(buffer.backspace());
            assert_eq!(buffer.text(), expected);
        }
        assert!(!buffer.backspace());
        assert!(buffer.is_empty());
        assert_eq!(buffer.finish(1), Err(ParserError::EmptySyllable));
        assert_eq!(buffer.push_key('-'), Ok(true));
        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn deletion_of_a_medial_can_be_repaired() {
        let mut buffer = buffer("fm,");
        assert!(buffer.backspace());
        assert!(buffer.backspace());
        assert_eq!(buffer.text(), "ㄑ");
        assert_eq!(buffer.push_key('u'), Ok(true));
        assert_eq!(buffer.push_key(','), Ok(true));
        assert_eq!(buffer.finish(4).unwrap().as_str(), "ㄑㄧㄝˋ");
    }

    #[test]
    fn every_table_syllable_round_trips_through_keys_and_all_tones() {
        let mut seen = BTreeSet::new();
        let key_by_symbol = "1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-"
            .chars()
            .map(|key| (dachen_symbol(key).unwrap(), key))
            .collect::<std::collections::BTreeMap<_, _>>();
        for &(initial, medial, finals) in RIMES {
            for final_symbol in finals.chars() {
                let body: String = [initial, medial, final_symbol]
                    .into_iter()
                    .filter(|c| *c != '\0' && *c != ' ')
                    .collect();
                assert!(seen.insert(body.clone()), "duplicate table spelling");
                let keys: String = body.chars().map(|c| key_by_symbol[&c]).collect();
                for tone in 1..=5 {
                    let syllable = buffer(&keys).finish(tone).unwrap();
                    assert_eq!(Syllable::parse(syllable.as_str()).unwrap(), syllable);
                }
            }
        }
        assert!(seen.len() >= 400);
    }

    #[test]
    fn serde_deserialization_cannot_bypass_validation() {
        let valid = <&str as IntoDeserializer<'_, DeserializeError>>::into_deserializer("˙ㄉㄜ");
        assert_eq!(Syllable::deserialize(valid).unwrap().as_str(), "ㄉㄜ˙");
        let invalid = <&str as IntoDeserializer<'_, DeserializeError>>::into_deserializer("ㄅㄩ");
        assert!(Syllable::deserialize(invalid).is_err());
    }
}
