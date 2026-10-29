use crate::decoder::{self, Lock};
use crate::lexicon::MAX_PHRASE_SYLLABLES;
use crate::parser::{PhoneticBuffer, Syllable};
use crate::{
    Candidate, Event, InputMode, KeyEvent, Lexicon, MAX_SYLLABLES, Output, PAGE_SIZE, Snapshot,
    Status,
};
use std::{collections::BTreeMap, sync::Arc};

const MAX_CANDIDATES: usize = crate::lexicon::MAX_CANDIDATES_PER_READING;

/// One context's mutable composition. The immutable lexicon may be shared.
#[derive(Clone)]
pub struct Engine {
    lexicon: Arc<Lexicon>,
    syllables: Vec<Syllable>,
    pending: PhoneticBuffer,
    cursor: usize,
    locks: Vec<Lock>,
    text: String,
    offsets: Vec<usize>,
    candidates: Vec<Candidate>,
    selecting: bool,
    candidate_index: usize,
    generation: u64,
    mode: InputMode,
    active: bool,
    shift_pending: bool,
    shift_used: bool,
    held_keys: BTreeMap<String, usize>,
}

impl Engine {
    pub fn new(lexicon: Arc<Lexicon>) -> Self {
        Self {
            lexicon,
            syllables: vec![],
            pending: PhoneticBuffer::default(),
            cursor: 0,
            locks: vec![],
            text: String::new(),
            offsets: vec![0],
            candidates: vec![],
            selecting: false,
            candidate_index: 0,
            generation: 0,
            mode: InputMode::Chinese,
            active: true,
            shift_pending: false,
            shift_used: false,
            held_keys: BTreeMap::new(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut preedit = self.text.clone();
        let position = self.offsets[self.cursor];
        let pending = self.pending.text();
        preedit.insert_str(position, &pending);
        Snapshot {
            preedit,
            cursor: position + pending.len(),
            candidates: self.candidates.clone(),
            page: self.candidate_index / PAGE_SIZE,
            candidate_index: self.candidate_index,
            selecting: self.selecting,
            generation: self.generation,
            mode: self.mode,
            active: self.active,
        }
    }

    /// Simulate on a private value; shared lexicon data is immutable.
    pub fn probe(&self, key: &KeyEvent) -> bool {
        self.clone().handle(Event::Key(key.clone())).consumed
    }

    pub fn handle(&mut self, event: Event) -> Output {
        let mut commit = None;
        let mut status = None;
        let consumed = match event {
            Event::Activate => {
                self.active = true;
                self.bump();
                true
            }
            Event::Deactivate => {
                self.held_keys.clear();
                self.cancel();
                self.active = false;
                self.bump();
                true
            }
            Event::FocusLost | Event::Reset => {
                self.held_keys.clear();
                self.cancel();
                self.bump();
                true
            }
            Event::Select { generation, index } => {
                if generation != self.generation
                    || !self.selecting
                    || index >= self.candidates.len()
                {
                    status = Some(Status::StaleSelection);
                } else {
                    self.select(index);
                    self.bump();
                }
                true
            }
            Event::Key(key) => self.key(key, &mut commit, &mut status),
        };
        if consumed && self.rebuild().is_err() {
            // Never publish a partial commit after an internal state error.
            self.cancel();
            self.bump();
            commit = None;
            status = Some(Status::InternalError);
        }
        Output {
            consumed,
            snapshot: self.snapshot(),
            commit,
            status,
        }
    }

    fn key(
        &mut self,
        key: KeyEvent,
        commit: &mut Option<String>,
        status: &mut Option<Status>,
    ) -> bool {
        if !self.active {
            return false;
        }
        if key.key == "shift" {
            if key.release {
                // A second physical Shift can still be held after this release.
                if key.shift {
                    return false;
                }
                let toggle = self.shift_pending
                    && !self.shift_used
                    && self.held_keys.is_empty()
                    && !key.ctrl
                    && !key.alt
                    && !key.meta;
                self.shift_pending = false;
                self.shift_used = false;
                if toggle {
                    self.toggle_mode();
                    return true;
                }
            } else if !key.repeat && !self.shift_pending {
                self.shift_pending = true;
                self.shift_used = key.ctrl || key.alt || key.meta || !self.held_keys.is_empty();
            }
            return false;
        }
        if self.shift_pending {
            self.shift_used = true;
        }
        // Native adapters observe releases even when the event passes through.
        // Bounds prevent arbitrary public KeyEvents from growing this set.
        if key.key != "caps_lock" && !key.key.is_empty() && key.key.len() <= 32 {
            if key.release {
                if let Some(count) = self.held_keys.get_mut(&key.key) {
                    *count -= 1;
                    if *count == 0 {
                        self.held_keys.remove(&key.key);
                    }
                }
            } else if self.held_keys.len() < 128 || self.held_keys.contains_key(&key.key) {
                // Return and keypad Enter can share one logical name.
                // They remain held until both native releases arrive.
                let count = self.held_keys.entry(key.key.clone()).or_default();
                if key.repeat {
                    *count = (*count).max(1);
                } else {
                    *count = count.saturating_add(1);
                }
            }
        }
        if key.release {
            return false;
        }
        if key.ctrl || key.alt || key.meta {
            return false;
        }
        if key.key == "caps_lock" {
            if !key.repeat {
                self.toggle_mode();
                return true;
            }
            return false;
        }
        if self.mode == InputMode::Latin {
            return false;
        }
        if self.selecting {
            return self.selection_key(&key.key);
        }

        let occupied = !self.syllables.is_empty() || !self.pending.is_empty();
        match key.key.as_str() {
            "escape" if occupied => self.cancel(),
            "backspace" if occupied => {
                if !self.pending.backspace() && self.cursor > 0 {
                    self.cursor -= 1;
                    self.remove(self.cursor);
                }
            }
            "delete" if occupied => {
                if self.pending.is_empty() && self.cursor < self.syllables.len() {
                    self.remove(self.cursor);
                }
            }
            "left" | "right" | "home" | "end" if occupied => {
                if self.pending.is_empty() {
                    self.cursor = match key.key.as_str() {
                        "left" => self.cursor.saturating_sub(1),
                        "right" => (self.cursor + 1).min(self.syllables.len()),
                        "home" => 0,
                        _ => self.syllables.len(),
                    };
                }
            }
            "down" if !self.syllables.is_empty() && self.pending.is_empty() => {
                if !self.candidates.is_empty() {
                    self.selecting = true;
                    self.candidate_index = 0;
                }
            }
            "enter" if occupied => {
                if !self.pending.is_empty() {
                    *status = Some(Status::InvalidSyllable);
                } else {
                    *commit = Some(self.text.clone());
                    self.cancel();
                }
            }
            "space" if self.pending.is_empty() && !self.syllables.is_empty() => {
                *commit = Some(self.text.clone());
                self.cancel();
            }
            "space" | "6" | "3" | "4" | "7" if occupied => {
                let tone = match key.key.as_str() {
                    "6" => 2,
                    "3" => 3,
                    "4" => 4,
                    "7" => 5,
                    _ => 1,
                };
                match self.pending.finish(tone) {
                    Ok(syllable) if self.syllables.len() < MAX_SYLLABLES => {
                        if self
                            .lexicon
                            .lookup(std::slice::from_ref(&syllable))
                            .is_empty()
                        {
                            *status = Some(Status::UnknownReading);
                        }
                        self.insert(syllable);
                        self.pending.clear();
                    }
                    Ok(_) => *status = Some(Status::LimitReached),
                    Err(_) => *status = Some(Status::InvalidSyllable),
                }
            }
            _ => {
                let mut chars = key.key.chars();
                let Some(character) = chars.next().filter(|_| chars.next().is_none()) else {
                    return false;
                };
                // Validate on a copy so errors and limits preserve the old buffer.
                let mut next = self.pending.clone();
                match next.push_key(character) {
                    Ok(false) => return false,
                    Ok(true)
                        if self.pending.is_empty() && self.syllables.len() >= MAX_SYLLABLES =>
                    {
                        *status = Some(Status::LimitReached)
                    }
                    Ok(true) => {
                        if self.pending.is_empty() {
                            self.locks.retain(|lock| {
                                !(lock.start < self.cursor && self.cursor < lock.end)
                            });
                        }
                        self.pending = next;
                    }
                    Err(_) => *status = Some(Status::InvalidSyllable),
                }
            }
        }
        self.bump();
        true
    }

    fn selection_key(&mut self, key: &str) -> bool {
        let length = self.candidates.len();
        match key {
            "escape" => self.selecting = false,
            "enter" | "space" => self.select(self.candidate_index),
            "up" | "left" => self.candidate_index = self.candidate_index.saturating_sub(1),
            "down" | "right" => {
                self.candidate_index = (self.candidate_index + 1).min(length.saturating_sub(1))
            }
            "page_up" => self.candidate_index = self.candidate_index.saturating_sub(PAGE_SIZE),
            "page_down" => {
                self.candidate_index =
                    (self.candidate_index + PAGE_SIZE).min(length.saturating_sub(1))
            }
            value if value.len() == 1 && matches!(value.as_bytes()[0], b'1'..=b'9') => {
                let index = self.candidate_index / PAGE_SIZE * PAGE_SIZE
                    + usize::from(value.as_bytes()[0] - b'1');
                if index < length {
                    self.select(index);
                }
            }
            _ => return true,
        }
        self.bump();
        true
    }

    fn select(&mut self, index: usize) {
        let Some(candidate) = self.candidates.get(index).cloned() else {
            return;
        };
        self.locks
            .retain(|lock| lock.end <= candidate.start || lock.start >= candidate.end);
        self.locks.push(Lock {
            start: candidate.start,
            end: candidate.end,
            text: candidate.text,
        });
        self.locks.sort_by_key(|lock| lock.start);
        self.cursor = candidate.end;
        self.selecting = false;
        self.candidate_index = 0;
    }

    fn insert(&mut self, syllable: Syllable) {
        let position = self.cursor;
        self.locks
            .retain(|lock| !(lock.start < position && position < lock.end));
        for lock in &mut self.locks {
            if lock.start >= position {
                lock.start += 1;
                lock.end += 1;
            }
        }
        self.syllables.insert(position, syllable);
        self.cursor += 1;
    }

    fn remove(&mut self, position: usize) {
        self.locks
            .retain(|lock| !(lock.start <= position && position < lock.end));
        for lock in &mut self.locks {
            if lock.start > position {
                lock.start -= 1;
                lock.end -= 1;
            }
        }
        self.syllables.remove(position);
    }

    fn toggle_mode(&mut self) {
        self.cancel();
        self.mode = match self.mode {
            InputMode::Chinese => InputMode::Latin,
            InputMode::Latin => InputMode::Chinese,
        };
        self.bump();
    }

    fn cancel(&mut self) {
        self.syllables.clear();
        self.pending.clear();
        self.locks.clear();
        self.cursor = 0;
        self.text.clear();
        self.offsets = vec![0];
        self.candidates.clear();
        self.selecting = false;
        self.candidate_index = 0;
        self.shift_pending = false;
        self.shift_used = false;
    }

    fn bump(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    fn rebuild(&mut self) -> Result<(), decoder::DecoderError> {
        let decoded = decoder::decode(&self.lexicon, &self.syllables, &self.locks)?;
        self.text = decoded.text;
        self.offsets = decoded.offsets;
        if !self.selecting {
            self.candidates.clear();
            self.candidate_index = 0;
            if self.pending.is_empty() && !self.syllables.is_empty() {
                let start = self.cursor.min(self.syllables.len() - 1);
                let mut choices = vec![];
                for end in start + 1..=self.syllables.len().min(start + MAX_PHRASE_SYLLABLES) {
                    for entry in self.lexicon.lookup(&self.syllables[start..end]) {
                        // Explicit selection may replace intersecting locks;
                        // select() preserves only disjoint, untouched locks.
                        choices.push((
                            entry.frequency,
                            Candidate {
                                text: entry.text.clone(),
                                start,
                                end,
                            },
                        ));
                    }
                }
                choices.sort_by(|left, right| {
                    right
                        .0
                        .cmp(&left.0)
                        .then_with(|| left.1.text.cmp(&right.1.text))
                        .then_with(|| right.1.end.cmp(&left.1.end))
                });
                self.candidates = choices
                    .into_iter()
                    .take(MAX_CANDIDATES)
                    .map(|(_, candidate)| candidate)
                    .collect();
            }
        }
        Ok(())
    }
}
