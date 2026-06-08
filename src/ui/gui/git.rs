use super::{
    black,
    blue,
    button,
    gray,
    green,
    red,
    set_round_bg,
    white,
};
use super::file_change::render_udiff;
use super::worker::{
    Job,
    JobId,
    JobKind,
    JobResult,
    JobResultKind,
};
use crate::{Error, Hunk, subprocess, truncate_chars};
use iced::{Background, Element, Length, Size, Task};
use iced::alignment::Vertical;
use iced::border::{Border, Radius};
use iced::widget::{Column, Id, MouseArea, Row, Scrollable, Space, text};
use iced::widget::container::{Container, Style};
use regex::Regex;
use std::collections::HashSet;
use std::fmt;
use std::sync::LazyLock;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub path: String,
    pub window_size: Size,
    pub popup_scroll_id: Id,

    // This is a background worker that loads `git_info`.
    // When it's done, it'll update `git_info`.
    pub git_info_job_id: Option<JobId>,

    pub git_info: Option<GitInfo>,
    pub expanded_file_diffs: HashSet<String>,
    pub expanded_commits: HashSet<GitHash>,
    pub selected_tab: Tab,
    pub hovered_tab: Option<Tab>,
    pub error: Option<String>,
    pub zoom: f32,
}

impl IcedContext {
    pub fn new(
        path: &str,
        window_size: Size,
        popup_scroll_id: Id,
        init_job_id: JobId,
        zoom: f32,
    ) -> IcedContext {
        IcedContext {
            path: path.to_string(),
            window_size,
            popup_scroll_id,
            git_info_job_id: Some(init_job_id),
            git_info: None,
            expanded_file_diffs: HashSet::new(),
            expanded_commits: HashSet::new(),
            selected_tab: Tab::Changes,
            hovered_tab: None,
            error: None,
            zoom,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    Tick { frame: usize },
    HoverTab(Tab),
    UnhoverTab,
    SelectTab(Tab),
    ExpandFileDiff(String),
    ExpandCommit(GitHash),
    Stage(usize, usize),
    Unstage(usize, usize),
    Revert(usize, usize),
    BackgroundJob(Job),
    BackgroundJobResult(JobResult),
    Notify(String),
}

#[derive(Clone, Debug)]
pub struct GitInfo {
    pub is_git_repo: bool,
    pub staged: Vec<FileDiff>,
    pub unstaged: Vec<FileDiff>,
    pub recent_commits: Vec<CommitInfo>,
}

// First 64 bit of the hash.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GitHash(u64);

impl fmt::Display for GitHash {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{:016x}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct CommitInfo {
    pub commit_hash: GitHash,
    pub tree_hash: GitHash,

    // TODO: what if there are multiple?
    pub parent: Option<GitHash>,

    pub title: String,
    pub message: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub committer_name: String,
    pub committer_email: String,

    // vs parent
    pub diff: Option<Vec<FileDiff>>,
    pub add: usize,
    pub remove: usize,
}

#[derive(Clone, Debug)]
pub struct FileDiff {
    pub file_a: String,
    pub file_b: String,
    pub hunks: Vec<Hunk>,
    pub add: usize,
    pub remove: usize,
}

impl FileDiff {
    pub fn title(&self) -> String {
        match (self.file_a.as_str(), self.file_b.as_str()) {
            ("dev/null", f) | (f, "dev/null") => f.to_string(),
            (a, b) if a == b => a.to_string(),
            (a, b) => format!("{a} -> {b}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeKind {
    Staged,
    Unstaged,
    Committed(GitHash),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Changes,
    Commits,
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    match message {
        IcedMessage::Tick { frame } => {
            // Updates the info every 1.6 seconds.
            if context.git_info_job_id.is_none() && frame % 16 == 0 {
                let new_job_id = JobId::new();
                context.git_info_job_id = Some(new_job_id);
                return Ok(Task::done(IcedMessage::BackgroundJob(Job {
                    id: new_job_id,
                    kind: JobKind::GetGitInfo { path: context.path.to_string() },
                })));
            }
        },
        IcedMessage::HoverTab(t) => {
            context.hovered_tab = Some(t);
        },
        IcedMessage::UnhoverTab => {
            context.hovered_tab = None;
        },
        IcedMessage::SelectTab(t) => {
            context.selected_tab = t;
        },
        IcedMessage::ExpandFileDiff(d) => {
            if context.expanded_file_diffs.contains(&d) {
                context.expanded_file_diffs.remove(&d);
            } else {
                context.expanded_file_diffs.insert(d);
            }
        },
        IcedMessage::ExpandCommit(c) => {
            if context.expanded_commits.contains(&c) {
                context.expanded_commits.remove(&c);
            } else {
                context.expanded_commits.insert(c);
            }
        },
        IcedMessage::Stage(i, j) => {
            return Ok(Task::done(IcedMessage::Notify(String::from("Not implemented: Stage changes"))));
        },
        IcedMessage::Unstage(i, j) => {
            return Ok(Task::done(IcedMessage::Notify(String::from("Not implemented: Unstage changes"))));
        },
        IcedMessage::Revert(i, j) => {
            return Ok(Task::done(IcedMessage::Notify(String::from("Not implemented: Revert changes"))));
        },
        IcedMessage::BackgroundJob(_) => unreachable!(),
        IcedMessage::BackgroundJobResult(job_result) => match &job_result.kind {
            JobResultKind::GetGitInfo(i) if context.git_info_job_id == job_result.id => {
                let first_load = context.git_info.is_none();
                context.git_info = Some(i.clone());
                context.git_info_job_id = None;

                if first_load {
                    context.expanded_file_diffs.clear();

                    for (change_kind, file_diff) in i.staged.iter().map(|d| (ChangeKind::Staged, d)).chain(i.unstaged.iter().map(|d| (ChangeKind::Unstaged, d))) {
                        if file_diff.add + file_diff.remove < 30 {
                            let expand_key = format!("{change_kind:?}-{}", file_diff.title());
                            context.expanded_file_diffs.insert(expand_key);
                        }
                    }
                }

                if !i.is_git_repo {
                    context.error = Some(String::from("Not a git repository"));
                }
            },
            JobResultKind::GetGitInfoError(e) => {
                context.error = Some(e.to_string());
            },
            _ => {},
        },
        IcedMessage::Notify(_) => unreachable!(),
    }

    Ok(Task::none())
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    let body = if let Some(git_info) = &context.git_info {
        let body = match context.selected_tab {
            Tab::Changes => render_changes(git_info, context),
            Tab::Commits => render_commits(git_info, context),
        };

        Scrollable::new(body)
            .id(context.popup_scroll_id.clone())
            .into()
    } else {
        text!("loading...").size(context.zoom * 14.0).into()
    };

    Column::from_vec(vec![
        render_tab_buttons(context),
        body,
        if let Some(error) = &context.error {
            text!("{error}").color(red()).size(context.zoom * 18.0).into()
        } else {
            Space::new().into()
        },
    ]).into()
}

fn render_tab_buttons<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    fn render_tab_button<'m>(
        name: &'static str,
        on_enter: IcedMessage,
        on_exit: IcedMessage,
        on_press: IcedMessage,
        is_selected: bool,
        is_hovered: bool,
        zoom: f32,
    ) -> Element<'m, IcedMessage> {
        let background = match (is_selected, is_hovered) {
            (true, true) => gray(0.6),
            (true, false) => gray(0.8),
            (false, true) => gray(0.4),
            (false, false) => gray(0.2),
        };

        MouseArea::new(
            Container::new(
                text!("{name}")
                    .color(if is_selected { black() } else { white() })
                    .size(zoom * 14.0)
            )
                .center_x(Length::FillPortion(1))
                .padding(zoom * 8.0)
                .style(move |_| Style {
                    background: Some(Background::Color(background)),
                    border: Border {
                        color: white(),
                        width: 0.0,
                        radius: Radius::new(999.0),
                    },
                    ..Style::default()
                })
        )
            .on_enter(on_enter)
            .on_exit(on_exit)
            .on_press(on_press)
            .into()
    }

    Row::from_vec(vec![
        render_tab_button(
            "Changes",
            IcedMessage::HoverTab(Tab::Changes),
            IcedMessage::UnhoverTab,
            IcedMessage::SelectTab(Tab::Changes),
            context.selected_tab == Tab::Changes,
            context.hovered_tab == Some(Tab::Changes),
            context.zoom,
        ),
        render_tab_button(
            "Commits",
            IcedMessage::HoverTab(Tab::Commits),
            IcedMessage::UnhoverTab,
            IcedMessage::SelectTab(Tab::Commits),
            context.selected_tab == Tab::Commits,
            context.hovered_tab == Some(Tab::Commits),
            context.zoom,
        ),
    ])
        .width(context.window_size.width)
        .spacing(context.zoom * 8.0)
        .into()
}

fn render_changes<'c>(git_info: &'c GitInfo, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec(vec![
        render_changes_unit(
            Some("Staged"),
            "No staged changes",
            ChangeKind::Staged,
            &git_info.staged,
            &context.expanded_file_diffs,
            context.window_size,
            context.zoom,
        ),
        render_changes_unit(
            Some("Unstaged"),
            "No unstaged changes",
            ChangeKind::Unstaged,
            &git_info.unstaged,
            &context.expanded_file_diffs,
            context.window_size,
            context.zoom,
        ),
    ])
        .padding(context.zoom * 8.0)
        .spacing(context.zoom * 8.0)
        .into()
}

fn render_commits<'c>(git_info: &'c GitInfo, context: &'c IcedContext) -> Element<'c, IcedMessage> {
    Column::from_vec(
        git_info.recent_commits.iter().map(
            |commit| {
                let is_expanded = context.expanded_commits.contains(&commit.commit_hash);
                let title = Row::from_vec(vec![
                    if is_expanded {
                        button("▼", IcedMessage::ExpandCommit(commit.commit_hash), white(), context.zoom).into()
                    } else {
                        button("▶", IcedMessage::ExpandCommit(commit.commit_hash), white(), context.zoom).into()
                    },
                    Column::from_vec(vec![
                        Row::from_vec(vec![
                            text!("{} (", truncate_chars(&commit.title, 88)).size(context.zoom * 14.0).into(),
                            text!("+{}", commit.add).color(green()).size(context.zoom * 14.0).into(),
                            text!(", ").size(context.zoom * 14.0).into(),
                            text!("-{}", commit.remove).color(red()).size(context.zoom * 14.0).into(),
                            text!(")").size(context.zoom * 14.0).into(),
                        ]).into(),

                        // TODO: display timestamp
                        text!("{} <{}>", commit.author_name, commit.author_email).size(context.zoom * 14.0).into(),
                    ]).into(),
                ])
                    .width(context.window_size.width)
                    .spacing(context.zoom * 4.0)
                    .align_y(Vertical::Center);
                let mut column: Vec<Element<IcedMessage>> = vec![title.into()];

                if is_expanded && let Some(diff) = &commit.diff {
                    column.push(render_changes_unit(
                        None,
                        "No changes in this commit",
                        ChangeKind::Committed(commit.commit_hash),
                        diff,
                        &context.expanded_file_diffs,
                        context.window_size,
                        context.zoom,
                    ));
                }

                Container::new(
                    Column::from_vec(column)
                        .spacing(context.zoom * 8.0)
                )
                    .padding(context.zoom * 8.0)
                    .style(|_| set_round_bg(gray(0.2), context.zoom))
                    .into()
            }
        ).collect()
    )
        .padding(context.zoom * 8.0)
        .spacing(context.zoom * 8.0)
        .into()
}

fn render_changes_unit<'d, 'e>(
    title: Option<&'static str>,
    empty: &'static str,
    change_kind: ChangeKind,
    changes: &'d [FileDiff],
    expanded_file_diffs: &'e HashSet<String>,
    window_size: Size,
    zoom: f32,
) -> Element<'d, IcedMessage> {
    let mut column: Vec<Element<IcedMessage>> = vec![];

    if let Some(title) = title {
        column.push(text!("{title}").size(zoom * 18.0).into());
    }

    for (i, change) in changes.iter().enumerate() {
        let title = change.title();
        let expand_key = format!("{change_kind:?}-{title}");
        let is_expanded = expanded_file_diffs.contains(&expand_key);
        let title = Row::from_vec(vec![
            if is_expanded {
                button("▼", IcedMessage::ExpandFileDiff(expand_key), white(), zoom).into()
            } else {
                button("▶", IcedMessage::ExpandFileDiff(expand_key), white(), zoom).into()
            },
            Space::new().width(zoom * 8.0).into(),
            text!("{title} (").size(zoom * 14.0).into(),
            text!("+{}", change.add).color(green()).size(zoom * 14.0).into(),
            text!(", ").size(zoom * 14.0).into(),
            text!("-{}", change.remove).color(red()).size(zoom * 14.0).into(),
            text!(")").size(zoom * 14.0).into(),
        ]);
        let hunks: Vec<Element<IcedMessage>> = if is_expanded {
            change.hunks.iter().enumerate().map(
                move |(j, hunk)| {
                    let buttons: Vec<Element<IcedMessage>> = match change_kind {
                        ChangeKind::Unstaged => vec![
                            button("Revert", IcedMessage::Revert(i, j), red(), zoom).into(),
                            button("Stage", IcedMessage::Stage(i, j), green(), zoom).into(),
                        ],
                        ChangeKind::Staged => vec![
                            button("Unstage", IcedMessage::Unstage(i, j), blue(), zoom).into(),
                        ],
                        ChangeKind::Committed(_) => vec![],
                    };

                    Container::new(
                        Column::from_vec(vec![
                            Row::from_vec(buttons)
                                .spacing(zoom * 8.0)
                                .into(),
                            render_udiff(
                                &hunk.to_udiff(),
                                window_size.width,
                                zoom,
                            ),
                        ])
                            .spacing(zoom * 4.0)
                    )
                        .padding(zoom * 4.0)
                        .style(move |_| set_round_bg(black(), zoom))
                        .into()
                }
            ).collect()
        } else {
            vec![]
        };

        column.push(Container::new(Column::from_vec(vec![
            title.into(),
            Column::from_vec(hunks).spacing(zoom * 8.0).into(),
        ])
            .spacing(zoom * 8.0))
            .padding(zoom * 8.0)
            .width(window_size.width)
            .style(move |_| set_round_bg(gray(0.2), zoom))
            .into()
        );
    }

    if changes.is_empty() {
        column.push(text!("{empty}").size(zoom * 14.0).into());
    }

    Column::from_vec(column).spacing(zoom * 8.0).into()
}

static LOG_LINE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([0-9a-f]{16}).+").unwrap());

pub fn get_git_info(path: &str) -> Result<GitInfo, Error> {
    let is_git_repo = subprocess::run(
        String::from("git"),
        &[String::from("status")],
        false,
        &[],
        path,
        3,
        "",
        false,
    )?;
    let is_git_repo = !String::from_utf8_lossy(&is_git_repo.stderr).contains("fatal: not a git repository");

    let unstaged_changes = if is_git_repo {
        let unstaged_changes = subprocess::run(
            String::from("git"),
            &[
                String::from("diff"),
                String::from("-U5"),
                String::from("--diff-algorithm=patience"),
                String::from("--no-color"),
            ],
            false,
            &[],
            path,
            3,
            "",
            false,
        )?;
        parse_git_diff(&String::from_utf8_lossy(&unstaged_changes.stdout))
    } else {
        vec![]
    };

    let staged_changes = if is_git_repo {
        let staged_changes = subprocess::run(
            String::from("git"),
            &[
                String::from("diff"),
                String::from("--cached"),
                String::from("-U5"),
                String::from("--diff-algorithm=patience"),
                String::from("--no-color"),
            ],
            false,
            &[],
            path,
            3,
            "",
            false,
        )?;
        parse_git_diff(&String::from_utf8_lossy(&staged_changes.stdout))
    } else {
        vec![]
    };

    let recent_commits = if is_git_repo {
        let commit_hashes = subprocess::run(
            String::from("git"),
            &[
                String::from("log"),
                String::from("--oneline"),
                String::from("--abbrev=16"),
                String::from("-n100"),
                String::from("--no-color"),
            ],
            false,
            &[],
            path,
            3,
            "",
            false,
        )?;
        let commit_hashes: Vec<GitHash> = String::from_utf8_lossy(&commit_hashes.stdout).lines().filter_map(
            |line| match LOG_LINE_RE.captures(line) {
                Some(cap) => Some(GitHash(u64::from_str_radix(cap.get(1).unwrap().as_str(), 16).unwrap())),
                None => None,
            }
        ).collect();
        let mut recent_commits = vec![];
        let started_at = Instant::now();

        for commit in commit_hashes.into_iter() {
            recent_commits.push(load_commit_info(path, commit)?);

            // We're not gonna spend more than 10 seconds here.
            if Instant::now().duration_since(started_at.clone()).as_millis() > 10_000 {
                break;
            }
        }

        recent_commits
    } else {
        vec![]
    };

    Ok(GitInfo {
        is_git_repo,
        staged: staged_changes,
        unstaged: unstaged_changes,
        recent_commits,
    })
}

static LINE_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"diff --git a/(.+) b/(.+)").unwrap());

fn parse_git_diff(diff: &str) -> Vec<FileDiff> {
    fn parse_line_header(header: &str) -> (String, String) {
        match LINE_HEADER_RE.captures(header) {
            Some(cap) => (cap.get(1).unwrap().as_str().to_string(), cap.get(2).unwrap().as_str().to_string()),
            None => unreachable!(),
        }
    }

    // 1. If I use `String::from_utf8(_)?`,
    //    - The user can't see the diffs if the files are broken.
    // 2. If I use `String::from_utf8_lossy()`,
    //    - The user can see the diffs even if the files are broken.
    //    - The user cannot stage/revert a broken file, but the program
    //      will not crash and it won't break git.
    let mut files = vec![];
    let mut curr_file = (String::new(), String::new());  // (a, b)
    let mut hunks = vec![];
    let mut curr_hunk = vec![];
    let mut ignore_until_at = false;

    for line in diff.lines() {
        if line.starts_with("@") || line.starts_with("diff") {
            if !curr_hunk.is_empty() {
                hunks.push(Hunk::from_lines(&curr_hunk));
                curr_hunk = vec![];
            }

            if line.starts_with("diff") {
                ignore_until_at = true;

                if !hunks.is_empty() {
                    let add = hunks.iter().map(|h| h.add).sum::<usize>();
                    let remove = hunks.iter().map(|h| h.remove).sum::<usize>();
                    files.push(FileDiff {
                        file_a: curr_file.0.to_string(),
                        file_b: curr_file.1.to_string(),
                        hunks,
                        add,
                        remove,
                    });
                }

                curr_file = parse_line_header(line);
                hunks = vec![];
            }

            else {
                ignore_until_at = false;
            }
        }

        else if ignore_until_at {
            //
        }

        else {
            curr_hunk.push(line);
        }
    }

    if !curr_hunk.is_empty() {
        hunks.push(Hunk::from_lines(&curr_hunk));
    }

    if !hunks.is_empty() {
        let add = hunks.iter().map(|h| h.add).sum::<usize>();
        let remove = hunks.iter().map(|h| h.remove).sum::<usize>();
        files.push(FileDiff {
            file_a: curr_file.0.to_string(),
            file_b: curr_file.1.to_string(),
            hunks,
            add,
            remove,
        });
    }

    files
}

// TODO: cache `load_commit_info`.

// If a commit has multiple parents, it only takes the first one and ignores the rest.
static COMMIT_INFO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)tree ([0-9a-f]{16})[0-9a-f]+\n(?:parent ([0-9a-f]{16})[0-9a-f]+\n)?(?:parent [0-9a-f]+\n)*author (.+) <(.+)> (\d+) (.+)\ncommitter (.+) <(.+)> (\d+) (.+)\n([^\n]+)(?:\n(.+))?").unwrap());

fn load_commit_info(path: &str, commit: GitHash) -> Result<CommitInfo, Error> {
    let commit_info = subprocess::run(
        String::from("git"),
        &[
            String::from("cat-file"),
            String::from("commit"),
            format!("{commit}"),
        ],
        false,
        &[],
        path,
        3,
        "",
        false,
    )?;

    let mut commit_info = match COMMIT_INFO_RE.captures(&String::from_utf8_lossy(&commit_info.stdout)) {
        Some(cap) => CommitInfo {
            commit_hash: commit,
            tree_hash: GitHash(u64::from_str_radix(cap.get(1).unwrap().as_str(), 16).unwrap()),
            parent: cap.get(2).map(|cap| GitHash(u64::from_str_radix(cap.as_str(), 16).unwrap())),
            title: cap.get(11).unwrap().as_str().to_string(),
            message: cap.get(12).map(|cap| cap.as_str().to_string()),
            author_name: cap.get(3).unwrap().as_str().to_string(),
            author_email: cap.get(4).unwrap().as_str().to_string(),
            committer_name: cap.get(7).unwrap().as_str().to_string(),
            committer_email: cap.get(8).unwrap().as_str().to_string(),
            diff: None,
            add: 0,
            remove: 0,
        },
        None => panic!("TODO: {}", String::from_utf8_lossy(&commit_info.stdout)),
    };

    if let Some(parent) = commit_info.parent {
        let diff = subprocess::run(
            String::from("git"),
            &[
                String::from("diff"),
                format!("{parent}"),
                format!("{commit}"),
                String::from("-U5"),
                String::from("--diff-algorithm=patience"),
                String::from("--no-color"),
            ],
            false,
            &[],
            path,
            3,
            "",
            false,
        )?;
        let diff = parse_git_diff(&String::from_utf8_lossy(&diff.stdout));
        commit_info.add = diff.iter().map(|diff| diff.add).sum();
        commit_info.remove = diff.iter().map(|diff| diff.remove).sum();
        commit_info.diff = Some(diff);
    }

    Ok(commit_info)
}
