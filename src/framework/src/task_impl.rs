use crate::{
    checkpoint::CheckPoint,
    engine::{FindTaskBy, ListOffset, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{CheckPointVersion, PreserveStateId},
    task::{
        HistoryStrategy, ListCheckPointFilter, ListPreservedSourceStateFilter, PreserveSourceState,
        SourceState, Task, TaskInfo,
    },
};

pub(crate) struct TaskImpl {
    info: TaskInfo,
}

impl TaskImpl {
    pub fn new(task_info: TaskInfo) -> Self {
        TaskImpl { info: task_info }
    }
}

#[async_trait::async_trait]
impl PreserveSourceState for TaskImpl {
    async fn preserve(&self) -> BackupResult<PreserveStateId> {
        unimplemented!()
    }

    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState> {
        unimplemented!()
    }

    // Any preserved state for backup by source will be restored automatically when it done(success/fail/cancel).
    // But it should be restored by the application when no transfering start, because the engine is uncertain whether the user will use it to initiate the transfer task.
    // It will fail when a transfer task is valid, you should wait it done or cancel it.
    async fn restore(&self, state_id: PreserveStateId) -> BackupResult<()> {
        unimplemented!()
    }

    async fn restore_all_idle(&self) -> BackupResult<usize> {
        unimplemented!()
    }

    async fn list_preserved_source_states(
        &self,
        filter: ListPreservedSourceStateFilter,
        offset: u32,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl Task for TaskImpl {
    fn uuid(&self) -> &TaskUuid {
        &self.info.uuid
    }
    async fn task_info(&self) -> BackupResult<TaskInfo> {
        unimplemented!()
    }
    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()> {
        unimplemented!()
    }
    async fn history_strategy(&self) -> BackupResult<HistoryStrategy> {
        unimplemented!()
    }
    async fn set_history_strategy(&self, strategy: HistoryStrategy) -> BackupResult<()> {
        unimplemented!()
    }
    async fn prepare_checkpoint(
        &self,
        preserved_source_state_id: PreserveStateId,
        is_delta: bool,
    ) -> BackupResult<Box<dyn CheckPoint>> {
        unimplemented!()
    }
    async fn list_checkpoints(
        &self,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Box<dyn CheckPoint>>> {
        unimplemented!()
    }
    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Box<dyn CheckPoint>>> {
        unimplemented!()
    }
    async fn remove_checkpoint(&self, version: CheckPointVersion) -> BackupResult<()> {
        unimplemented!()
    }
    async fn remove_checkpoints_in_condition(
        &self,
        filter: &ListCheckPointFilter,
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
                None => Err(BackupError::ErrorState(format!(
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
            None => Err(BackupError::ErrorState(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.state(state_id).await,
            None => Err(BackupError::ErrorState(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn restore(&self, state_id: PreserveStateId) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.restore(state_id).await,
            None => Err(BackupError::ErrorState(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn restore_all_idle(&self) -> BackupResult<usize> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.restore_all_idle().await,
            None => Err(BackupError::ErrorState(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn list_preserved_source_states(
        &self,
        filter: ListPreservedSourceStateFilter,
        offset: u32,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.list_preserved_source_states(filter, offset, limit).await,
            None => Err(BackupError::ErrorState(format!(
                "task({:?}) has been removed.",
                self.uuid()
            ))),
        }
    }
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
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.update(task_info).await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn history_strategy(&self) -> BackupResult<HistoryStrategy> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.history_strategy().await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn set_history_strategy(&self, strategy: HistoryStrategy) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.set_history_strategy(strategy).await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn prepare_checkpoint(
        &self,
        preserved_source_state_id: PreserveStateId,
        is_delta: bool,
    ) -> BackupResult<Box<dyn CheckPoint>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => {
                t.prepare_checkpoint(preserved_source_state_id, is_delta)
                    .await
            }
            None => Err(BackupError::ErrorState(format!(
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
    ) -> BackupResult<Vec<Box<dyn CheckPoint>>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.list_checkpoints(filter, offset, limit).await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Box<dyn CheckPoint>>> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.query_checkpoint(version).await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn remove_checkpoint(&self, version: CheckPointVersion) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.remove_checkpoint(version).await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }

    async fn remove_checkpoints_in_condition(
        &self,
        filter: &ListCheckPointFilter,
    ) -> BackupResult<()> {
        let t = self.engine.get_task(&self.uuid).await?;
        match t {
            Some(t) => t.remove_checkpoints_in_condition(filter).await,
            None => Err(BackupError::ErrorState(format!(
                "task({}) has been removed.",
                self.uuid()
            ))),
        }
    }
}
