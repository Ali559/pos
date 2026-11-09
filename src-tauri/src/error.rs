use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Errors returned to the front end by commands.
///
/// Serialized as `{ code, message, detail? }`. `code` is stable and is what
/// the UI translates (`errors.<code>` in the locale files); `message` is an
/// English fallback for logs and unknown codes.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database is not ready")]
    DbNotReady,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("could not read the image: {0}")]
    Image(#[from] image::ImageError),
    #[error("could not compute the blurhash")]
    Blurhash,
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("could not hash the password")]
    Hash,

    /// Input that fails validation; message is shown as-is
    #[error("{0}")]
    Invalid(String),
    #[error("sign in first")]
    NotSignedIn,
    #[error("missing permission {0}")]
    Forbidden(String),
    #[error("wrong username, password or PIN")]
    BadCredentials,
    #[error("not found")]
    NotFound,
    /// A business rule refused the action; the code says which one
    #[error("{0}")]
    Rule(&'static str),
    /// Same, naming the thing it's about (e.g. which product is short)
    #[error("{0}: {1}")]
    RuleAbout(&'static str, String),
    /// An error the main computer returned to this till, passed on as is
    #[error("{message}")]
    Remote { code: String, message: String, detail: Option<String> },
}

impl Error {
    pub fn code(&self) -> &str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::NotSignedIn => "not_signed_in",
            Error::Forbidden(_) => "forbidden",
            Error::BadCredentials => "bad_credentials",
            Error::NotFound => "not_found",
            Error::Rule(code) | Error::RuleAbout(code, _) => code,
            Error::Remote { code, .. } => code,
            Error::Image(_) | Error::Blurhash => "bad_image",
            _ => "internal",
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Error", 3)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        match self {
            Error::Forbidden(detail) | Error::RuleAbout(_, detail) => s.serialize_field("detail", detail)?,
            Error::Remote { detail: Some(detail), .. } => s.serialize_field("detail", detail)?,
            _ => s.skip_field("detail")?,
        }
        s.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
