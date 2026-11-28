//! Tab completion of the word being typed, from words seen recently.

/// Shortest word offered as a completion.
const MIN_WORD: usize = 3;

#[derive(Default)]
pub struct Completion {
    /// The cycle in progress: the input before the word, the candidates,
    /// which one is showing, and the input it produced.
    cycle: Option<(String, Vec<String>, usize, String)>,
}

impl Completion {
    /// Completes the last word of `input` with the first of `words` that
    /// starts with it, ignoring case; pressing again moves to the next.
    /// `words` are offered in the order given, so put the most recent
    /// first. Returns false if there is nothing to complete.
    pub fn complete<'a>(
        &mut self,
        input: &mut String,
        words: impl Iterator<Item = &'a str>,
    ) -> bool {
        if let Some((head, candidates, index, shown)) = &mut self.cycle
            && shown == input
        {
            *index = (*index + 1) % candidates.len();
            *input = format!("{head}{}", candidates[*index]);
            shown.clone_from(input);
            return true;
        }
        // After the last space, whatever its length: a no-break space,
        // AltGr+Space, is two bytes.
        let start = (input.char_indices())
            .rfind(|(_, ch)| ch.is_whitespace())
            .map_or(0, |(at, ch)| at + ch.len_utf8());
        let (head, stem) = input.split_at(start);
        if stem.is_empty() {
            self.cycle = None;
            return false;
        }
        let stem_lower = stem.to_lowercase();
        let mut candidates: Vec<String> = Vec::new();
        for word in words {
            let fits = word.len() > stem.len()
                && word.chars().count() >= MIN_WORD
                && word.to_lowercase().starts_with(&stem_lower);
            if fits
                && !candidates
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(word))
            {
                candidates.push(word.to_owned());
            }
        }
        if candidates.is_empty() {
            self.cycle = None;
            return false;
        }
        // The stem itself closes the cycle, to get back what was typed.
        candidates.push(stem.to_owned());
        let head = head.to_owned();
        *input = format!("{head}{}", candidates[0]);
        self.cycle = Some((head, candidates, 0, input.clone()));
        true
    }
}

/// The words of `text`: runs of letters, digits, `'` and `-`.
pub fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch: char| !(ch.is_alphanumeric() || ch == '\'' || ch == '-'))
        .filter(|word| !word.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_and_cycles_through_candidates() {
        let seen = "A goblin and a Gorgon guard the golden gate";
        let mut completion = Completion::default();
        let mut input = "kill go".to_owned();
        assert!(completion.complete(&mut input, words(seen)));
        assert_eq!(input, "kill goblin");
        assert!(completion.complete(&mut input, words(seen)));
        assert_eq!(input, "kill Gorgon");
        assert!(completion.complete(&mut input, words(seen)));
        assert_eq!(input, "kill golden");
        assert!(completion.complete(&mut input, words(seen)));
        assert_eq!(input, "kill go");
        assert!(completion.complete(&mut input, words(seen)));
        assert_eq!(input, "kill goblin");
    }

    #[test]
    fn typing_after_a_completion_starts_afresh() {
        let seen = "goblin gate";
        let mut completion = Completion::default();
        let mut input = "go".to_owned();
        completion.complete(&mut input, words(seen));
        input.push_str(" ga");
        assert!(completion.complete(&mut input, words(seen)));
        assert_eq!(input, "goblin gate");
    }

    #[test]
    fn nothing_to_complete() {
        let mut completion = Completion::default();
        for text in ["", "look ", "xyz"] {
            let mut input = text.to_owned();
            assert!(!completion.complete(&mut input, words("goblin gate")));
            assert_eq!(input, text);
        }
        assert_eq!(
            words("it's a well-known half-orc!").collect::<Vec<_>>(),
            ["it's", "a", "well-known", "half-orc"]
        );
    }

    /// A no-break space (AltGr+Space on many keyboards) is whitespace of
    /// two bytes: completing the word after it must not panic.
    #[test]
    fn a_word_after_a_no_break_space_is_completed() {
        let mut completion = Completion::default();
        let mut input = "kill\u{a0}go".to_owned();
        assert!(completion.complete(&mut input, words("goblin")));
        assert_eq!(input, "kill\u{a0}goblin");
    }
}
