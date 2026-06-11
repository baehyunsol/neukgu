use ragit_fs::{basename, join};
use serde::{Deserialize, Serialize};
use std::fmt;

// Both `relative` and `absolute` are normalized.
// If the path is inside working-dir, it the `relative` field exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Path {
    pub relative: Option<String>,
    pub absolute: String,
}

impl Path {
    // `logs/summary*.md` or `logs/done`
    pub fn is_summary_file(&self) -> bool {
        self.is_done_file() || match self {
            Path { relative: Some(r), .. } if r.starts_with("logs/") => match basename(r) {
                Ok(base) => base.starts_with("summary") && base.ends_with(".md"),
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_done_file(&self) -> bool {
        match self {
            Path { relative: Some(r), .. } if r == "logs/done" => true,
            _ => false,
        }
    }

    pub fn is_index_dir(&self) -> bool {
        match self {
            Path { relative: Some(r), .. } if r.starts_with(".neukgu/") || r == ".neukgu" => true,
            _ => false,
        }
    }

    pub fn is_skills_dir(&self) -> bool {
        match self {
            Path { relative: Some(r), .. } if r.starts_with(".neukgu/skills/") || r == ".neukgu/skills" => true,
            _ => false,
        }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let Some(relative) = &self.relative {
            write!(fmt, "{relative}")
        } else {
            write!(fmt, "{}", self.absolute)
        }
    }
}

// TODO: I don't think it runs on Windows...
// If it's invalid, it returns None. (e.g. `normalize_path("/../../", ..)`)
// If it's valid but not in working dir, it returns Some(Path { relative: None, .. }). (e.g. `normalize_path("../a/b/", ..)`)
pub fn normalize_path(path: &str, working_dir: &str) -> Option<Path> {
    fn normalize_path_lexically(path: &str) -> Option<String> {
        let mut result = vec![];

        for segment in path.split("/") {
            match segment {
                "." => {},
                ".." => match result.pop() {
                    Some(s) => {
                        if s == "" {
                            return None;
                        }
                    },
                    None => {
                        return None;
                    },
                },
                s => {
                    result.push(s.to_string());
                },
            }
        }

        if let Some(last) = result.last() && last == "" {
            result.pop().unwrap();
        }

        Some(result.join("/"))
    }

    if path == "" {
        None
    }

    else if path.starts_with("/") {
        let mut absolute = normalize_path_lexically(path)?;

        if absolute == "" {
            absolute = String::from("/")
        }

        Some(Path { relative: None, absolute })
    }

    else {
        let absolute = match join(working_dir, path) {
            Ok(path) => path,
            Err(_) => return None,
        };
        let absolute = normalize_path_lexically(&absolute)?;
        let mut relative = normalize_path_lexically(path);

        if let Some(r) = &relative && r == "" {
            relative = Some(String::from("."));
        }

        Some(Path { relative, absolute })
    }
}

#[cfg(test)]
mod tests {
    use super::{Path, normalize_path};

    #[test]
    fn normalize_path_test() {
        let mut failures = vec![];

        for (sample, answer) in [
            ("/c/d", Some((None, "/c/d"))),
            ("c/d", Some((Some("c/d"), "/a/b/c/d"))),
            ("c/d/", Some((Some("c/d"), "/a/b/c/d"))),
            ("./c/d", Some((Some("c/d"), "/a/b/c/d"))),
            ("../c/d", Some((None, "/a/c/d"))),
            ("./../c/d", Some((None, "/a/c/d"))),
            ("/../c/d", None),
            ("c/../d", Some((Some("d"), "/a/b/d"))),
            ("c/../../d", Some((None, "/a/d"))),
            ("/c/../d", Some((None, "/d"))),
            ("/c/../../d", None),
            ("/", Some((None, "/"))),
            ("/./", Some((None, "/"))),
            (".", Some((Some("."), "/a/b"))),
            ("./././", Some((Some("."), "/a/b"))),
            ("././.", Some((Some("."), "/a/b"))),
            ("", None),
        ] {
            let result = normalize_path(sample, "/a/b");

            match (&result, &answer) {
                (None, None) => {},
                (None, Some(_)) | (Some(_), None) => {
                    failures.push(format!("input: {sample:?}, result: {result:?}, answer: {answer:?}"));
                },
                (Some(Path { relative, absolute }), Some((res_rel, res_abs))) => {
                    let i_hate_this_kinda_code = match (relative, res_rel) {
                        (Some(_), None) | (None, Some(_)) => false,
                        (Some(r1), Some(r2)) => r1 == r2,
                        (None, None) => true,
                    };

                    if !i_hate_this_kinda_code || absolute != res_abs {
                        failures.push(format!("input: {sample:?}, result: {result:?}, answer: {answer:?}"));
                    }
                },
            }
        }

        if !failures.is_empty() {
            panic!("{}", failures.join("\n------\n"));
        }
    }
}
