use crate::{
    engine::{SourceId, SourceInfo, SourceQueryBy, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::CheckPointMeta,
    source::{Source, SourceCheckPoint, SourceTask},
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
impl Source<String, String, String, String, String> for SourceWrapper {
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

    async fn source_task(
        &self,
        task_info: TaskInfo,
    ) -> BackupResult<Box<dyn SourceTask<String, String, String, String, String>>> {
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
impl SourceTask<String, String, String, String, String> for SourceTaskWrapper {
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

    async fn source_checkpoint(
        &self,
        meta: CheckPointMeta<String, String, String, String, String>,
    ) -> BackupResult<Box<dyn SourceCheckPoint>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .source_checkpoint(meta)
            .await
    }
}
