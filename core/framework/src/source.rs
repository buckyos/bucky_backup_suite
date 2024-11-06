use crate::{
    checkpoint::{ItemEnumerate, ItemId},
    engine::{SourceId, SourceInfo, TaskUuid},
    error::BackupResult,
    meta::LockedSourceStateId,
};

pub enum SourceStatus {
    StandBy,
    Scaning,
    Finish,
    Failed,
}

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

    async fn original_state(&self) -> BackupResult<Option<String>>;
    async fn lock_state(&self, original_state: &str) -> BackupResult<Option<String>>;
    async fn unlock_state(&self, original_state: &str) -> BackupResult<()>;

    async fn locked_source(
        &self,
        locked_state_id: LockedSourceStateId,
        locked_state: Option<&str>,
    ) -> BackupResult<Box<dyn LockedSource>>;
}

#[async_trait::async_trait]
pub trait LockedSource: Send + Sync {
    fn locked_state_id(&self) -> LockedSourceStateId;
    async fn prepare(&self) -> BackupResult<()>;
    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate>;
    async fn status(&self) -> BackupResult<SourceStatus>;
    async fn wait_status<F>(&self) -> BackupResult<StatusWaitor<SourceStatus>>;
}
