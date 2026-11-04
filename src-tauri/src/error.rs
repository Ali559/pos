use serde::{Serialize, Serializer};

/// Errors returned to the front end by commands. Serialized as a plain
/// message string so `invoke()` rejects with something displayable.
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
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
