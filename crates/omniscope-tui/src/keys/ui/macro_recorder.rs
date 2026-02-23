use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;

/// Records and replays key sequences (Vim macros).
#[derive(Debug, Clone, Default)]
pub struct MacroRecorder {
    /// Register currently being recorded into (None = not recording).
    pub recording_register: Option<char>,
    /// Buffer of keys recorded so far.
    pub buffer: Vec<(KeyCode, KeyModifiers)>,
    /// Stored macros keyed by register char.
    pub macros: HashMap<char, Vec<(KeyCode, KeyModifiers)>>,
    /// Last played register (for `@@`).
    pub last_played: Option<char>,
}

impl MacroRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start recording into register `reg`.
    pub fn start_recording(&mut self, reg: char) {
        self.recording_register = Some(reg);
        self.buffer.clear();
    }

    /// Stop recording and save the macro.
    pub fn stop_recording(&mut self) {
        if let Some(reg) = self.recording_register.take() {
            self.macros.insert(reg, self.buffer.clone());
            self.buffer.clear();
        }
    }

    /// Record a key event (called during recording).
    pub fn record_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if self.recording_register.is_some() {
            // Don't record the 'q' that stops recording
            self.buffer.push((code, modifiers));
        }
    }

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        self.recording_register.is_some()
    }

    /// Get the macro stored in register `reg`.
    pub fn get_macro(&self, reg: char) -> Option<&Vec<(KeyCode, KeyModifiers)>> {
        self.macros.get(&reg)
    }

    /// List all recorded macros (register + length).
    pub fn list_macros(&self) -> Vec<(char, usize)> {
        let mut result: Vec<_> = self
            .macros
            .iter()
            .map(|(&reg, keys)| (reg, keys.len()))
            .collect();
        result.sort_by_key(|(reg, _)| *reg);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_replay() {
        let mut rec = MacroRecorder::new();

        rec.start_recording('a');
        assert!(rec.is_recording());

        rec.record_key(KeyCode::Char('j'), KeyModifiers::NONE);
        rec.record_key(KeyCode::Char('j'), KeyModifiers::NONE);
        rec.record_key(KeyCode::Char('k'), KeyModifiers::NONE);

        rec.stop_recording();
        assert!(!rec.is_recording());

        let macro_a = rec.get_macro('a').unwrap();
        assert_eq!(macro_a.len(), 3);
        assert_eq!(macro_a[0], (KeyCode::Char('j'), KeyModifiers::NONE));
    }

    #[test]
    fn test_list_macros() {
        let mut rec = MacroRecorder::new();

        rec.start_recording('b');
        rec.record_key(KeyCode::Char('j'), KeyModifiers::NONE);
        rec.stop_recording();

        rec.start_recording('a');
        rec.record_key(KeyCode::Char('k'), KeyModifiers::NONE);
        rec.record_key(KeyCode::Char('k'), KeyModifiers::NONE);
        rec.stop_recording();

        let list = rec.list_macros();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], ('a', 2));
        assert_eq!(list[1], ('b', 1));
    }
}
