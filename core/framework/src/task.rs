use std::{sync::Arc, time::SystemTime};

use crate::{
    checkpoint::{CheckPoint, CheckPointStatus},
    engine::{ListOffset, SourceId, TargetId, TaskUuid},
    error::{BackupError, BackupResult},
    meta::{CheckPointVersion, PreserveStateId},
};

pub enum SourceState {
    None,
    Original(Option<String>), // None if nothing for restore.
    Preserved(Option<String>, Option<String>), // <original, preserved>
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub uuid: TaskUuid,
    pub friendly_name: String,
    pub description: String,
    pub source_id: SourceId,
    pub source_param: String, // Any parameters(address .eg) for the source, the source can get it from engine.
    pub target_id: TargetId,
    pub target_param: String, // Any parameters(address .eg) for the target, the target can get it from engine.
    pub priority: u32,
    pub history_strategy: HistoryStrategy,
    pub attachment: String, // The application can save any attachment with task.
    pub flag: u64,          // Save any flags for the task. it will be filterd when list the tasks.
}

pub struct ListPreservedSourceStateFilter {
    pub time: (Option<SystemTime>, Option<SystemTime>),
    pub idle: Option<bool>,
}

#[async_trait::async_trait]
pub trait PreserveSourceState: Send + Sync {
    async fn preserve(&self) -> BackupResult<PreserveStateId>;
    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState>;

    // Any preserved state for backup by source will be restored automatically when it done(success/fail/cancel).
    // But it should be restored by the application when no transfering start, because the engine is uncertain whether the user will use it to initiate the transfer task.
    // It will fail when a transfer task is valid, you should wait it done or cancel it.
    async fn restore(&self, state_id: PreserveStateId) -> BackupResult<()>;
    async fn restore_all_idle(&self) -> Result<usize, (BackupError, usize)>;

    async fn list_preserved_source_states(
        &self,
        filter: ListPreservedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>>;
}

pub enum ListCheckPointFilterTime {
    CreateTime(Option<SystemTime>, Option<SystemTime>), // <begin-time, end-time>
    CompleteTime(Option<SystemTime>, Option<SystemTime>), // <begin-time, end-time>
}

pub struct ListCheckPointFilter {
    pub time: ListCheckPointFilterTime,
    pub status: Option<Vec<CheckPointStatus>>,
}

#[async_trait::async_trait]
pub trait Task<MetaType>: PreserveSourceState + Send + Sync {
    fn uuid(&self) -> &TaskUuid;
    async fn task_info(&self) -> BackupResult<TaskInfo>;
    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()>;
    async fn prepare_checkpoint(
        &self,
        preserved_source_state_id: PreserveStateId,
        is_delta: bool,
    ) -> BackupResult<Arc<dyn CheckPoint<MetaType>>>;
    async fn list_checkpoints(
        &self,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn CheckPoint<MetaType>>>>;
    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Arc<dyn CheckPoint<MetaType>>>>;
    async fn remove_checkpoint(
        &self,
        version: CheckPointVersion,
        is_remove_on_target: bool,
    ) -> BackupResult<()>;
}

#[derive(Debug, Clone)]
pub struct HistoryStrategy {
    pub reserve_history_limit: u32,
    pub continuous_abort_incomplete_limit: u32,
    pub continuous_abort_seconds_limit: u32,
}

impl Default for HistoryStrategy {
    fn default() -> Self {
        HistoryStrategy {
            reserve_history_limit: 1,
            continuous_abort_incomplete_limit: 3,
            continuous_abort_seconds_limit: 3600 * 24 * 7, // 1 week
        }
    }
}
