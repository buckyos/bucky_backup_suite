use crate::{error::BackupResult, task::Task};

pub trait SourceMgr {
    async fn register(
        &self,
        classify: String,
        url: String,
        description: String,
    ) -> BackupResult<SourceId>;

    async fn unregister(&self, by: SourceId) -> BackupResult<()>;

    async fn list(
        &self,
        classify: Option<String>,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<SourceInfo>>;

    async fn query_by(&self, by: SourceQueryBy) -> BackupResult<Option<SourceInfo>>;

    async fn update(
        &self,
        by: SourceQueryBy,
        url: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()>;
}

pub trait TargetMgr {
    async fn register(
        &self,
        classify: String,
        url: String,
        description: String,
    ) -> BackupResult<TargetId>;

    async fn unregister(&self, by: TargetQueryBy) -> BackupResult<()>;

    async fn list(
        &self,
        classify: Option<String>,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TargetInfo>>;

    async fn query_by(&self, by: TargetQueryBy) -> BackupResult<Option<TargetInfo>>;

    async fn update(
        &self,
        by: TargetQueryBy,
        url: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()>;
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
        attachment: String,   // The application can save any attachment with task.
    ) -> BackupResult<Box<dyn Task>>;

    async fn remove_task(&self, by: FindTaskBy) -> BackupResult<()>;

    async fn list_task(
        &self,
        by: ListTaskBy,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Box<dyn Task>>>;

    async fn find_task(&self, by: FindTaskBy) -> BackupResult<Box<dyn Task>>;
}

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

pub struct SourceInfo {
    pub id: SourceId,
    pub classify: String,
    pub url: String,
    pub description: String,
}

pub enum SourceQueryBy {
    Id(SourceId),
    Url(String),
}

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

pub struct TargetInfo {
    pub id: SourceId,
    pub classify: String,
    pub url: String,
    pub description: String,
}

pub enum TargetQueryBy {
    Id(TargetId),
    Url(String),
}

pub enum ListOffset {
    First(u64),
    Last(u64),
}

pub struct TaskId(u64);

impl Into<u64> for TaskId {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<u64> for TaskId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

pub enum ListTaskBy {
    All,
    SourceId(SourceId),
    TargetId(TargetId),
}

pub enum FindTaskBy {
    TaskId(TaskId),
}
