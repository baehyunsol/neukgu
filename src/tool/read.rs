use super::{Path, ToolCallError, ToolCallSuccess, normalize_path};
use crate::{
    Config,
    Context,
    Error,
    ImageId,
    PdfId,
    TurnResult,
    TurnResultSummary,
    normalize_and_get_id,
    render_and_get_id,
};
use hayro::hayro_syntax::Pdf;
use ragit_fs::{
    FileError,
    basename,
    exists,
    extension,
    is_dir,
    is_symlink,
    join,
    read_bytes,
    read_dir,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum RangeType {
    Line,
    FileEntry,
    PdfPage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TypedFile {
    Text(String),
    Image(ImageId, (u64, u64)),
    BrokenImage { error: String },
    Dir(Vec<FileEntry>),
    Symlink { pointee: String },
    Pdf(PdfId),
    BrokenPdf { error: String },
    Etc,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FileEntry {
    TextFile {
        name: String,
        bytes: u64,
        chars: u64,
        lines: u64,
    },
    PdfFile {
        name: String,
        pages: u64,
    },
    BrokenPdf {
        name: String,
        bytes: u64,
    },
    ImageFile {
        name: String,
        size: (u32, u32),
    },
    BrokenImage {
        name: String,
        bytes: u64,
    },
    EtcFile {
        name: String,
        bytes: u64,
    },
    Dir {
        name: String,
    },
    Symlink {
        name: String,
    },
}

// NOTE: The pdf crate can parse non-pdf files. For example, it parses
// psd/psb (photoshop file format) files without any errors. So, we have
// to classify the files with their extensions.
// It already checked that the path exists.
pub fn read_file(path: &str, context: &Context) -> Result<TypedFile, Error> {
    let real_path = join(&context.working_dir, path)?;

    if is_dir(&real_path) {
        let mut entries = vec![];
        let mut basenames = vec![];

        for e in read_dir(&real_path, true)? {
            let name = basename(&e)?;
            let ext = extension(&e)?.unwrap_or(String::new()).to_ascii_lowercase();

            // hidden
            if name == ".neukgu" {
                continue;
            }

            basenames.push(name.clone());

            if is_dir(&e) {
                entries.push(FileEntry::Dir { name });
            }

            else if is_symlink(&e) {
                entries.push(FileEntry::Symlink { name });
            }

            else {
                let bytes = read_bytes(&e)?;

                let entry = match ext.as_str() {
                    "pdf" => match Pdf::new(bytes.clone()) {
                        Ok(pdf) => FileEntry::PdfFile { name, pages: pdf.pages().len() as u64 },
                        Err(_) => FileEntry::BrokenPdf { name, bytes: bytes.len() as u64 },
                    },
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "tiff" | "bmp" => match image::load_from_memory(&bytes) {
                        Ok(buffer) => FileEntry::ImageFile { name, size: (buffer.width(), buffer.height()) },
                        Err(_) => FileEntry::BrokenImage { name, bytes: bytes.len() as u64 },
                    },
                    _ => match String::from_utf8(bytes.clone()) {
                        Ok(s) => {
                            let chars = s.chars().count() as u64;
                            let lines = s.lines().count() as u64;
                            let bytes = bytes.len() as u64;
                            FileEntry::TextFile { name, bytes, chars, lines }
                        },
                        Err(_) => FileEntry::EtcFile { name, bytes: bytes.len() as u64 },
                    },
                };

                entries.push(entry);
            }
        }

        // Let's inform AI that these bins are available!
        if path == "bins" {
            for bin in context.available_binaries.iter() {
                if !basenames.contains(bin) {
                    entries.push(FileEntry::EtcFile { name: bin.to_string(), bytes: 0 });
                }
            }
        }

        Ok(TypedFile::Dir(entries))
    }

    else if is_symlink(&real_path) {
        let pointee = std::fs::read_link(&real_path).map_err(|err| FileError::from_std(err, &real_path))?;
        let pointee = pointee.into_os_string().into_string().unwrap();
        Ok(TypedFile::Symlink { pointee })
    }

    else {
        let bytes = read_bytes(&real_path)?;
        let ext = extension(&real_path)?.unwrap_or(String::new()).to_ascii_lowercase();

        match ext.as_str() {
            "pdf" => match render_and_get_id(&bytes, &context.working_dir) {
                Ok(id) => Ok(TypedFile::Pdf(id)),
                Err(Error::UserInterrupt) => Err(Error::UserInterrupt),
                Err(e) => Ok(TypedFile::BrokenPdf { error: format!("{e:?}") }),
            },
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "tiff" | "bmp" => match normalize_and_get_id(&bytes, &context.working_dir) {
                Ok(id) => {
                    let image_buffer = image::load_from_memory(&bytes)?;
                    Ok(TypedFile::Image(id, (image_buffer.width() as u64, image_buffer.height() as u64)))
                },
                Err(e) => Ok(TypedFile::BrokenImage { error: format!("{e:?}") }),
            },
            _ => match String::from_utf8(bytes) {
                Ok(s) => Ok(TypedFile::Text(s)),
                Err(_) => Ok(TypedFile::Etc),
            },
        }
    }
}

pub fn check_read_path(path: &Path, working_dir: &str) -> Result<Result<(String, String), ToolCallError>, Error> {
    let joined_path = match normalize_path(path) {
        Some(path) if path.is_empty() => String::from("."),
        Some(path) => path.join("/"),
        None => path.join("/"),
    };

    // If the AI tries to read `../../Documents/`, that's a permission error whether or not the path exists.
    if !check_read_permission(path) {
        return Ok(Err(ToolCallError::NoPermissionToRead { path: joined_path }));
    }

    let real_path = join(working_dir, &joined_path)?;

    // If the file is a symlink, `exists` checks the existence of the pointee, not the pointer
    if !exists(&real_path) && !is_symlink(&real_path) {
        return Ok(Err(ToolCallError::NoSuchFile { path: joined_path }));
    }

    Ok(Ok((joined_path, real_path)))
}

fn check_read_permission(path: &Path) -> bool {
    match normalize_path(path) {
        Some(path) => match (path.get(0).map(|s| s.as_str()), path.get(1).map(|s| s.as_str())) {
            (Some(".neukgu"), Some("skills")) => true,
            (Some(".neukgu"), _) => false,

            // If `path.get(0)` is `None`, that's working-dir. The agent can read working-dir.
            _ => true,
        },
        None => false,
    }
}

impl Context {
    // Sometimes, when I ask AI to inspect a code repository, it tries to read all
    // the files in the repository, even if the repository is too big to fit in the
    // AI's context. Neukgu's auto-context-engineering will remove the traces, but
    // the AI doesn't know that, so the AI will fall into an infinite loop.
    //
    // In order to prevent that, the harness forces the AI to write summaries regularly.
    pub fn is_reading_too_much(&mut self, config: &Config) -> Result<bool, Error> {
        Ok(self.history.len() >= config.max_read_without_write && {
            let this_turn = self.history.last().unwrap();

            this_turn.result == TurnResultSummary::ToolCallSuccess && {
                let recent_turn_ids = self.history.iter().rev().filter(
                    |t| t.result != TurnResultSummary::ParseError
                ).take(config.max_read_without_write).map(|t| t.clone()).collect::<Vec<_>>();
                let mut recent_turns = vec![];

                for turn_id in recent_turn_ids.iter() {
                    recent_turns.push(self.load_turn(&turn_id.id)?);
                }

                recent_turns.iter().all(
                    |turn| matches!(
                        turn.turn_result,
                        TurnResult::ToolCallSuccess(
                            ToolCallSuccess::ReadText { .. } |
                            ToolCallSuccess::ReadPdf { .. } |
                            ToolCallSuccess::ReadImage { .. } |
                            ToolCallSuccess::ReadDir { .. }
                        ),
                    )
                )
            }
        })
    }
}
