use std::sync::Arc;

use crate::{
    error::BackupResult,
    task::{HistoryStrategy, Task},
};

#[async_trait::async_trait]
pub trait SourceMgr {
    async fn register(
        &self,
        classify: String,
        url: String,
        friendly_name: String,
        config: String,
        description: String,
    ) -> BackupResult<SourceId>;

    async fn unregister(&self, by: &SourceQueryBy) -> BackupResult<()>;

    async fn list(
        &self,
        filter: &ListSourceFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<SourceInfo>>;

    async fn query_by(&self, by: &SourceQueryBy) -> BackupResult<Option<SourceInfo>>;

    async fn update(
        &self,
        by: &SourceQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait TargetMgr {
    async fn register(
        &self,
        classify: String,
        url: String,
        friendly_name: String,
        config: String,
        description: String,
    ) -> BackupResult<TargetId>;

    async fn unregister(&self, by: &TargetQueryBy) -> BackupResult<()>;

    async fn list(
        &self,
        filter: &ListTargetFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TargetInfo>>;

    async fn query_by(&self, by: &TargetQueryBy) -> BackupResult<Option<TargetInfo>>;

    async fn update(
        &self,
        by: &TargetQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait Config {
    async fn get_config(&self) -> BackupResult<EngineConfig>;
    async fn set_config(&self, config: EngineConfig) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait TaskMgr {
    async fn create_task(
        &self,
        friendly_name: String,
        description: String,
        source_id: SourceId,
        source_param: String, // Any parameters(address .eg) for the source, the source can get it from engine.
        target_id: String,
        target_param: String, // Any parameters(address .eg) for the target, the target can get it from engine.
        history_strategy: HistoryStrategy,
        priority: u32,
        attachment: String, // The application can save any attachment with task.
        flag: u64,          // Save any flags for the task. it will be filterd when list the tasks.
    ) -> BackupResult<Arc<dyn Task>>;

    async fn remove_task(&self, by: &FindTaskBy) -> BackupResult<()>;

    async fn list_task(
        &self,
        filter: &ListTaskFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn Task>>>;

    async fn find_task(&self, by: &FindTaskBy) -> BackupResult<Option<Arc<dyn Task>>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(u64);

impl Into<u64> for SourceId {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<u64> for SourceId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone)]
pub struct SourceInfo {
    pub id: SourceId,
    pub classify: String,
    pub friendly_name: String,
    pub url: String,
    pub config: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ListSourceFilter {
    pub classify: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SourceQueryBy {
    Id(SourceId),
    Url(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetId(u64);

impl Into<u64> for TargetId {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<u64> for TargetId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone)]
pub struct TargetInfo {
    pub id: TargetId,
    pub classify: String,
    pub friendly_name: String,
    pub url: String,
    pub config: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ListTargetFilter {
    pub classify: Option<String>,
}

#[derive(Debug, Clone)]
pub enum TargetQueryBy {
    Id(TargetId),
    Url(String),
}

#[derive(Debug, Clone)]
pub enum ListOffset {
    First(u64),
    Last(u64),
}

pub struct ListTaskFilter {
    pub source_id: Option<Vec<SourceId>>,
    pub target_id: Option<Vec<TargetId>>,
    pub flag: Option<Vec<u64>>,
}

pub enum FindTaskBy {
    TaskUuid(String),
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub transfering_task_limit: u32, // max count of the tasks transfering, they will be push in a queue if there are more tasks.
    pub timeout_secs: u32, // if there is no transfering progress in this time, the task will be pause, and other tasks will be scheduled.
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            transfering_task_limit: 4,
            timeout_secs: 16,
        }
    }
}
