use std::{sync::Arc, time::SystemTime};

use crate::{
    checkpoint::{CheckPoint, CheckPointStatus, DeleteFromTarget},
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
                log::warn!("a checkpoint({:?}) is running", version);
                Err(BackupError::ErrorState(
                    "a checkpoint is running".to_owned(),
                ))
            }
            None => match another_lock_selected {
                Some(state_id) => {
                    log::warn!("another lock({:?}) is running", state_id);
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
        let locked_state = match org_state.as_ref() {
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

    async fn create_checkpoint(
        &self,
        is_delta: bool,
        is_compress: bool,
    ) -> BackupResult<Arc<dyn CheckPoint>> {
        let locked_state = self.lock_source().await?;

        let prev_checkpoint_version = if is_delta {
            self.egnine
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
                .pop()
                .map(|prev_checkpoint| prev_checkpoint.version())
        } else {
            None
        };

        self.egnine
            .create_checkpoint(
                &self.info.uuid,
                Some(locked_state.id),
                prev_checkpoint_version,
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
        is_delete_on_target: DeleteFromTarget,
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

    async fn create_checkpoint(
        &self,
        is_delta: bool,
        is_compress: bool,
    ) -> BackupResult<Arc<dyn CheckPoint>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.create_checkpoint(is_delta, is_compress).await,
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
        is_delete_on_target: DeleteFromTarget,
    ) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.remove_checkpoint(version, is_delete_on_target).await,
            None => Err(BackupError::NotFound(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }
}
