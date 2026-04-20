use crate::TurnId;
use ragit_fs::FileError;

#[derive(Debug)]
pub enum Error {
    ApiKeyNotFound { env_var: String },
    UnavailableBinaries(Vec<String>),
    FailedToAcquireWriteLock,
    IndexDirNotFound,
    IndexDirAlreadyExists,

    // CLI has `--instruction` arg, but `neukgu-instruction.md` already exists.
    InstructionAlreadyExists,

    FrontendNotAvailable,
    InvalidModelName(String),
    CannotCalcDiff { path: String, turn_id: TurnId },
    MockApiExpectationFailure { expect: String },

    HttpError { status_code: u16 },
    CliError {
        message: String,
        span: Option<ragit_cli::RenderedSpan>,
    },

    IoError(std::io::Error),
    FromUtf8Error(std::string::FromUtf8Error),

    /// I don't know how to handle `anyhow::Error`, so I just convert it to string.
    BrowserError(String),

    /// see <https://docs.rs/ragit-fs/latest/ragit_fs/struct.FileError.html>
    FileError(FileError),

    /// see <https://docs.rs/iced/latest/iced/enum.Error.html>
    IcedError(iced::Error),

    /// see <https://docs.rs/image/latest/image/error/enum.ImageError.html>
    ImageError(image::ImageError),

    /// see <https://docs.rs/hayro-syntax/0.6.0/hayro_syntax/enum.LoadPdfError.html>
    LoadPdfError(hayro::hayro_syntax::LoadPdfError),

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

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Error {
        Error::FromUtf8Error(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e)
    }
}

impl From<iced::Error> for Error {
    fn from(e: iced::Error) -> Error {
        Error::IcedError(e)
    }
}

impl From<image::ImageError> for Error {
    fn from(e: image::ImageError) -> Error {
        Error::ImageError(e)
    }
}

impl From<hayro::hayro_syntax::LoadPdfError> for Error {
    fn from(e: hayro::hayro_syntax::LoadPdfError) -> Error {
        Error::LoadPdfError(e)
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
