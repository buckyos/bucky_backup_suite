use thiserror::Error;



#[derive(Error, Debug)]
pub enum BuckyBackupError {
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("AlreadyDone: {0}")]
    AlreadyDone(String),
    #[error("TryLater: {0}")]
    TryLater(String),
    #[error("NeedProcess: {0}")]
    NeedProcess(String),
    #[error("Failed: {0}")]
    Failed(String),
    #[error("NotFound: {0}")]
    NotFound(String),
}

pub type BackupResult<T> = std::result::Result<T, BuckyBackupError>;
