use std::time::SystemTime;

use crate::{
    checkpoint::{CheckPoint, CheckPointStatus},
    engine::{ListOffset, SourceId, TargetId},
    error::BackupResult,
    meta::{CheckPointMetaEngine, CheckPointVersion, PreserveStateId},
};

pub enum SourceState {
    None,
    Original(Option<String>), // None if nothing for restore.
    Preserved((Option<String>, Option<String>)), // <original, preserved>
}

pub struct TaskInfo {
    pub uuid: String,
    pub friendly_name: String,
    pub description: String,
    pub source_id: SourceId,
    pub source_param: String, // Any parameters(address .eg) for the source, the source can get it from engine.
    pub target_id: String,
    pub target_param: String, // Any parameters(address .eg) for the target, the target can get it from engine.
    pub attachment: String,   // The application can save any attachment with task.
}

pub struct ListPreservedSourceStateFilter {
    time: (Option<SystemTime>, Option<SystemTime>),
    idle: Option<bool>,
}

#[async_trait::async_trait]
pub trait PreserveSourceState {
    async fn preserve(&self) -> BackupResult<PreserveStateId>;
    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState>;

    // Any preserved state for backup by source will be restored automatically when it done(success/fail/cancel).
    // But it should be restored by the application when no transfering start, because the engine is uncertain whether the user will use it to initiate the transfer task.
    // It will fail when a transfer task is valid, you should wait it done or cancel it.
    async fn restore(&self, state_id: PreserveStateId) -> BackupResult<()>;
    async fn restore_all_idle(&self) -> BackupResult<usize>;

    async fn list_preserved_source_states(
        &self,
        filter: ListPreservedSourceStateFilter,
        offset: u32,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>>;
}

pub enum ListCheckPointFilterTime {
    CreateTime((Option<SystemTime>, Option<SystemTime>)), // <begin-time, end-time>
    CompleteTime((Option<SystemTime>, Option<SystemTime>)), // <begin-time, end-time>
}

pub struct ListCheckPointFilter {
    pub time: ListCheckPointFilterTime,
    pub status: Option<Vec<CheckPointStatus>>,
}

#[async_trait::async_trait]
pub trait Task: PreserveSourceState {
    async fn task_info(&self) -> TaskInfo;
    async fn update(&self, task_info: &TaskInfo) -> BackupResult<()>;
    async fn prepare_checkpoint(
        &self,
        preserved_source_state_id: PreserveStateId,
    ) -> BackupResult<Box<dyn CheckPoint>>;
    async fn list_checkpoints(
        &self,
        filter: ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Box<dyn CheckPoint>>>;
    async fn query_checkpoint(
        &self,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Box<dyn CheckPoint>>>;
    async fn remove_checkpoint(&self, version: CheckPointVersion) -> BackupResult<()>;
    async fn remove_checkpoints_in_condition(
        &self,
        filter: ListCheckPointFilter,
    ) -> BackupResult<()>;
}
