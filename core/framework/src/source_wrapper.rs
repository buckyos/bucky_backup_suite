use std::path::Path;

use crate::{
    checkpoint::{DirChildType, ItemEnumerate, LinkInfo},
    engine::{SourceId, SourceInfo, SourceQueryBy, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{Attributes, LockedSourceStateId},
    source::{LockedSource, Source, SourceStatus, SourceTask},
    status_waiter::Waiter,
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

    async fn source_task(
        &self,
        task_uuid: &TaskUuid,
        source_entitiy: &str,
    ) -> BackupResult<Box<dyn SourceTask>> {
        let s = self.engine.get_source_impl(&self.source_id).await?;
        match s {
            Some(s) => s.source_task(task_uuid, source_entitiy).await,
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
    // lock
    async fn original_state(&self) -> BackupResult<Option<String>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .original_state()
            .await
    }

    async fn lock_state(&self, original_state: Option<&str>) -> BackupResult<Option<String>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .locked_state(original_state)
            .await
    }

    async fn unlock_state(&self, original_state: Option<&str>) -> BackupResult<()> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .restore_state(original_state)
            .await
    }

    async fn locked_source(
        &self,
        locked_state_id: LockedSourceStateId,
        locked_state: Option<&str>,
    ) -> BackupResult<Box<dyn LockedSource>> {
        self.engine
            .get_source_task_impl(self.source_id, &self.task_uuid)
            .await?
            .locked_source(locked_state_id, locked_state)
            .await
    }
}

pub(crate) struct LockedSourceWrapper {
    source_id: SourceId,
    task_uuid: TaskUuid,
    locked_state_id: LockedSourceStateId,
    engine: Engine,
}

impl LockedSourceWrapper {
    pub(crate) fn new(
        source_id: SourceId,
        task_uuid: TaskUuid,
        locked_state_id: LockedSourceStateId,
        engine: Engine,
    ) -> Self {
        Self {
            source_id,
            engine,
            task_uuid,
            locked_state_id,
        }
    }
}

#[async_trait::async_trait]
impl LockedSource for LockedSourceWrapper {
    fn locked_state_id(&self) -> LockedSourceStateId {
        self.locked_state_id
    }

    async fn prepare(&self) -> BackupResult<()> {
        self.engine
            .get_locked_source_impl(self.source_id, &self.task_uuid, self.locked_state_id)
            .await?
            .prepare()
            .await
    }
    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate> {
        self.engine
            .get_locked_source_impl(self.source_id, &self.task_uuid, self.locked_state_id)
            .await?
            .enumerate_item()
            .await
    }
    async fn status(&self) -> BackupResult<SourceStatus> {
        self.engine
            .get_locked_source_impl(self.source_id, &self.task_uuid, self.locked_state_id)
            .await?
            .status()
            .await
    }
    async fn status_waiter(&self) -> BackupResult<Waiter<BackupResult<SourceStatus>>> {
        self.engine
            .get_locked_source_impl(self.source_id, &self.task_uuid, self.locked_state_id)
            .await?
            .status_waiter()
            .await
    }
}
