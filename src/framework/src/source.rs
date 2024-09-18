use crate::{
    checkpoint::StorageReader,
    engine::{SourceId, SourceInfo, TaskUuid},
    error::BackupResult,
    meta::PreserveStateId,
    task::TaskInfo,
};

#[async_trait::async_trait]
pub trait SourceFactory: Send + Sync {
    async fn from_source_info(&self, source_info: SourceInfo) -> BackupResult<Box<dyn Source>>;
}

#[async_trait::async_trait]
pub trait Source: Send + Sync {
    fn source_id(&self) -> SourceId;
    async fn source_info(&self) -> BackupResult<SourceInfo>;
    async fn source_task(&self, task_info: TaskInfo) -> BackupResult<Box<dyn SourceTask>>;

    async fn update_config(&self, config: &str) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait SourceTask: Send + Sync {
    fn task_uuid(&self) -> &TaskUuid;
    // preserve
    async fn original_state(&self) -> BackupResult<Option<String>>;
    async fn preserved_state(&self) -> BackupResult<Option<String>>;
    async fn restore_state(&self, original_state: &str) -> BackupResult<()>;

    async fn source_preserved(
        &self,
        preserved_state_id: PreserveStateId,
        preserved_state: Option<&str>,
    ) -> BackupResult<Box<dyn SourcePreserved>>;
}

#[async_trait::async_trait]
pub trait SourcePreserved: StorageReader + Send + Sync {
    fn preserved_state_id(&self) -> PreserveStateId;
}
