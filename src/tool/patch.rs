use super::{ToolCallError, ToolCallSuccess};
use crate::ParseError;
use ragit_fs::read_string;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LineDiff {
    pub kind: DiffKind,
    pub line: String,
}

impl fmt::Display for LineDiff {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}{}", self.kind, self.line)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DiffKind {
    Context,
    Add,
    Remove,
}

impl fmt::Display for DiffKind {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            DiffKind::Context => write!(fmt, " "),
            DiffKind::Add => write!(fmt, "+"),
            DiffKind::Remove => write!(fmt, "-"),
        }
    }
}

impl DiffKind {
    pub fn revert(self) -> DiffKind {
        match self {
            DiffKind::Context => DiffKind::Context,
            DiffKind::Add => DiffKind::Remove,
            DiffKind::Remove => DiffKind::Add,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PatchError {
    NoMatch { missing_context_marker: Option<String> },
    MultipleMatch,

    // only context lines
    NoUpdate,
}

pub fn patch_file(path: &str, diff: &[LineDiff]) -> Result<ToolCallSuccess, ToolCallError> {
    // IO errors must be checked before calling this function!
    let old_content = read_string(path).unwrap();

    match patch_diff(&old_content, diff) {
        Ok(content) => Ok(ToolCallSuccess::Patch {
            path: path.to_string(),
            diff: diff.to_vec(),
            new_content: content,
        }),
        Err(e) => Err(ToolCallError::CannotApplyPatch(e)),
    }
}

pub fn patch_diff(content: &str, diff: &[LineDiff]) -> Result<String, PatchError> {
    // strategy: find `context_before` in `context` and replace that with `context_after`
    let old_content_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let old_content_trailing_newline = content.ends_with("\n");  // TODO: it cannot handle if it ends with multiple newlines
    let context_before: Vec<String> = diff.iter().filter(
        |LineDiff { kind, .. }| *kind == DiffKind::Context || *kind == DiffKind::Remove
    ).map(
        |LineDiff { line, .. }| line.to_string()
    ).collect();

    let context_after: Vec<String> = diff.iter().filter(
        |LineDiff { kind, .. }| *kind == DiffKind::Context || *kind == DiffKind::Add
    ).map(
        |LineDiff { line, .. }| line.to_string()
    ).collect();

    if context_before == context_after {
        return Err(PatchError::NoUpdate);
    }

    if context_before.is_empty() && content.is_empty() {
        return Ok(context_after.join("\n"));
    }

    if context_before.is_empty() || old_content_lines.len() < context_before.len() {
        return Err(PatchError::NoMatch { missing_context_marker: check_missing_context_marker(content, diff) });
    }

    let mut matches = vec![];

    for i in 0..(old_content_lines.len() - context_before.len() + 1) {
        if &old_content_lines[i..(i + context_before.len())] == &context_before {
            matches.push(i);
        }
    }

    match matches.len() {
        0 => Err(PatchError::NoMatch { missing_context_marker: check_missing_context_marker(content, diff) }),
        1 => {
            let match_at = matches[0];
            let new_content_lines = [
                old_content_lines[..match_at].to_vec(),
                context_after,
                old_content_lines[(match_at + context_before.len())..].to_vec(),
            ].concat();
            Ok(format!(
                "{}{}",
                new_content_lines.join("\n"),
                if old_content_trailing_newline { "\n" } else { "" },
            ))
        },
        _ => Err(PatchError::MultipleMatch),
    }
}

pub fn parse_line_diff(lines: &str) -> Result<Vec<LineDiff>, ParseError> {
    let mut diff = vec![];

    for line in lines.lines() {
        if line.is_empty() {
            continue;
        }

        if line.starts_with(" ") {
            diff.push(LineDiff {
                kind: DiffKind::Context,
                line: line.get(1..).map(|s| s.to_string()).unwrap_or(String::new()),
            });
        } else if line.starts_with("+") {
            diff.push(LineDiff {
                kind: DiffKind::Add,
                line: line.get(1..).map(|s| s.to_string()).unwrap_or(String::new()),
            });
        } else if line.starts_with("-") {
            diff.push(LineDiff {
                kind: DiffKind::Remove,
                line: line.get(1..).map(|s| s.to_string()).unwrap_or(String::new()),
            });
        } else {
            return Err(ParseError::InvalidPatchPrefix {
                line: line.to_string(),
                prefix: line.chars().next().unwrap(),
            });
        }
    }

    Ok(diff)
}

pub fn revert_hunks(udiff: &str) -> Vec<Vec<LineDiff>> {
    let mut hunks = vec![];
    let mut curr_hunk = vec![];

    for line in udiff.lines() {
        if line.starts_with("+") {
            // revert the diff
            curr_hunk.push(LineDiff { kind: DiffKind::Remove, line: line.get(1..).unwrap().to_string() });
        } else if line.starts_with("-") {
            // revert the diff
            curr_hunk.push(LineDiff { kind: DiffKind::Add, line: line.get(1..).unwrap().to_string() });
        } else if line.starts_with(" ") {
            curr_hunk.push(LineDiff { kind: DiffKind::Context, line: line.get(1..).unwrap().to_string() });
        } else if line.starts_with("@") {
            if !curr_hunk.is_empty() {
                hunks.push(curr_hunk);
            }

            curr_hunk = vec![];
        } else {
            panic!("TODO: {line:?}");
        }
    }

    if !curr_hunk.is_empty() {
        hunks.push(curr_hunk);
    }

    hunks
}

/*
```
pub struct Person {
    pub name: String,
    pub age: u16,
}
```

->

```
     pub name: String,
+    pub is_rustacean: bool,
     pub age: u16,
```

-> The context lines must have 5 spaces (one for context-marker and 4 for indentation), but many
chinese models output only 4 spaces. So it checks such mistakes and generates a warning.
*/
fn check_missing_context_marker(content: &str, diff: &[LineDiff]) -> Option<String> {
    let lines: HashSet<String> = content.lines().map(|s| s.to_string()).collect();
    let context_lines: Vec<String> = diff.iter().filter_map(
        |LineDiff { kind, line }| match kind {
            DiffKind::Context if line.starts_with(" ") => Some(line.to_string()),
            _ => None,
        }
    ).collect();

    for context_line in context_lines.iter() {
        if !lines.contains(context_line) && lines.contains(&format!(" {context_line}")) {
            return Some(format!(" {context_line}"));
        }
    }

    None
}

// VIBE NOTE: gpt-5.5 (via neukgu) wrote some of these tests.
#[cfg(test)]
mod tests {
    use super::{PatchError, parse_line_diff, patch_diff};

    #[test]
    fn replaces_single_line_with_context() {
        let content = "a\nb\nc\n";
        let patch = parse_line_diff(" a\n-b\n+B\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nB\nc\n".to_string()));
    }

    #[test]
    fn adds_line_between_context_lines() {
        let content = "a\nc\n";
        let patch = parse_line_diff(" a\n+b\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nb\nc\n".to_string()));
    }

    #[test]
    fn removes_line_between_context_lines() {
        let content = "a\nb\nc\n";
        let patch = parse_line_diff(" a\n-b\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nc\n".to_string()));
    }

    #[test]
    fn patches_without_trailing_newline() {
        let content = "a\nb\nc";
        let patch = parse_line_diff(" a\n-b\n+B\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nB\nc".to_string()));
    }

    #[test]
    fn can_patch_at_start() {
        let content = "a\nb\nc\n";
        let patch = parse_line_diff("-a\n+A\n b\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("A\nb\nc\n".to_string()));
    }

    #[test]
    fn can_patch_at_end() {
        let content = "a\nb\nc\n";
        let patch = parse_line_diff(" b\n-c\n+C\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nb\nC\n".to_string()));
    }

    #[test]
    fn can_patch_without_context1() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("-b\n+B").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nB\nc".to_string()));
    }

    #[test]
    fn can_patch_without_context2() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("-b\n-c").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a".to_string()));
    }

    #[test]
    fn returns_no_match_when_context_does_not_match() {
        let content = "a\nb\nc\n";
        let patch = parse_line_diff(" x\n-b\n+B\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Err(PatchError::NoMatch));
    }

    #[test]
    fn returns_multiple_match_for_ambiguous_patch() {
        let content = "a\nb\na\nb\n";
        let patch = parse_line_diff(" a\n-b\n+B\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Err(PatchError::MultipleMatch));
    }

    #[test]
    fn add_only_patch_is_ambiguous_for_non_empty_content() {
        let content = "a\nb\n";
        let patch = parse_line_diff("+x\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Err(PatchError::NoMatch));
    }

    #[test]
    fn add_only_patch_applies_to_empty_content1() {
        let content = "";
        let patch = parse_line_diff("+x\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("x".to_string()));
    }

    #[test]
    fn add_only_patch_applies_to_empty_content2() {
        let content = "";
        let patch = parse_line_diff("+x\n+y").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("x\ny".to_string()));
    }

    #[test]
    fn add_only_patch_applies_to_empty_content3() {
        let content = "\n";  // this is not an empty content!
        let patch = parse_line_diff("+x\n+y").unwrap();

        assert_eq!(patch_diff(content, &patch), Err(PatchError::NoMatch));
    }

    #[test]
    fn remove_only_patch1() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("-a\n-b\n-c").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("".to_string()));
    }

    #[test]
    fn remove_only_patch2() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("-b\n-c").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a".to_string()));
    }

    #[test]
    fn context_after_add1() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("+0\n-b").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\n0\nc".to_string()));
    }

    #[test]
    fn context_after_add2() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("+0\n b").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\n0\nb\nc".to_string()));
    }

    #[test]
    fn context_after_add3() {
        let content = "a\nb\nc";
        let patch = parse_line_diff("+0\n b\n+1").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\n0\nb\n1\nc".to_string()));
    }

    #[test]
    fn exact_context_only_patch_is_noop() {
        let content = "a\nb\nc\n";
        let patch = parse_line_diff(" b\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nb\nc\n".to_string()));
    }

    #[test]
    fn context_only_patch_can_match_whole_file_without_trailing_newline() {
        let content = "a\nb\nc";
        let patch = parse_line_diff(" a\n b\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nb\nc".to_string()));
    }

    #[test]
    fn context_only_patch_is_ambiguous_when_context_repeats() {
        let content = "a\nb\na\nb\n";
        let patch = parse_line_diff(" a\n b\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Err(PatchError::MultipleMatch));
    }

    #[test]
    fn preserves_trailing_newline_when_removing_all_lines() {
        let content = "a\nb\n";
        let patch = parse_line_diff("-a\n-b\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("\n".to_string()));
    }

    #[test]
    fn can_replace_empty_line() {
        let content = "a\n\nc\n";
        let patch = parse_line_diff(" a\n-\n+B\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\nB\nc\n".to_string()));
    }

    #[test]
    fn can_add_empty_line() {
        let content = "a\nc\n";
        let patch = parse_line_diff(" a\n+\n c\n").unwrap();

        assert_eq!(patch_diff(content, &patch), Ok("a\n\nc\n".to_string()));
    }
}
