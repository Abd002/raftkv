pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Corruption(String),
    RecordTooLarge,
    Poisoned,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O: error {error}"),
            Self::Corruption(error) => write!(f, "Corruption: {error}"),
            Self::RecordTooLarge => write!(f, "Record is Too Large."),
            Self::Poisoned => write!(f, "This data is Poisoned."),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}
