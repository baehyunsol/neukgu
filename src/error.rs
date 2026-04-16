use ragit_fs::FileError;

#[derive(Debug)]
pub enum Error {
    ApiKeyNotFound { env_var: String },
    UnavailableBinaries(Vec<String>),
    FailedToAcquireWriteLock,
    IndexDirNotFound,
    IndexDirAlreadyExists,

    // CLI has `--instruction` arg, but `instruction.md` already exists.
    InstructionAlreadyExists,
    FrontendNotAvailable,
    HttpError { status_code: u16 },
    CliError {
        message: String,
        span: Option<ragit_cli::RenderedSpan>,
    },

    /// I don't know how to handle `anyhow::Error`, so I just convert it to string.
    BrowserError(String),

    /// see <https://docs.rs/ragit-fs/latest/ragit_fs/struct.FileError.html>
    FileError(FileError),

    IoError(std::io::Error),

    /// see <https://docs.rs/image/latest/image/error/enum.ImageError.html>
    ImageError(image::ImageError),

    /// see <https://docs.rs/png/latest/png/enum.EncodingError.html>
    PngEncodingError(png::EncodingError),

    /// see <https://docs.rs/reqwest/latest/reqwest/struct.Error.html>
    ReqwestError(reqwest::Error),

    /// see <https://docs.rs/serde_json/latest/serde_json/struct.Error.html>
    SerdeJsonError(serde_json::Error),

    /// see <https://docs.rs/usvg/0.47.0/usvg/enum.Error.html>
    SvgError(resvg::usvg::Error),

    /// see <https://docs.rs/tera/latest/tera/struct.Error.html>
    TeraError(tera::Error),
}

impl From<ragit_cli::Error> for Error {
    fn from(e: ragit_cli::Error) -> Self {
        Error::CliError {
            message: e.kind.render(),
            span: e.span,
        }
    }
}

impl From<FileError> for Error {
    fn from(e: FileError) -> Error {
        Error::FileError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e)
    }
}

impl From<image::ImageError> for Error {
    fn from(e: image::ImageError) -> Error {
        Error::ImageError(e)
    }
}

impl From<png::EncodingError> for Error {
    fn from(e: png::EncodingError) -> Error {
        Error::PngEncodingError(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Error {
        Error::ReqwestError(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::SerdeJsonError(e)
    }
}

impl From<resvg::usvg::Error> for Error {
    fn from(e: resvg::usvg::Error) -> Error {
        Error::SvgError(e)
    }
}

impl From<tera::Error> for Error {
    fn from(e: tera::Error) -> Error {
        Error::TeraError(e)
    }
}

pub fn from_browser_error(e: anyhow::Error) -> Error {
    Error::BrowserError(format!("{e:?}"))
}
