const MAX_HISTORY_SIZE: usize = 100;

#[derive(Debug, Clone, Default)]
pub struct InputHistory {
    entries: Vec<String>,
    index: Option<usize>,
}

impl InputHistory {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: None,
        }
    }

    pub fn push(&mut self, input: String) {
        if input.trim().is_empty() {
            return;
        }

        if self.entries.last() == Some(&input) {
            return;
        }

        self.entries.push(input);

        if self.entries.len() > MAX_HISTORY_SIZE {
            self.entries.remove(0);
        }

        self.index = None;
    }

    #[must_use]
    pub fn prev(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        let new_index = match self.index {
            None => self.entries.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };

        self.index = Some(new_index);
        self.entries.get(new_index).cloned()
    }

    #[must_use]
    pub fn next(&mut self) -> Option<String> {
        let i = self.index?;

        if i >= self.entries.len() - 1 {
            self.index = None;
            return None;
        }

        self.index = Some(i + 1);
        self.entries.get(i + 1).cloned()
    }

    pub const fn reset_index(&mut self) {
        self.index = None;
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_empty_and_whitespace() {
        let mut history = InputHistory::new();
        history.push(String::new());
        history.push("  ".to_string());
        history.push("\t\n".to_string());

        assert!(history.is_empty());
    }

    #[test]
    fn deduplicates_consecutive() {
        let mut history = InputHistory::new();
        history.push("first".to_string());
        history.push("first".to_string());
        history.push("second".to_string());
        history.push("first".to_string());

        assert_eq!(history.len(), 3);
    }

    #[test]
    fn navigation() {
        let mut history = InputHistory::new();
        history.push("one".to_string());
        history.push("two".to_string());
        history.push("three".to_string());

        assert_eq!(history.prev(), Some("three".to_string()));
        assert_eq!(history.prev(), Some("two".to_string()));
        assert_eq!(history.prev(), Some("one".to_string()));
        assert_eq!(history.prev(), Some("one".to_string()));

        assert_eq!(history.next(), Some("two".to_string()));
        assert_eq!(history.next(), Some("three".to_string()));
        assert_eq!(history.next(), None);
    }

    #[test]
    fn enforces_size_limit() {
        let mut history = InputHistory::new();

        for i in 0..150 {
            history.push(format!("entry {i}"));
        }

        assert_eq!(history.len(), MAX_HISTORY_SIZE);

        history.reset_index();
        let mut oldest = None;
        for _ in 0..MAX_HISTORY_SIZE {
            oldest = history.prev();
        }
        assert_eq!(oldest, Some("entry 50".to_string()));
    }
}
