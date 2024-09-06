use crate::{
    engine::{SourceId, SourceInfo, SourceQueryBy},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    source::{Source, SourceTask},
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
        let s = self.engine.get_source(&self.source_id).await?;
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
        let s = self.engine.get_source(&self.source_id).await?;
        match s {
            Some(s) => s.source_task(task_info).await,
            None => Err(BackupError::ErrorState(format!(
                "source({:?}) has been removed.",
                self.source_id()
            ))),
        }
    }

    async fn update_config(&self, config: &str) -> BackupResult<()> {
        let s = self.engine.get_source(&self.source_id).await?;
        match s {
            Some(s) => s.update_config(config).await,
            None => Err(BackupError::ErrorState(format!(
                "source({:?}) has been removed.",
                self.source_id()
            ))),
        }
    }
}
