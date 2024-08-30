use crate::error::{BackupError, BackupResult};

pub enum CheckPointStatus {
    Standby,
    Start,
    Stop,
    Success,
    Failed(BackupError),
}

#[async_trait::async_trait]
pub trait CheckPoint {
    async fn restore(&self) -> BackupResult<()>;
}
