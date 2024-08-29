use crate::{error::BackupResult, task::TaskInfo};

#[async_trait::async_trait]
pub trait SourceFactory {
    async fn from_task(task_info: TaskInfo) -> BackupResult<Box<dyn Source>>;
}

#[async_trait::async_trait]
pub trait Source {
    async fn original_state(&self) -> BackupResult<Option<String>>;
    async fn preserved_state(&self) -> BackupResult<Option<String>>;
    async fn restore_state(&self, original_state: String) -> BackupResult<()>;
}
