use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error(
        "credential shell import requires a literal assignment; shell expressions are not executed"
    )]
    UnsupportedShellAssignment,
    #[error("cannot determine a config directory; set NIUCODE_CONFIG_DIR or HOME")]
    NoConfigDir,
    #[error("cannot store an empty API key")]
    EmptyKey,
    #[error("credentials file I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("credentials file is not valid TOML")]
    CorruptCredentials,
    #[error("could not serialize credentials")]
    Serialize,
}
