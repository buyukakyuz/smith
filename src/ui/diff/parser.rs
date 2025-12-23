use similar::{ChangeTag, TextDiff};

use super::types::{ChangeType, DiffLine, LineTag};

#[derive(Debug, Clone)]
pub struct ParsedDiff {
    pub change_type: ChangeType,
    pub additions: usize,
    pub deletions: usize,
    pub lines: Vec<DiffLine>,
}

impl ParsedDiff {
    #[must_use]
    pub fn new(old_content: &str, new_content: &str) -> Self {
        let diff = TextDiff::from_lines(old_content, new_content);

        let mut additions = 0;
        let mut deletions = 0;
        let mut lines = Vec::new();
        let mut old_line_num = 1;
        let mut new_line_num = 1;

        for change in diff.iter_all_changes() {
            let (line_num, tag) = match change.tag() {
                ChangeTag::Delete => {
                    deletions += 1;
                    let num = old_line_num;
                    old_line_num += 1;
                    (num, LineTag::Removed)
                }
                ChangeTag::Insert => {
                    additions += 1;
                    let num = new_line_num;
                    new_line_num += 1;
                    (num, LineTag::Added)
                }
                ChangeTag::Equal => {
                    let num = new_line_num;
                    old_line_num += 1;
                    new_line_num += 1;
                    (num, LineTag::Unchanged)
                }
            };

            lines.push(DiffLine {
                line_num,
                tag,
                content: change.to_string(),
            });
        }

        let change_type = match (old_content.is_empty(), new_content.is_empty()) {
            (true, _) => ChangeType::Create,
            (_, true) => ChangeType::Delete,
            _ => ChangeType::Update,
        };

        Self {
            change_type,
            additions,
            deletions,
            lines,
        }
    }
}

const CONTEXT_LINES: usize = 3;

pub type Hunk<'a> = Vec<&'a DiffLine>;

#[must_use]
pub fn extract_hunks(lines: &[DiffLine]) -> Vec<Hunk<'_>> {
    let mut hunks = Vec::new();
    let mut current_hunk: Hunk<'_> = Vec::new();
    let mut last_change_idx: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.tag.is_change() {
            let start = last_change_idx.map_or_else(
                || idx.saturating_sub(CONTEXT_LINES),
                |last| idx.saturating_sub(CONTEXT_LINES).max(last + 1),
            );

            for i in start..idx {
                if !current_hunk.contains(&&lines[i]) {
                    current_hunk.push(&lines[i]);
                }
            }

            current_hunk.push(line);
            last_change_idx = Some(idx);
        } else if let Some(last_idx) = last_change_idx {
            let distance = idx - last_idx;

            if distance <= CONTEXT_LINES {
                current_hunk.push(line);
            } else if distance == CONTEXT_LINES + 1 {
                current_hunk.push(line);
                hunks.push(current_hunk);
                current_hunk = Vec::new();
                last_change_idx = None;
            }
        }
    }

    if !current_hunk.is_empty() {
        hunks.push(current_hunk);
    }

    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_creation() {
        let diff = ParsedDiff::new("", "fn main() {}\n");
        assert_eq!(diff.change_type, ChangeType::Create);
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);
    }

    #[test]
    fn parses_deletion() {
        let diff = ParsedDiff::new("fn old() {}\n", "");
        assert_eq!(diff.change_type, ChangeType::Delete);
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 1);
    }

    #[test]
    fn parses_update() {
        let diff = ParsedDiff::new("old line\n", "new line\n");
        assert_eq!(diff.change_type, ChangeType::Update);
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 1);
    }

    #[test]
    fn no_changes_when_identical() {
        let content = "same\n";
        let diff = ParsedDiff::new(content, content);
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 0);
    }

    #[test]
    fn extracts_hunks_with_context() {
        let lines: Vec<DiffLine> = (1..=21)
            .map(|i| DiffLine {
                line_num: i,
                tag: if i == 11 {
                    LineTag::Added
                } else {
                    LineTag::Unchanged
                },
                content: format!("line {i}\n"),
            })
            .collect();

        let hunks = extract_hunks(&lines);

        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].len(), 7);
    }
}
