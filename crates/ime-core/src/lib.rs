//! Deterministic, offline Zhuyin input engine. No platform or file I/O.
pub mod decoder;
mod engine;
pub mod lexicon;
pub mod parser;
pub use engine::Engine;
pub use lexicon::Lexicon;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyEvent {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
    pub shift: bool,
    pub repeat: bool,
    pub release: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Key(KeyEvent),
    Activate,
    Deactivate,
    FocusLost,
    Reset,
    Select { generation: u64, index: usize },
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        enum WireEvent {
            Key(KeyEvent),
            Activate {},
            Deactivate {},
            FocusLost {},
            Reset {},
            Select { generation: u64, index: usize },
        }
        Ok(match WireEvent::deserialize(deserializer)? {
            WireEvent::Key(key) => Self::Key(key),
            WireEvent::Activate {} => Self::Activate,
            WireEvent::Deactivate {} => Self::Deactivate,
            WireEvent::FocusLost {} => Self::FocusLost,
            WireEvent::Reset {} => Self::Reset,
            WireEvent::Select { generation, index } => Self::Select { generation, index },
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    Chinese,
    Latin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    InvalidSyllable,
    UnknownReading,
    LimitReached,
    StaleSelection,
    InternalError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    /// Inclusive syllable index, not a text byte offset.
    pub start: usize,
    /// Exclusive syllable index, not a text byte offset.
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub preedit: String,
    pub cursor: usize,
    pub candidates: Vec<Candidate>,
    pub page: usize,
    pub candidate_index: usize,
    pub selecting: bool,
    pub generation: u64,
    pub mode: InputMode,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub consumed: bool,
    pub snapshot: Snapshot,
    pub commit: Option<String>,
    pub status: Option<Status>,
}

pub const MAX_SYLLABLES: usize = 64;
pub const PAGE_SIZE: usize = 9;
