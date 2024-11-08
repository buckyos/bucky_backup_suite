use std::{sync::Arc, time::SystemTime};

use crate::{
    checkpoint::{CheckPoint, CheckPointStatus},
    engine::{FindTaskBy, ListOffset, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{CheckPointVersion, LockedSourceStateId},
    source::SourceTask,
    source_wrapper::SourceTaskWrapper,
    storage::{self, ListLockedSourceStateFilter},
    target::TargetTask,
    task::{
        ListCheckPointFilter, ListCheckPointFilterTime, SourceState, SourceStateInfo, Task,
        TaskInfo,
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

    pub fn info(&self) -> &TaskInfo {
        &self.info
    }

    async fn create_source_state_atomic(&self) -> BackupResult<LockedSourceStateId> {
        // atomic
        let new_locked_state_id = self.egnine.new_source_state(&self.info.uuid).await?;

        let cur_magic = self.egnine.process_magic();
        let mut running_checkpoint = None;
        let mut invalid_states = vec![];
        let mut concurrency_states = vec![];

        while running_checkpoint.is_none() {
            const LIST_LIMIT: u32 = 16;
            let locked_states = self
                .egnine
                .list_locked_source_states(
                    &self.info.uuid,
                    &ListLockedSourceStateFilter {
                        state_id: None,
                        time: (None, None),
                        state: vec![
                            storage::SourceState::None,
                            storage::SourceState::Original,
                            storage::SourceState::Locked,
                            storage::SourceState::ConsumeCheckPoint,
                        ],
                    },
                    ListOffset::First(0),
                    LIST_LIMIT,
                )
                .await?;

            let locked_state_count = locked_states.len() as u32;
            for state_info in locked_states {
                if state_info.id == new_locked_state_id {
                    continue;
                }

                match state_info.state {
                    crate::task::SourceState::None
                    | crate::task::SourceState::Original
                    | crate::task::SourceState::Locked => {
                        if state_info.creator_magic == cur_magic {
                            concurrency_states.push(state_info.id);
                        } else {
                            invalid_states.push(state_info.id);
                        }
                    }
                    crate::task::SourceState::ConsumeCheckPoint(version) => {
                        running_checkpoint = Some(version);
                    }
                    crate::task::SourceState::Unlocked(_) => continue,
                }
            }

            if locked_state_count < LIST_LIMIT {
                break;
            }
        }

        let mut another_lock_selected = None;
        if running_checkpoint.is_some() {
            invalid_states.extend(concurrency_states);
            invalid_states.push(new_locked_state_id);
        } else {
            let select_state_id_pos = concurrency_states
                .iter()
                .position(|s| *s < new_locked_state_id);
            if let Some(select_pos) = select_state_id_pos.as_ref() {
                another_lock_selected = concurrency_states.get(*select_pos).cloned();
                concurrency_states.splice(*select_pos..*select_pos + 1, [new_locked_state_id]);
            }

            invalid_states.extend(concurrency_states);
        }

        if invalid_states.len() > 0 {
            self.egnine
                .delete_source_locked_state(&ListLockedSourceStateFilter {
                    state_id: Some(invalid_states),
                    time: (None, None),
                    state: vec![],
                })
                .await?;
        }

        match running_checkpoint {
            Some(version) => {
                log::warn!("a checkpoint({}) is running", version);
                Err(BackupError::ErrorState(
                    "a checkpoint is running".to_owned(),
                ))
            }
            None => match another_lock_selected {
                Some(state_id) => {
                    log::warn!("another lock({}) is running", state_id);
                    Err(BackupError::ErrorState(
                        "another lock is running".to_owned(),
                    ))
                }
                None => Ok(new_locked_state_id),
            },
        }
    }
    async fn lock_source(&self) -> BackupResult<SourceStateInfo> {
        let state_id = self.create_source_state_atomic().await?;

        let org_state = self.source.original_state().await?;
        self.egnine
            .save_source_original_state(state_id, org_state.as_deref())
            .await?;
        let locked_state = match org_state {
            Some(org_state) => self.source.lock_state(org_state.as_str()).await?,
            None => None,
        };
        self.egnine
            .save_source_locked_state(state_id, locked_state.as_deref())
            .await?;

        Ok(SourceStateInfo {
            id: state_id,
            state: SourceState::Locked,
            original: org_state,
            locked_state,
            creator_magic: self.egnine.process_magic(),
        })
    }
}

#[async_trait::async_trait]
impl Task for TaskImpl {
    fn uuid(&self) -> &TaskUuid {
        &self.info.uuid
    }

    async fn task_info(&self) -> BackupResult<TaskInfo> {
        Ok(self.info.clone())
    }

    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()> {
        self.egnine.update_task_info(task_info).await
    }

    async fn create_checkpoint(&self, is_delta: bool) -> BackupResult<Arc<dyn CheckPoint>> {
        let locked_state = self.lock_source().await?;

        let (root_dir_meta, prev_checkpoint_version, prev_occupied_size, prev_consume_size) = loop {
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
                    let mut prev_meta = CheckPoint::full_meta(prev_checkpoint.as_ref()).await?;
                    let prev_version = prev_meta.version;

                    let last_target_checkpoint = self
                        .egnine
                        .get_target_checkpoint(self.info.target_id, &self.info.uuid, prev_version)
                        .await?;

                    let prev_occupied_size =
                        prev_meta.all_prev_version_occupied_size + prev_meta.occupied_size;
                    let prev_consume_size =
                        prev_meta.all_prev_version_consume_size + prev_meta.consume_size;

                    prev_meta.prev_versions.push(prev_version);

                    let prev_version = prev_meta.prev_versions;
                    break (
                        DirectoryMetaEngine::delta_from_reader(
                            &prev_meta.root,
                            prev_checkpoint.as_ref(),
                            source_preserved.as_ref(),
                        )
                        .await?,
                        prev_version,
                        prev_occupied_size,
                        prev_consume_size,
                    );
                }
            }

            break (
                DirectoryMetaEngine::from_reader(source_preserved.as_ref()).await?,
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
            root: StorageItem::Dir(root_dir_meta, vec![]),
            occupied_size: 0,
            consume_size: 0,
            all_prev_version_occupied_size: prev_occupied_size,
            all_prev_version_consume_size: prev_consume_size,
            service_meta: None,
        };

        let occupied_size = checkpoint_meta.estimate_occupy_size();
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
    ) -> BackupResult<Vec<Arc<dyn CheckPoint>>> {
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
    ) -> BackupResult<Option<Arc<dyn CheckPoint>>> {
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
impl Task for TaskWrapper {
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

    async fn create_checkpoint(&self, is_delta: bool) -> BackupResult<Arc<dyn CheckPoint>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.create_checkpoint(is_delta).await,
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
    ) -> BackupResult<Vec<Arc<dyn CheckPoint>>> {
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
    ) -> BackupResult<Option<Arc<dyn CheckPoint>>> {
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
