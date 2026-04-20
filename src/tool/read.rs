use super::{Path, normalize_path};
use crate::{Context, ImageId, PdfId, normalize_and_get_id, render_and_get_id};
use hayro::hayro_syntax::Pdf;
use ragit_fs::{FileError, basename, is_dir, read_bytes, read_dir};
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
    Dir(Vec<FileEntry>),
    Pdf(PdfId),
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
    ImageFile {
        name: String,
        size: (u32, u32),
    },
    EtcFile {
        name: String,
        bytes: u64,
    },
    Dir {
        name: String,
    },
}

// It already checked that the path exists.
pub fn read_file(path: &str, context: &Context) -> Result<TypedFile, FileError> {
    if is_dir(path) {
        let mut entries = vec![];
        let mut basenames = vec![];

        for e in read_dir(path, true)? {
            let name = basename(&e)?;

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

                let entry = match String::from_utf8(bytes.clone()) {
                    Ok(s) => {
                        let chars = s.chars().count() as u64;
                        let lines = s.lines().count() as u64;
                        let bytes = bytes.len() as u64;
                        FileEntry::TextFile { name, bytes, chars, lines }
                    },
                    Err(_) => match image::load_from_memory(&bytes) {
                        Ok(buffer) => FileEntry::ImageFile { name, size: (buffer.width(), buffer.height()) },
                        Err(_) => match Pdf::new(bytes.clone()) {
                            Ok(pdf) => FileEntry::PdfFile { name, pages: pdf.pages().len() as u64 },
                            Err(_) => FileEntry::EtcFile { name, bytes: bytes.len() as u64 },
                        },
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
        let bytes = read_bytes(path)?;

        match String::from_utf8(bytes.clone()) {
            Ok(s) => Ok(TypedFile::Text(s)),
            Err(_) => match normalize_and_get_id(&bytes) {
                Ok(id) => Ok(TypedFile::Image(id)),
                Err(_) => match render_and_get_id(&bytes) {
                    Ok(id) => Ok(TypedFile::Pdf(id)),
                    Err(e) => {
                        eprintln!("{e:?}");
                        Ok(TypedFile::Etc)
                    },
                },
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
