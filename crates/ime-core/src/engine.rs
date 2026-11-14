//! Context-local libchewing integration. No input history, file I/O or learning.
use crate::assets::{MAX_CANDIDATES, ReadOnlyDictionary};
use crate::{
    Assets, Candidate, Event, InputMode, KeyEvent, MAX_SYLLABLES, Output, PAGE_SIZE, Snapshot,
    Status,
};
use chewing::conversion::ChewingEngine;
use chewing::dictionary::{Dictionary, Layered, LookupStrategy};
use chewing::editor::zhuyin_layout::{KeyBehavior, Standard, SyllableEditor};
use chewing::editor::{
    AbbrevTable, BasicEditor, Editor, EditorKeyBehavior, LaxUserFreqEstimate, SymbolSelector,
};
use chewing::input::{
    KeyboardEvent,
    keymap::{QWERTY_MAP, map_ascii},
    keysym::*,
};
use chewing::zhuyin::Bopomofo;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Default)]
struct Modifiers {
    shift_pending: bool,
    shift_used: bool,
    held: BTreeMap<String, usize>,
}
#[derive(PartialEq, Eq)]
enum Route {
    Pass,
    Toggle,
    Input,
}
impl Modifiers {
    fn route(
        &mut self,
        key: &KeyEvent,
        active: bool,
        mode: InputMode,
        occupied: bool,
        selecting: bool,
    ) -> Route {
        if !active {
            return Route::Pass;
        }
        if key.key == "shift" {
            if key.release {
                if key.shift {
                    return Route::Pass;
                }
                let toggle = self.shift_pending
                    && !self.shift_used
                    && self.held.is_empty()
                    && !key.ctrl
                    && !key.alt
                    && !key.meta;
                self.shift_pending = false;
                self.shift_used = false;
                return if toggle { Route::Toggle } else { Route::Pass };
            }
            if !key.repeat && !self.shift_pending {
                self.shift_pending = true;
                self.shift_used = key.ctrl || key.alt || key.meta || !self.held.is_empty();
            }
            return Route::Pass;
        }
        if self.shift_pending {
            self.shift_used = true;
        }
        if key.key != "caps_lock" && !key.key.is_empty() && key.key.len() <= 32 {
            if key.release {
                if let Some(n) = self.held.get_mut(&key.key) {
                    *n -= 1;
                    if *n == 0 {
                        self.held.remove(&key.key);
                    }
                }
            } else if self.held.len() < 128 || self.held.contains_key(&key.key) {
                let n = self.held.entry(key.key.clone()).or_default();
                *n = if key.repeat {
                    (*n).max(1)
                } else {
                    n.saturating_add(1)
                };
            }
        }
        if key.release {
            return Route::Pass;
        }
        if key.key == "caps_lock" {
            return if key.repeat {
                Route::Pass
            } else {
                Route::Toggle
            };
        }
        if key.ctrl || key.alt || key.meta || mode == InputMode::Latin {
            return Route::Pass;
        }
        if selecting || phonetic(&key.key) || (occupied && special(&key.key)) {
            Route::Input
        } else {
            Route::Pass
        }
    }
}

fn special(key: &str) -> bool {
    matches!(
        key,
        "space"
            | "enter"
            | "escape"
            | "backspace"
            | "delete"
            | "left"
            | "right"
            | "up"
            | "down"
            | "home"
            | "end"
            | "page_up"
            | "page_down"
            | "tab"
    )
}
fn native(key: &str) -> Option<KeyboardEvent> {
    let sym = match key {
        "space" => return Some(map_ascii(&QWERTY_MAP, b' ')),
        "enter" => SYM_RETURN,
        "escape" => SYM_ESC,
        "backspace" => SYM_BACKSPACE,
        "delete" => SYM_DELETE,
        "left" => SYM_LEFT,
        "right" => SYM_RIGHT,
        "up" => SYM_UP,
        "down" => SYM_DOWN,
        "home" => SYM_HOME,
        "end" => SYM_END,
        "page_up" => SYM_PAGEUP,
        "page_down" => SYM_PAGEDOWN,
        "tab" => SYM_TAB,
        value if value.len() == 1 && value.is_ascii() => {
            let event = map_ascii(&QWERTY_MAP, value.as_bytes()[0]);
            return (event.state == 0).then_some(event);
        }
        _ => return None,
    };
    Some(KeyboardEvent::builder().ksym(sym).build())
}
fn phonetic(key: &str) -> bool {
    // Ask the upstream layout, rather than keeping a second Dachen table.
    native(key)
        .is_some_and(|event| !matches!(Standard::new().key_press(event), KeyBehavior::KeyError))
}
fn tone(key: &str) -> Option<Bopomofo> {
    match key {
        "space" => Some(Bopomofo::TONE1),
        "6" => Some(Bopomofo::TONE2),
        "3" => Some(Bopomofo::TONE3),
        "4" => Some(Bopomofo::TONE4),
        "7" => Some(Bopomofo::TONE5),
        _ => None,
    }
}

pub struct Engine {
    assets: Arc<Assets>,
    editor: Editor,
    modifiers: Modifiers,
    candidate_index: usize,
    generation: u64,
    mode: InputMode,
    active: bool,
}
impl Engine {
    pub fn new(assets: Arc<Assets>) -> Self {
        let dict = Layered::new(vec![Box::new(ReadOnlyDictionary(Arc::clone(
            &assets.dictionary,
        )))]);
        let mut editor = Editor::new(
            Box::new(ChewingEngine::new()),
            dict,
            LaxUserFreqEstimate::new(0),
            AbbrevTable::new(),
            SymbolSelector::new(b"".as_slice()).expect("empty symbol table"),
        );
        editor.set_editor_options(|o| {
            o.disable_auto_learn_phrase = true;
            o.auto_commit_threshold = usize::MAX;
            o.candidates_per_page = PAGE_SIZE;
            o.esc_clear_all_buffer = true;
            o.enable_fullwidth_toggle_key = false;
            o.sort_candidates_by_frequency = true;
        });
        Self {
            assets,
            editor,
            modifiers: Modifiers::default(),
            candidate_index: 0,
            generation: 0,
            mode: InputMode::Chinese,
            active: true,
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        // Reuse one upstream conversion for text and offsets; do not convert twice.
        let intervals: Vec<_> = self.editor.intervals().collect();
        let mut preedit: String = intervals.iter().map(|interval| &*interval.text).collect();
        let mut cursor = preedit.len();
        let position = self.editor.cursor();
        let mut bytes = 0;
        for interval in intervals {
            if position < interval.end {
                cursor = bytes
                    + interval
                        .text
                        .chars()
                        .take(position.saturating_sub(interval.start))
                        .map(char::len_utf8)
                        .sum::<usize>();
                break;
            }
            bytes += interval.text.len();
        }
        let pending = self.editor.syllable_buffer_display();
        preedit.insert_str(cursor, &pending);
        cursor += pending.len();
        let selecting = self.editor.is_selecting();
        let candidates = if selecting {
            let start = position;
            self.editor
                .all_candidates()
                .unwrap_or_default()
                .into_iter()
                .take(MAX_CANDIDATES)
                .map(|text| {
                    let end = start + text.chars().count();
                    Candidate { text, start, end }
                })
                .collect()
        } else {
            vec![]
        };
        Snapshot {
            preedit,
            cursor,
            candidates,
            page: self.candidate_index / PAGE_SIZE,
            candidate_index: self.candidate_index,
            selecting,
            generation: self.generation,
            mode: self.mode,
            active: self.active,
        }
    }
    /// Only modifier bookkeeping is copied. No Editor call or history replay occurs.
    pub fn probe(&self, key: &KeyEvent) -> bool {
        self.modifiers.clone().route(
            key,
            self.active,
            self.mode,
            self.occupied(),
            self.editor.is_selecting(),
        ) != Route::Pass
    }
    fn occupied(&self) -> bool {
        !self.editor.is_empty() || self.editor.entering_syllable()
    }
    fn cancel(&mut self) {
        self.editor.clear();
        self.editor.cancel_entering_syllable();
        self.editor.ack();
        self.candidate_index = 0;
        self.modifiers.shift_pending = false;
        self.modifiers.shift_used = false;
    }
    fn select(&mut self, index: usize) -> Result<(), ()> {
        self.editor.select(index).map_err(|_| ())?;
        self.candidate_index = 0;
        Ok(())
    }
    pub fn handle(&mut self, event: Event) -> Output {
        let mut status = None;
        let consumed = match event {
            Event::Activate => {
                self.active = true;
                true
            }
            Event::Deactivate => {
                self.cancel();
                self.modifiers = Modifiers::default();
                self.active = false;
                true
            }
            Event::Reset | Event::FocusLost => {
                self.cancel();
                self.modifiers = Modifiers::default();
                true
            }
            Event::Select { generation, index } => {
                if generation != self.generation
                    || !self.editor.is_selecting()
                    || index >= self.snapshot().candidates.len()
                {
                    status = Some(Status::StaleSelection);
                } else if self.select(index).is_err() {
                    status = Some(Status::InternalError);
                }
                true
            }
            Event::Key(key) => {
                let occupied = self.occupied();
                let selecting = self.editor.is_selecting();
                match self
                    .modifiers
                    .route(&key, self.active, self.mode, occupied, selecting)
                {
                    Route::Pass => false,
                    Route::Toggle => {
                        self.cancel();
                        self.mode = match self.mode {
                            InputMode::Chinese => InputMode::Latin,
                            InputMode::Latin => InputMode::Chinese,
                        };
                        true
                    }
                    Route::Input => {
                        self.input(&key.key, &mut status);
                        true
                    }
                }
            }
        };
        let mut commit = if self.editor.display_commit().is_empty() {
            None
        } else {
            Some(self.editor.display_commit().to_owned())
        };
        self.editor.ack();
        if status == Some(Status::InternalError) {
            self.cancel();
            commit = None;
        }
        if consumed && status != Some(Status::StaleSelection) {
            self.generation = self.generation.wrapping_add(1);
        }
        Output {
            consumed,
            snapshot: self.snapshot(),
            commit,
            status,
        }
    }
    fn input(&mut self, key: &str, status: &mut Option<Status>) {
        if self.editor.is_selecting() {
            let count = self.snapshot().candidates.len();
            match key {
                "escape" => {
                    let _ = self.editor.cancel_selecting();
                    self.candidate_index = 0;
                }
                "enter" | "space" => {
                    if self.select(self.candidate_index).is_err() {
                        *status = Some(Status::InternalError);
                    }
                }
                "up" | "left" => self.candidate_index = self.candidate_index.saturating_sub(1),
                "down" | "right" => {
                    self.candidate_index = (self.candidate_index + 1).min(count.saturating_sub(1))
                }
                "page_up" => self.candidate_index = self.candidate_index.saturating_sub(PAGE_SIZE),
                "page_down" => {
                    self.candidate_index =
                        (self.candidate_index + PAGE_SIZE).min(count.saturating_sub(1))
                }
                "tab" => {
                    let result = if self.editor.has_next_selection_point() {
                        self.editor.jump_to_next_selection_point()
                    } else {
                        self.editor.jump_to_first_selection_point()
                    };
                    if result.is_err() {
                        *status = Some(Status::InternalError);
                    }
                    self.candidate_index = 0;
                }
                k if k.len() == 1 && (b'1'..=b'9').contains(&k.as_bytes()[0]) => {
                    let index = self.candidate_index / PAGE_SIZE * PAGE_SIZE
                        + usize::from(k.as_bytes()[0] - b'1');
                    if index < count && self.select(index).is_err() {
                        *status = Some(Status::InternalError);
                    }
                }
                _ => {}
            }
            return;
        }
        if key == "escape" {
            self.cancel();
            return;
        }
        if self.editor.entering_syllable() && special(key) && key != "space" && key != "backspace" {
            // libchewing Down clears a pending syllable. Never send a destructive transition.
            if key == "enter" {
                *status = Some(Status::InvalidSyllable);
            }
            return;
        }
        if let Some(tone) = tone(key) {
            if self.editor.entering_syllable() {
                let mut reading = self.editor.syllable_buffer();
                if tone != Bopomofo::TONE1 {
                    reading.update(tone);
                }
                if self
                    .assets
                    .dictionary
                    .lookup(&[reading], LookupStrategy::Standard)
                    .is_empty()
                {
                    // Upstream discards an unknown spelling. Keep the editable buffer instead.
                    *status = Some(Status::UnknownReading);
                    return;
                }
            } else if key == "space" {
                if self.editor.commit().is_err() {
                    *status = Some(Status::InternalError);
                }
                return;
            } else {
                *status = Some(Status::InvalidSyllable);
                return;
            }
        }
        if !self.editor.entering_syllable() && self.editor.len() >= MAX_SYLLABLES && phonetic(key) {
            *status = Some(Status::LimitReached);
            return;
        }
        if let Some(event) = native(key)
            && self.editor.process_keyevent(event) == EditorKeyBehavior::Bell
        {
            *status = Some(Status::InvalidSyllable);
        }
    }
}
