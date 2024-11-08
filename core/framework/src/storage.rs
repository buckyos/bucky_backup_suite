use std::{path::Path, time::SystemTime};

use crate::{
    checkpoint::{CheckPointInfo, CheckPointStatus},
    engine::{
        EngineConfig, FindTaskBy, ListOffset, ListSourceFilter, ListTargetFilter, ListTaskFilter,
        SourceId, SourceInfo, SourceQueryBy, TargetId, TargetInfo, TargetQueryBy, TaskUuid,
    },
    error::BackupResult,
    meta::{
        CheckPointVersion, ChunkId, ChunkInfo, ChunkItem, DefaultFileDiffBlock, FolderItem,
        LockedSourceStateId,
    },
    task::{ListCheckPointFilter, SourceStateInfo, TaskInfo},
};

pub trait Storage:
    StorageSourceMgr
    + StorageTargetMgr
    + StorageTaskMgr
    + StorageLockedSourceStateMgr
    + StorageCheckPointMgr
    + StorageFolderFileMgr
    + StorageChunkMgr
    + StorageConfig
{
}

#[async_trait::async_trait]
pub trait StorageSourceMgr: Send + Sync {
    async fn register(
        &self,
        classify: &str,
        url: &str,
        friendly_name: &str,
        config: &str,
        description: &str,
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
        url: Option<&str>,
        friendly_name: Option<&str>,
        config: Option<&str>,
        description: Option<&str>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageTargetMgr: Send + Sync {
    async fn register(
        &self,
        classify: &str,
        url: &str,
        friendly_name: &str,
        config: &str,
        description: &str,
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
        url: Option<&str>,
        friendly_name: Option<&str>,
        config: Option<&str>,
        description: Option<&str>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageTaskMgr: Send + Sync {
    async fn create_task(&self, task_info: &TaskInfo) -> BackupResult<()>;

    async fn delete_task(&self, by: &FindTaskBy) -> BackupResult<()>;

    async fn list_task(
        &self,
        filter: &ListTaskFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TaskInfo>>;

    async fn query_task(&self, by: &FindTaskBy) -> BackupResult<Option<TaskInfo>>;

    async fn update_task(&self, task_info: &TaskInfo) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageConfig: Send + Sync {
    async fn get_config(&self) -> BackupResult<Option<EngineConfig>>;

    async fn set_config(&self, config: &EngineConfig) -> BackupResult<()>;
}

pub enum SourceState {
    None,
    Original,
    Locked,
    ConsumeCheckPoint,
    Unlocked,
}

pub struct ListLockedSourceStateFilter {
    pub state_id: Option<Vec<LockedSourceStateId>>,
    // <begin-time, end-time>
    pub time: (Option<SystemTime>, Option<SystemTime>),
    pub state: Vec<SourceState>,
}

#[async_trait::async_trait]
pub trait StorageLockedSourceStateMgr: Send + Sync {
    async fn new_state(
        &self,
        task_uuid: &TaskUuid,
        creator_magic: u64,
    ) -> BackupResult<LockedSourceStateId>;

    async fn original_state(
        &self,
        state_id: LockedSourceStateId,
        original_state: Option<&str>,
    ) -> BackupResult<()>;

    async fn locked_state(
        &self,
        state_id: LockedSourceStateId,
        locked_state: Option<&str>,
    ) -> BackupResult<()>;

    async fn state(&self, state_id: LockedSourceStateId) -> BackupResult<SourceStateInfo>;

    async fn list_locked_source_states(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListLockedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<SourceStateInfo>>;

    async fn unlock_source_state(&self, state_id: LockedSourceStateId) -> BackupResult<()>;

    async fn delete_source_state(&self, filter: &ListLockedSourceStateFilter) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageCheckPointMgr: Send + Sync {
    async fn create_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        locked_source_id: Option<LockedSourceStateId>, // It will be lost for `None`
    ) -> BackupResult<CheckPointVersion>;

    async fn delete_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn start_checkpoint_only_once_per_preserved_source(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn update_status(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        status: CheckPointStatus,
        old_status: CheckPointStatus,
    ) -> BackupResult<CheckPointStatus>;

    async fn list_checkpoints(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo>>;

    async fn query_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Option<CheckPointInfo>>;
}

pub type FileSeq = u64;

pub struct ListFolderFileFilter<'a> {
    path: Option<&'a Path>,
    parent: Option<&'a Path>,
    min_seq: Option<FileSeq>,
    max_seq: Option<FileSeq>,
    version: Option<CheckPointVersion>,
}

pub struct ListDefaultDiffBlockFilter {
    min_original_offset: Option<u64>,
    min_new_file_offset: Option<u64>,
    min_diff_file_offset: Option<u64>,
}

#[async_trait::async_trait]
pub trait StorageFolderFileMgr: Send + Sync {
    async fn add_file(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        file_meta: &FolderItem,
    ) -> BackupResult<FileSeq>;

    async fn delete_files(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        filter: ListFolderFileFilter<'_>,
    ) -> BackupResult<u32>;

    async fn add_default_diff_block(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        file_path: &Path,
        diff_block: &DefaultFileDiffBlock,
    ) -> BackupResult<()>;

    async fn list_file(
        &self,
        task_uuid: &TaskUuid,
        filter: ListFolderFileFilter<'_>,
        offset: u32,
        limit: u32,
    ) -> BackupResult<Vec<FolderItem>>;

    async fn list_default_diff_blocks(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        file_path: &Path,
        filter: &ListDefaultDiffBlockFilter,
    ) -> BackupResult<Vec<DefaultFileDiffBlock>>;

    async fn delete_default_diff_blocks(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListDefaultDiffBlockFilter,
    ) -> BackupResult<u32>;
}

pub struct ListChunkFilter {
    chunk_id: Option<ChunkId>,
    version: Option<CheckPointVersion>,
    min_chunk_id: Option<ChunkId>,
    max_chunk_id: Option<ChunkId>,
}

pub struct ListChunkFileFilter<'a> {
    path: Option<&'a Path>,
    chunk_id: Option<ChunkId>,
    verion: Option<CheckPointVersion>,
}

#[async_trait::async_trait]
pub trait StorageChunkMgr: Send + Sync {
    async fn new_chunk(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        compress: Option<&str>,
    ) -> BackupResult<ChunkId>;

    async fn delete_chunks(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListChunkFilter,
    ) -> BackupResult<u32>;

    async fn finish_chunk(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        chunk_id: ChunkId,
        hash: &str,
        size: u64,
    ) -> BackupResult<()>;

    async fn list_chunks(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListChunkFilter,
        offset: u32,
        limit: u32,
    ) -> BackupResult<Vec<ChunkInfo>>;

    async fn add_chunk_file(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        file: &ChunkItem,
    ) -> BackupResult<()>;

    async fn delete_chunk_file(
        &self,
        task_uuid: &TaskUuid,
        filter: ListChunkFileFilter<'_>,
    ) -> BackupResult<u32>;

    async fn list_chunk_file(
        &self,
        task_uuid: &TaskUuid,
        filter: ListChunkFileFilter<'_>,
        offset: u32,
        limit: u32,
    ) -> BackupResult<Vec<ChunkItem>>;
}

#[async_trait::async_trait]
pub trait StorageTransaction: Send + Sync {
    async fn start_transaction(&self) -> BackupResult<()>;
    async fn commit_transaction(&self) -> BackupResult<()>;
}
