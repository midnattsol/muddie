//! Decides which incoming text is spoken and when.

use std::time::{Duration, Instant};

/// How long unterminated text must stay unchanged before it is spoken.
/// Servers that do not mark their prompts leave them unterminated, and a
/// line still in transit looks the same, so the narrator waits this long.
const SETTLE: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct Narrator {
    /// Unterminated text waiting to settle, and when it last changed.
    pending: Option<(String, Instant)>,
    /// The part of the current unterminated text already spoken.
    spoken: String,
}

impl Narrator {
    /// A line or prompt was completed; returns what to speak of it.
    pub fn line(&mut self, text: &str) -> Option<String> {
        self.pending = None;
        let spoken = std::mem::take(&mut self.spoken);
        speakable(text.strip_prefix(spoken.as_str()).unwrap_or(text))
    }

    /// The unterminated text at the end of the output is now `text`.
    pub fn partial(&mut self, text: &str, now: Instant) {
        let unchanged = match &self.pending {
            Some((pending, _)) => pending == text,
            None => self.spoken == text,
        };
        if !unchanged {
            self.pending = Some((text.to_owned(), now));
        }
    }

    /// Returns unterminated text that has settled and should be spoken now.
    pub fn poll(&mut self, now: Instant) -> Option<String> {
        if self.deadline()? > now {
            return None;
        }
        let (text, _) = self.pending.take()?;
        let rest = speakable(text.strip_prefix(self.spoken.as_str()).unwrap_or(&text));
        self.spoken = text;
        rest
    }

    /// When [`Self::poll`] should next be called, if anything is waiting.
    pub fn deadline(&self) -> Option<Instant> {
        self.pending.as_ref().map(|(_, since)| *since + SETTLE)
    }
}

fn speakable(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_spoken_and_blank_ones_skipped() {
        let mut narrator = Narrator::default();
        assert_eq!(
            narrator.line("  A goblin arrives. "),
            Some("A goblin arrives.".into())
        );
        assert_eq!(narrator.line("   "), None);
    }

    #[test]
    fn partial_is_spoken_once_settled_and_not_repeated() {
        let mut narrator = Narrator::default();
        let start = Instant::now();
        narrator.partial("Name: ", start);
        assert_eq!(narrator.poll(start + SETTLE / 2), None);
        // The same text arriving again must not restart the wait.
        narrator.partial("Name: ", start + SETTLE / 2);
        assert_eq!(narrator.poll(start + SETTLE), Some("Name:".into()));
        assert_eq!(narrator.deadline(), None);
        narrator.partial("Name: ", start + SETTLE * 2);
        assert_eq!(narrator.deadline(), None);
        // Completing the line speaks only what was not heard yet.
        assert_eq!(narrator.line("Name: "), None);
        assert_eq!(narrator.line("Name: "), Some("Name:".into()));
    }

    #[test]
    fn growing_partial_speaks_only_the_new_part() {
        let mut narrator = Narrator::default();
        let start = Instant::now();
        narrator.partial("The orc", start);
        assert_eq!(narrator.poll(start + SETTLE), Some("The orc".into()));
        narrator.partial("The orc hits", start + SETTLE);
        assert_eq!(narrator.poll(start + SETTLE * 2), Some("hits".into()));
        assert_eq!(narrator.line("The orc hits you."), Some("you.".into()));
    }

    #[test]
    fn line_arriving_before_settling_is_spoken_whole() {
        let mut narrator = Narrator::default();
        let start = Instant::now();
        narrator.partial("The orc", start);
        assert_eq!(
            narrator.line("The orc hits you."),
            Some("The orc hits you.".into())
        );
        assert_eq!(narrator.poll(start + SETTLE), None);
    }
}
