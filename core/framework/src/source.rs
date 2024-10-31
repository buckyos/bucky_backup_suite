use crate::{
    checkpoint::{ItemId, StorageReader},
    engine::{SourceId, SourceInfo, TaskUuid},
    error::BackupResult,
};

#[async_trait::async_trait]
pub trait SourceFactory: Send + Sync {
    async fn from_source_info(&self, source_info: SourceInfo) -> BackupResult<Box<dyn Source>>;
}

#[async_trait::async_trait]
pub trait Source: Send + Sync {
    fn source_id(&self) -> SourceId;
    async fn source_info(&self) -> BackupResult<SourceInfo>;
    async fn source_task(
        &self,
        task_uuid: &TaskUuid,
        source_entitiy: &str,
    ) -> BackupResult<Box<dyn SourceTask>>;

    async fn update_config(&self, config: &str) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait SourceTask: Send + Sync {
    fn task_uuid(&self) -> &TaskUuid;
    // preserve
    async fn original_state(&self) -> BackupResult<Option<String>>;
    async fn lock_state(&self, original_state: &str) -> BackupResult<Option<String>>;
    async fn restore_state(&self, original_state: &str) -> BackupResult<()>;

    async fn locked_source(
        &self,
        locked_state_id: LockedStateId,
        locked_state: Option<&str>,
    ) -> BackupResult<Box<dyn LockedSource>>;
}

#[async_trait::async_trait]
pub trait LockedSource: StorageReader + Send + Sync {
    fn locked_state_id(&self) -> LockedStateId;
    async fn prepare(&self) -> BackupResult<()>;
    async fn item_iter(&self) -> BackupResult<futures::stream::Iter<ItemId>>;
}
