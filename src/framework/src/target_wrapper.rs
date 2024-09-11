use std::path::Path;

use crate::{
    checkpoint::{DirReader, LinkInfo, StorageReader},
    engine::{TargetId, TargetInfo, TargetQueryBy, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{CheckPointMetaEngine, CheckPointVersion, StorageItemAttributes},
    target::{Target, TargetCheckPoint, TargetTask},
    task::TaskInfo,
};

pub(crate) struct TargetWrapper {
    target_id: TargetQueryBy,
    engine: Engine,
}

impl TargetWrapper {
    pub(crate) fn new(target_id: TargetId, engine: Engine) -> Self {
        Self {
            target_id: TargetQueryBy::Id(target_id),
            engine,
        }
    }
}

#[async_trait::async_trait]
impl Target<String, String, String, String, String, String> for TargetWrapper {
    fn target_id(&self) -> TargetId {
        match &self.target_id {
            TargetQueryBy::Id(id) => *id,
            TargetQueryBy::Url(_) => unreachable!(),
        }
    }

    async fn target_info(&self) -> BackupResult<TargetInfo> {
        let t = self.engine.get_target_impl(&self.target_id).await?;
        match t {
            Some(t) => t.target_info().await,
            None => Err(BackupError::ErrorState(format!(
                "target({:?}) has been removed.",
                self.target_id()
            ))),
        }
    }

    async fn target_task(
        &self,
        task_info: TaskInfo,
    ) -> BackupResult<Box<dyn TargetTask<String, String, String, String, String, String>>> {
        let t = self.engine.get_target_impl(&self.target_id).await?;
        match t {
            Some(t) => t.target_task(task_info).await,
            None => Err(BackupError::ErrorState(format!(
                "target({:?}) has been removed.",
                self.target_id()
            ))),
        }
    }

    async fn update_config(&self, config: &str) -> BackupResult<()> {
        let t = self.engine.get_target_impl(&self.target_id).await?;
        match t {
            Some(t) => t.update_config(config).await,
            None => Err(BackupError::ErrorState(format!(
                "target({:?}) has been removed.",
                self.target_id()
            ))),
        }
    }
}

pub(crate) struct TargetTaskWrapper {
    target_id: TargetId,
    task_uuid: TaskUuid,
    engine: Engine,
}

impl TargetTaskWrapper {
    pub(crate) fn new(target_id: TargetId, task_uuid: TaskUuid, engine: Engine) -> Self {
        Self {
            target_id,
            engine,
            task_uuid,
        }
    }
}

#[async_trait::async_trait]
impl TargetTask<String, String, String, String, String, String> for TargetTaskWrapper {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }
    async fn estimate_consume_size(&self, meta: &CheckPointMetaEngine) -> BackupResult<u64> {
        self.engine
            .get_target_task_impl(self.target_id, &self.task_uuid)
            .await?
            .estimate_consume_size(meta)
            .await
    }
    async fn fill_target_meta(
        &self,
        meta: &mut CheckPointMetaEngine,
    ) -> BackupResult<(Vec<String>, Box<dyn TargetCheckPoint>)> {
        self.engine
            .get_target_task_impl(self.target_id, &self.task_uuid)
            .await?
            .fill_target_meta(meta)
            .await
    }

    async fn target_checkpoint_from_filled_meta(
        &self,
        meta: &CheckPointMetaEngine,
        target_meta: &[&str],
    ) -> BackupResult<Box<dyn TargetCheckPoint>> {
        self.engine
            .get_target_task_impl(self.target_id, &self.task_uuid)
            .await?
            .target_checkpoint_from_filled_meta(meta, target_meta)
            .await
    }
}

pub(crate) struct TargetCheckPointWrapper {
    target_id: TargetId,
    task_uuid: TaskUuid,
    version: CheckPointVersion,
    engine: Engine,
}

impl TargetCheckPointWrapper {
    pub(crate) fn new(
        target_id: TargetId,
        task_uuid: TaskUuid,
        version: CheckPointVersion,
        engine: Engine,
    ) -> Self {
        Self {
            target_id,
            engine,
            task_uuid,
            version,
        }
    }
}

#[async_trait::async_trait]
impl StorageReader for TargetCheckPointWrapper {
    async fn read_dir(&self, path: &Path) -> BackupResult<Box<dyn DirReader>> {
        self.engine
            .get_target_checkpoint_impl(self.target_id, &self.task_uuid, self.version)
            .await?
            .read_dir(path)
            .await
    }
    async fn file_size(&self, path: &Path) -> BackupResult<u64> {
        self.engine
            .get_target_checkpoint_impl(self.target_id, &self.task_uuid, self.version)
            .await?
            .file_size(path)
            .await
    }
    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        self.engine
            .get_target_checkpoint_impl(self.target_id, &self.task_uuid, self.version)
            .await?
            .read_file(path, offset, length)
            .await
    }
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo> {
        self.engine
            .get_target_checkpoint_impl(self.target_id, &self.task_uuid, self.version)
            .await?
            .read_link(path)
            .await
    }
    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes> {
        self.engine
            .get_target_checkpoint_impl(self.target_id, &self.task_uuid, self.version)
            .await?
            .stat(path)
            .await
    }
}

#[async_trait::async_trait]
impl TargetCheckPoint for TargetCheckPointWrapper {
    fn checkpoint_version(&self) -> CheckPointVersion {
        self.version
    }

    async fn transfer(&self) -> BackupResult<()> {
        self.engine
            .get_target_checkpoint_impl(self.target_id, &self.task_uuid, self.version)
            .await?
            .transfer()
            .await
    }
}
