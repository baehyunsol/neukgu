use super::{Path, normalize_path};
use crate::{Context, Error, ImageId, PdfId, normalize_and_get_id, render_and_get_id};
use hayro::hayro_syntax::Pdf;
use ragit_fs::{basename, extension, is_dir, join, read_bytes, read_dir};
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
    Image(ImageId),
    BrokenImage { error: String },
    Dir(Vec<FileEntry>),
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
                Ok(id) => Ok(TypedFile::Image(id)),
                Err(e) => Ok(TypedFile::BrokenImage { error: format!("{e:?}") }),
            },
            _ => match String::from_utf8(bytes) {
                Ok(s) => Ok(TypedFile::Text(s)),
                Err(_) => Ok(TypedFile::Etc),
            },
        }
    }
}

pub fn check_read_permission(path: &Path) -> bool {
    match normalize_path(path) {
        Some(path) => match path.get(0).map(|s| s.as_str()) {
            Some(".neukgu") => false,

            // If `path.get(0)` is `None`, that's working-dir. The agent can read working-dir.
            _ => true,
        },
        None => false,
    }
}
