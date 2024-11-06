use crate::{
    checkpoint::{ItemEnumerate, ItemId, StorageReader},
    engine::{TargetId, TargetInfo, TaskUuid},
    error::BackupResult,
    meta::{CheckPointMeta, CheckPointVersion, MetaBound},
    task::TaskInfo,
};

pub enum TargetStatus {
    StandBy,
    Transfering,
    Finish,
    Failed,
}

#[async_trait::async_trait]
pub trait TargetFactory: Send + Sync {
    async fn from_target_info(&self, target_info: TargetInfo) -> BackupResult<Box<dyn Target>>;
}

#[async_trait::async_trait]
pub trait Target: Send + Sync {
    fn target_id(&self) -> TargetId;
    async fn target_info(&self) -> BackupResult<TargetInfo>;
    async fn target_task(
        &self,
        task_uuid: &TaskUuid,
        target_entitiy: &str,
    ) -> BackupResult<Box<dyn TargetTask>>;

    async fn update_config(&self, config: &str) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait TargetTask: Send + Sync {
    fn task_uuid(&self) -> &TaskUuid;
    async fn target_checkpoint(
        &self,
        checkpoint_version: &CheckPointVersion,
    ) -> BackupResult<dyn TargetCheckPoint>;
}

#[async_trait::async_trait]
pub trait TargetCheckPoint: StorageReader + Send + Sync {
    fn checkpoint_version(&self) -> CheckPointVersion;
    async fn transfer(&self) -> BackupResult<()>;
    async fn stop(&self) -> BackupResult<()>;
    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate>;
    async fn status(&self) -> BackupResult<TargetStatus>;
    async fn wait_status<F>(&self) -> BackupResult<StatusWaitor<TargetStatus>>;
}
