use crate::{
    engine::{TargetId, TargetInfo, TargetQueryBy},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    target::{Target, TargetTask},
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
impl Target<String, String, String, String, String> for TargetWrapper {
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
    ) -> BackupResult<Box<dyn TargetTask<String, String, String, String, String>>> {
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
