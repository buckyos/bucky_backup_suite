use crate::{
    checkpoint::CheckPoint,
    engine::ListOffset,
    engine_impl::Engine,
    error::BackupResult,
    meta::{CheckPointVersion, PreserveStateId},
    task::{
        HistoryStrategy, ListCheckPointFilter, ListPreservedSourceStateFilter, PreserveSourceState,
        SourceState, Task, TaskInfo,
    },
};

pub(crate) struct TaskImpl {}

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
    uuid: String,
}

impl TaskWrapper {
    pub fn new(engine: Engine, uuid: String) -> TaskWrapper {
        TaskWrapper { engine, uuid }
    }
}

#[async_trait::async_trait]
impl PreserveSourceState for TaskWrapper {
    async fn preserve(&self) -> BackupResult<PreserveStateId> {
        let ti = self.engine.get_task(self.uuid.as_str())?;
        match ti {
            Some(ti) => ti.preserve().await,
            None => Err(BackupError::ErrorState(format!("task({}) has removed.", self.uuid))),
        }
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
impl Task for TaskWrapper {
    async fn task_info(&self) -> BackupResult<TaskInfo> {
        let t = self.engine.get_task(self.uuid.as_str())?;
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
