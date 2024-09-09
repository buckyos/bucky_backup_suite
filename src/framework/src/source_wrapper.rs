use crate::{
    checkpoint::{DirReader, LinkInfo, StorageReader},
    engine::{SourceId, SourceInfo, SourceQueryBy, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{PreserveStateId, StorageItemAttributes},
    source::{Source, SourcePreserved, SourceTask},
    task::TaskInfo,
};

pub(crate) struct SourceWrapper {
    source_id: SourceQueryBy,
    engine: Engine,
}

impl SourceWrapper {
    pub(crate) fn new(source_id: SourceId, engine: Engine) -> Self {
        Self {
            source_id: SourceQueryBy::Id(source_id),
            engine,
        }
    }
}

#[async_trait::async_trait]
impl Source for SourceWrapper {
    fn source_id(&self) -> SourceId {
        match &self.source_id {
            SourceQueryBy::Id(id) => *id,
            SourceQueryBy::Url(_) => unreachable!(),
        }
    }

    async fn source_info(&self) -> BackupResult<SourceInfo> {
        let s = self.engine.get_source_impl(&self.source_id).await?;
        match s {
            Some(s) => s.source_info().await,
            None => Err(BackupError::ErrorState(format!(
                "source({:?}) has been removed.",
                self.source_id()
            ))),
        }
    }

    async fn source_task(&self, task_info: TaskInfo) -> BackupResult<Box<dyn SourceTask>> {
        let s = self.engine.get_source_impl(&self.source_id).await?;
        match s {
            Some(s) => s.source_task(task_info).await,
            None => Err(BackupError::ErrorState(format!(
                "source({:?}) has been removed.",
                self.source_id()
            ))),
        }
    }

    async fn update_config(&self, config: &str) -> BackupResult<()> {
        let s = self.engine.get_source_impl(&self.source_id).await?;
        match s {
            Some(s) => s.update_config(config).await,
            None => Err(BackupError::ErrorState(format!(
                "source({:?}) has been removed.",
                self.source_id()
            ))),
        }
    }
}

pub(crate) struct SourceTaskWrapper {
    source_id: SourceId,
    task_uuid: TaskUuid,
    engine: Engine,
}

impl SourceTaskWrapper {
    pub(crate) fn new(source_id: SourceId, task_uuid: TaskUuid, engine: Engine) -> Self {
        Self {
            source_id,
            engine,
            task_uuid,
        }
    }
}

#[async_trait::async_trait]
impl SourceTask for SourceTaskWrapper {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }
    // preserve
    async fn original_state(&self) -> BackupResult<Option<String>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .original_state()
            .await
    }

    async fn preserved_state(&self) -> BackupResult<Option<String>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .preserved_state()
            .await
    }

    async fn restore_state(&self, original_state: &str) -> BackupResult<()> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .restore_state(original_state)
            .await
    }

    async fn source_preserved(
        &self,
        preserved_state_id: PreserveStateId,
        preserved_state: Option<&str>,
    ) -> BackupResult<Box<dyn SourcePreserved>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .source_preserved(preserved_state_id, preserved_state)
            .await
    }
}

pub(crate) struct SourcePreservedWrapper {
    source_id: SourceId,
    task_uuid: TaskUuid,
    preserved_state_id: PreserveStateId,
    engine: Engine,
}

impl SourcePreservedWrapper {
    pub(crate) fn new(
        source_id: SourceId,
        task_uuid: TaskUuid,
        preserved_state_id: PreserveStateId,
        engine: Engine,
    ) -> Self {
        Self {
            source_id,
            engine,
            task_uuid,
            preserved_state_id,
        }
    }
}

#[async_trait::async_trait]
impl StorageReader for SourcePreservedWrapper {
    // for checkpoint
    async fn read_dir(&self, path: &[u8]) -> BackupResult<Box<dyn DirReader>> {
        self.engine
            .get_source_preserved_impl(self.source_id, &self.task_uuid, self.preserved_state_id)
            .await?
            .read_dir(path)
            .await
    }
    async fn read_file(&self, path: &[u8], offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        self.engine
            .get_source_preserved_impl(self.source_id, &self.task_uuid, self.preserved_state_id)
            .await?
            .read_file(path, offset, length)
            .await
    }
    async fn read_link(&self, path: &[u8]) -> BackupResult<LinkInfo> {
        self.engine
            .get_source_preserved_impl(self.source_id, &self.task_uuid, self.preserved_state_id)
            .await?
            .read_link(path)
            .await
    }
    async fn stat(&self, path: &[u8]) -> BackupResult<StorageItemAttributes> {
        self.engine
            .get_source_preserved_impl(self.source_id, &self.task_uuid, self.preserved_state_id)
            .await?
            .stat(path)
            .await
    }
}

#[async_trait::async_trait]
impl SourcePreserved for SourcePreservedWrapper {
    fn preserved_state_id(&self) -> PreserveStateId {
        self.preserved_state_id
    }
}
