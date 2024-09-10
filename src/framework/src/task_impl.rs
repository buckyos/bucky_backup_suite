use std::{
    sync::{atomic::AtomicU64, Arc},
    time::SystemTime,
};

use crate::{
    build_meta::{estimate_occupy_size, meta_from_delta, meta_from_reader},
    checkpoint::{CheckPoint, CheckPointStatus},
    checkpoint_impl::CheckPointWrapper,
    engine::{FindTaskBy, ListOffset, SourceQueryBy, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{CheckPointMeta, CheckPointMetaEngine, CheckPointVersion, PreserveStateId, StorageItem},
    source::SourceTask,
    source_wrapper::SourceTaskWrapper,
    target::TargetTask,
    task::{
        HistoryStrategy, ListCheckPointFilter, ListCheckPointFilterTime,
        ListPreservedSourceStateFilter, PreserveSourceState, SourceState, Task, TaskInfo,
    },
};

pub(crate) struct TaskImpl {
    info: TaskInfo,
    egnine: Engine,
    source: SourceTaskWrapper,
}

impl TaskImpl {
    pub fn new(task_info: TaskInfo, engine: Engine) -> Self {
        let uuid = task_info.uuid;
        let source_id = task_info.source_id;
        TaskImpl {
            info: task_info,
            egnine: engine.clone(),
            source: SourceTaskWrapper::new(source_id, uuid, engine),
        }
    }
}

#[async_trait::async_trait]
impl PreserveSourceState for TaskImpl {
    async fn preserve(&self) -> BackupResult<PreserveStateId> {
        let org_state = self.source.original_state().await?;
        let state_id = self
            .egnine
            .save_source_original_state(&self.info.uuid, org_state.as_deref())
            .await?;
        let preserved_state = self.source.preserved_state().await?;
        self.egnine
            .save_source_preserved_state(state_id, preserved_state.as_deref())
            .await?;
        Ok(state_id)
    }

    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState> {
        self.egnine.load_source_preserved_state(state_id).await
    }

    // Any preserved state for backup by source will be restored automatically when it done(success/fail/cancel).
    // But it should be restored by the application when no transfering start, because the engine is uncertain whether the user will use it to initiate the transfer task.
    // It will fail when a transfer task is valid, you should wait it done or cancel it.
    async fn restore(&self, state_id: PreserveStateId) -> BackupResult<()> {
        todo!("check it's idle");
        let state = self.egnine.load_source_preserved_state(state_id).await?;
        let original_state = match state {
            SourceState::None => return Ok(()),
            SourceState::Original(original_state) => original_state,
            SourceState::Preserved(original_state, _) => original_state,
        };

        if let Some(original_state) = &original_state {
            self.source.restore_state(original_state.as_str()).await?;
        }
        self.egnine.delete_source_preserved_state(state_id).await
    }

    async fn restore_all_idle(&self) -> Result<usize, (BackupError, usize)> {
        let mut success_count = 0;
        let mut first_err = None;
        let mut offset = 0;
        loop {
            let states = self
                .egnine
                .list_preserved_source_states(
                    &self.info.uuid,
                    ListPreservedSourceStateFilter {
                        time: (None, None),
                        idle: Some(true),
                    },
                    ListOffset::First(offset),
                    16,
                )
                .await
                .map_err(|e| (e, success_count))?;

            if states.is_empty() {
                return match first_err {
                    Some(err) => Err((err, success_count)),
                    None => Ok(success_count),
                };
            }

            for (state_id, state) in states {
                let original_state = match state {
                    SourceState::None => None,
                    SourceState::Original(original_state) => original_state,
                    SourceState::Preserved(original_state, _) => original_state,
                };

                if let Some(original_state) = &original_state {
                    if let Err(err) = self.source.restore_state(original_state.as_str()).await {
                        if first_err.is_none() {
                            first_err = Some(err);
                        }
                    } else {
                        success_count += 1;
                    }
                }
                self.egnine
                    .delete_source_preserved_state(state_id)
                    .await
                    .map_err(|err| (err, success_count))?; // remove the preserved state.
            }
        }
    }

    async fn list_preserved_source_states(
        &self,
        filter: ListPreservedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>> {
        self.egnine
            .list_preserved_source_states(&self.info.uuid, filter, offset, limit)
            .await
    }
}

#[async_trait::async_trait]
impl Task<CheckPointMetaEngine> for TaskImpl {
    fn uuid(&self) -> &TaskUuid {
        &self.info.uuid
    }
    async fn task_info(&self) -> BackupResult<TaskInfo> {
        Ok(self.info.clone())
    }
    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()> {
        self.egnine.update_task_info(task_info).await
    }
    async fn prepare_checkpoint(
        &self,
        preserved_source_state_id: PreserveStateId,
        is_delta: bool,
    ) -> BackupResult<Arc<dyn CheckPoint<CheckPointMetaEngine>>> {
        let source_preserved = self
            .egnine
            .get_source_preserved(
                self.info.source_id,
                &self.info.uuid,
                preserved_source_state_id,
            )
            .await?;

        let (mut root_dir_meta, prev_checkpoint_version, prev_occupied_size, prev_consume_size) = loop {
            if is_delta {
                let last_finish_checkpoint = self
                    .egnine
                    .list_checkpoints(
                        &self.info.uuid,
                        &ListCheckPointFilter {
                            time: ListCheckPointFilterTime::CompleteTime(None, None),
                            status: Some(vec![CheckPointStatus::Success]),
                        },
                        ListOffset::Last(0),
                        1,
                    )
                    .await?
                    .pop();

                if let Some(prev_checkpoint) = last_finish_checkpoint {
                    let last_target_checkpoint = self
                        .egnine
                        .get_target_checkpoint(
                            self.info.target_id,
                            &self.info.uuid,
                            prev_checkpoint.version(),
                        )
                        .await?;
                    let mut prev_info =
                        CheckPoint::<CheckPointMetaEngine>::info(prev_checkpoint.as_ref()).await?;

                    let prev_occupied_size = prev_info.meta.all_prev_version_occupied_size
                        + prev_info.meta.occupied_size;
                    let prev_consume_size =
                        prev_info.meta.all_prev_version_consume_size + prev_info.meta.consume_size;

                    let prev_version = prev_info.meta.version;
                    prev_info.meta.prev_versions.push(prev_version);

                    let prev_version = prev_info.meta.prev_versions;
                    break (
                        meta_from_delta(last_target_checkpoint.as_ref(), source_preserved.as_ref())
                            .await?,
                        prev_version,
                        prev_occupied_size,
                        prev_consume_size,
                    );
                }
            }

            break (
                meta_from_reader(source_preserved.as_ref()).await?,
                vec![],
                0,
                0,
            );
        };

        let now = SystemTime::now();
        let mut checkpoint_meta = CheckPointMeta {
            task_friendly_name: self.info.friendly_name.clone(),
            task_uuid: self.info.uuid,
            version: CheckPointVersion { time: now, seq: 0 },
            prev_versions: prev_checkpoint_version,
            create_time: now,
            complete_time: now,
            root: StorageItem::Dir(root_dir_meta),
            occupied_size: 0,
            consume_size: 0,
            all_prev_version_occupied_size: prev_occupied_size,
            all_prev_version_consume_size: prev_consume_size,
            service_meta: None,
        };

        let occupied_size = estimate_occupy_size(&checkpoint_meta);
        checkpoint_meta.occupied_size = occupied_size;

        let target_task = self
            .egnine
            .get_target_task(self.info.target_id, &self.info.uuid)
            .await?;
        let consume_size = target_task.estimate_consume_size(&checkpoint_meta).await?;
        checkpoint_meta.consume_size = consume_size;

        self.egnine
            .create_checkpoint(
                &self.info.uuid,
                Some(preserved_source_state_id),
                &mut checkpoint_meta,
            )
            .await
            .map(|cp| cp as Arc<dyn CheckPoint>)
    }
    async fn list_checkpoints(
        &self,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn CheckPoint<CheckPointMetaEngine>>>> {
        let checkpoints = self
            .egnine
            .list_checkpoints(&self.info.uuid, filter, offset, limit)
            .await?;
        let checkpoints = checkpoints
            .into_iter()
            .map(|cp| cp as Arc<dyn CheckPoint>)
            .collect();
        Ok(checkpoints)
    }
    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Arc<dyn CheckPoint<CheckPointMetaEngine>>>> {
        self.egnine
            .get_checkpoint(&self.info.uuid, version)
            .await
            .map(|cp| cp.map(|cp| cp as Arc<dyn CheckPoint>))
    }

    // the checkpoint should be stopped first if it's transfering.
    async fn remove_checkpoint(
        &self,
        version: CheckPointVersion,
        is_remove_on_target: bool,
    ) -> BackupResult<()> {
        unimplemented!()
    }
}

pub(crate) struct TaskWrapper {
    engine: Engine,
    uuid: FindTaskBy,
}

impl TaskWrapper {
    pub fn new(engine: Engine, uuid: TaskUuid) -> TaskWrapper {
        TaskWrapper {
            engine,
            uuid: FindTaskBy::Uuid(uuid),
        }
    }
}

// I don't known why the following code is not working for lifetime issue.
macro_rules! wrap_method {
    ($fn_name:ident, $return_type:ty $(, $arg_name:ident : $arg_type:ty )* ) => {
        async fn $fn_name(&self $(, $arg_name: $arg_type )* ) -> BackupResult<$return_type> {
            let t = self.engine.get_task(self.uuid.as_str()).await?;
            match t {
                Some(t) => t.$fn_name($( $arg_name ),*).await,
                None => Err(BackupError::NotFound(format!(
                    "task({}) has been removed.",
                    self.uuid
                ))),
            }
        }
    };
}

#[async_trait::async_trait]
impl PreserveSourceState for TaskWrapper {
    async fn preserve(&self) -> BackupResult<PreserveStateId> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.preserve().await,
            None => Err(BackupError::NotFound(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.state(state_id).await,
            None => Err(BackupError::NotFound(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn restore(&self, state_id: PreserveStateId) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.restore(state_id).await,
            None => Err(BackupError::NotFound(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn restore_all_idle(&self) -> Result<usize, (BackupError, usize)> {
        let t = self
            .engine
            .get_task(&self.uuid)
            .await
            .map_err(|err| (err, 0))?;
        match t {
            Some(t) => t.restore_all_idle().await,
            None => Err((
                BackupError::NotFound(format!("task({:?}) has been removed.", self.uuid())),
                0,
            )),
        }
    }

    async fn list_preserved_source_states(
        &self,
        filter: ListPreservedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.list_preserved_source_states(filter, offset, limit).await,
            None => Err(BackupError::NotFound(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }
}

#[async_trait::async_trait]
impl Task<CheckPointMetaEngine> for TaskWrapper {
    fn uuid(&self) -> &TaskUuid {
        match &self.uuid {
            FindTaskBy::Uuid(uuid) => uuid,
        }
    }

    async fn task_info(&self) -> BackupResult<TaskInfo> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.task_info().await,
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.update(task_info).await,
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn prepare_checkpoint(
        &self,
        preserved_source_state_id: PreserveStateId,
        is_delta: bool,
    ) -> BackupResult<Arc<dyn CheckPoint<CheckPointMetaEngine>>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => {
                t.prepare_checkpoint(preserved_source_state_id, is_delta)
                    .await
            }
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn list_checkpoints(
        &self,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn CheckPoint<CheckPointMetaEngine>>>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.list_checkpoints(filter, offset, limit).await,
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Arc<dyn CheckPoint<CheckPointMetaEngine>>>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.query_checkpoint(version).await,
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn remove_checkpoint(
        &self,
        version: CheckPointVersion,
        is_remove_on_target: bool,
    ) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.remove_checkpoint(version, is_remove_on_target).await,
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }
}
