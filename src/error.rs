use error_enum::{ErrorContainer, ErrorEnum, PrettyError};

#[derive(Debug, PartialEq, Eq, ErrorContainer)]
pub enum CliErrors {
    Namespace(NamespaceErrors),
    Tokio(TokioErrors),
    Unknown(UnknownErrors),
}

#[derive(Debug, PartialEq, Eq, ErrorEnum)]
#[error_enum(prefix = "UNKNOWN")]
pub enum UnknownErrors {
    #[error_enum(description = "Unknown Error")]
    Unknown(String),
}

#[derive(Debug, PartialEq, Eq, ErrorEnum)]
#[error_enum(prefix = "TOKIO")]
pub enum TokioErrors {
    #[error_enum(description = "Tokio Runtime Error")]
    Error(String),
}

#[derive(Debug, PartialEq, Eq, ErrorEnum)]
#[error_enum(prefix = "NS")]
pub enum NamespaceErrors {
    #[error_enum(description = "Unable to open network namespace")]
    UnableToOpenNamepace(String),
}

impl From<tokio::io::Error> for CliErrors {
    fn from(e: tokio::io::Error) -> CliErrors {
        CliErrors::Tokio(TokioErrors::Error(e.to_string()))
    }
}

impl From<nix::Error> for CliErrors {
    fn from(e: nix::Error) -> CliErrors {
        CliErrors::Namespace(NamespaceErrors::UnableToOpenNamepace(e.to_string()))
    }
}

impl From<reqwest::Error> for CliErrors {
    fn from(e: reqwest::Error) -> CliErrors {
        CliErrors::Unknown(UnknownErrors::Unknown(e.to_string()))
    }
}
