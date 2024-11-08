use std::{sync::Arc, time::SystemTime};

use crate::{
    checkpoint::{CheckPoint, CheckPointStatus, DeleteFromTarget},
    engine::{ListOffset, SourceId, TargetId, TaskUuid},
    error::BackupResult,
    meta::{CheckPointVersion, LockedSourceStateId},
};

pub enum SourceState {
    None,
    Original,
    Locked,
    ConsumeCheckPoint(CheckPointVersion),
    Unlocked(Option<CheckPointVersion>),
}

pub struct SourceStateInfo {
    pub id: LockedSourceStateId,
    pub state: SourceState,
    pub original: Option<String>,
    pub locked_state: Option<String>,
    pub creator_magic: u64,
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub uuid: TaskUuid,
    pub friendly_name: String,
    pub description: String,
    pub source_id: SourceId,
    pub source_entitiy: String, // Any parameters(address .eg) for the source, the source can get it from engine.
    pub target_id: TargetId,
    pub target_entitiy: String, // Any parameters(address .eg) for the target, the target can get it from engine.
    pub priority: u32,
    pub history_strategy: HistoryStrategy,
    pub attachment: String, // The application can save any attachment with task.
    pub flag: u64,          // Save any flags for the task. it will be filterd when list the tasks.
    pub is_delete: Option<DeleteFromTarget>,
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
pub trait Task: Send + Sync {
    fn uuid(&self) -> &TaskUuid;
    async fn task_info(&self) -> BackupResult<TaskInfo>;
    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()>;

    async fn create_checkpoint(&self, is_delta: bool) -> BackupResult<Arc<dyn CheckPoint>>;

    async fn list_checkpoints(
        &self,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn CheckPoint>>>;

    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Arc<dyn CheckPoint>>>;

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
