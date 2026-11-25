pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Corruption(String),
    Wal(kv_wal::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error {error}"),
            Self::Corruption(error) => write!(f, "Corruption: {error}"),
            Self::Wal(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

impl From<kv_wal::Error> for Error {
    fn from(value: kv_wal::Error) -> Self {
        Error::Wal(value)
    }
}
