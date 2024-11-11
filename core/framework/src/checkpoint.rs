use std::{
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    time::SystemTime,
};

use base58::ToBase58;
use sha2::{Digest, Sha256};

use crate::{
    engine::TaskUuid,
    error::{BackupError, BackupResult},
    meta::{Attributes, CheckPointVersion, ChunkId, FileDiffHeader, LockedSourceStateId},
    source::SourceStatus,
    status_waiter::Waiter,
    target::TargetStatus,
};

#[derive(Clone)]
pub struct CheckPointInfo {
    pub task_uuid: TaskUuid,
    pub task_friendly_name: String,
    pub version: CheckPointVersion,
    pub prev_version: Option<CheckPointVersion>,
    pub complete_time: Option<SystemTime>,
    pub locked_source_state_id: Option<LockedSourceStateId>,
    pub status: CheckPointStatus,
    pub source_status: SourceStatus,
    pub target_status: TargetStatus,
    pub last_status_changed_time: SystemTime,
}

#[derive(Clone)]
pub enum PendingStatus {
    // the operation should been post, but it was not been post for any reason.
    // for example: the progress restart
    None,
    // operation has been post
    Pending,
    // A time-consuming operation has already started
    Started,
    // operation has done
    Done,
    // operation is failed
    Failed(Option<BackupError>),
}

pub type IsSourcePrepareDone = bool;

#[derive(Clone, Copy)]
pub enum DeleteFromTarget {
    Reserve,
    Delete,
}

#[derive(Clone)]
pub enum CheckPointStatus {
    Standby,
    Prepare,
    StopPrepare,
    Start,
    Stop,
    Success,
    Failed(Option<BackupError>),
    Delete(DeleteFromTarget),
    DeleteAbort(DeleteFromTarget), // abort for unknown reason
}

pub enum PendingAttribute<A> {
    Pending(A),
    Done(A),
}

pub struct PrepareProgress {
    total_size: u64,
    item_count: u64,
}

pub struct TransferProgress {
    transfer_size: u64,
    transfer_item_count: u64,
    consume_size: u64,
}

pub enum ItemId {
    Chunk(ChunkId),
    Folder(DirChildType),
}

pub struct ItemTransferProgress {
    to: ItemId,
    source_range: Range<u64>,
    target_range: Range<u64>,
}

#[async_trait::async_trait]
pub trait CheckPoint: DirReader {
    fn task_uuid(&self) -> &TaskUuid;
    fn version(&self) -> CheckPointVersion;
    async fn info(&self) -> BackupResult<CheckPointInfo>;

    async fn prepare(&self) -> BackupResult<()>;
    async fn prepare_progress(&self) -> BackupResult<PendingAttribute<PrepareProgress>>;

    async fn transfer(&self) -> BackupResult<()>;
    async fn transfer_progress(&self) -> BackupResult<PendingAttribute<TransferProgress>>;
    async fn item_transfer_progress(
        &self,
        id: &ItemId,
    ) -> BackupResult<PendingAttribute<Vec<ItemTransferProgress>>>;

    async fn stop(&self) -> BackupResult<()>;

    async fn enumerate_or_generate_chunk(
        &self,
        capacities: &[u64],
    ) -> BackupResult<futures::stream::Iter<Box<dyn ChunkReader>>>;

    async fn enumerate_or_generate_folder(
        &self,
    ) -> BackupResult<futures::stream::Iter<FolderReader>>;

    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate>;

    async fn status(&self) -> BackupResult<CheckPointStatus>;
    async fn status_waiter(&self) -> BackupResult<Waiter<CheckPointStatus>>;
}

pub enum DirChildType {
    File(PathBuf),
    FileDiff(PathBuf),
    Dir(PathBuf),
    Link(PathBuf),
}

impl DirChildType {
    pub fn path(&self) -> &Path {
        match self {
            DirChildType::File(path) => path,
            DirChildType::FileDiff(path) => path,
            DirChildType::Dir(path) => path,
            DirChildType::Link(path) => path,
        }
    }
}

#[derive(Clone)]
pub struct LinkInfo {
    pub target: PathBuf,
    pub is_hard: bool,
}

#[async_trait::async_trait]
pub trait DirReader: Send + Sync {
    async fn read_dir(&self, path: &Path) -> BackupResult<Vec<DirChildType>>;
    async fn file_size(&self, path: &Path) -> BackupResult<u64>;
    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn copy_file_to(&self, path: &Path, to: &Path) -> BackupResult<Waiter<PendingStatus>>;
    async fn file_diff_header(&self, path: &Path) -> BackupResult<FileDiffHeader>;
    async fn read_file_diff(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo>;
    async fn stat(&self, path: &Path) -> BackupResult<Attributes>;
}

#[async_trait::async_trait]
pub trait FileContentReader: Send + Sync {
    async fn read(&self, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn copy_to(&self, to: &Path) -> BackupResult<Waiter<PendingStatus>>;
}

#[async_trait::async_trait]
pub trait FileDiffContentReader: Send + Sync {
    async fn file_size(&self) -> BackupResult<u64>;
    async fn read(&self, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn copy_to(&self, to: &Path) -> BackupResult<Waiter<PendingStatus>>;
}

pub enum FolderItemContentReader {
    Dir(Box<dyn DirReader>),
    File(Box<dyn FileContentReader>),
    FileDiff(Box<dyn FileDiffContentReader>),
    Link(LinkInfo),
}

pub struct FolderReader {
    size: u64,
    attr: Attributes,
    path: PathBuf,
    reader: Option<FolderItemContentReader>,
}

#[async_trait::async_trait]
pub trait ChunkReader: Send + Sync {
    async fn capacity(&self) -> BackupResult<u64>;
    async fn len(&self) -> BackupResult<u64>;
    async fn read(&self, offset: u64, length_limit: u32) -> BackupResult<Vec<u8>>;
    async fn copy_file_to(&self, to: &Path) -> BackupResult<Waiter<PendingStatus>>;
}

pub enum ItemEnumerate {
    Folder(futures::stream::Iter<FolderReader>),
    Chunk(futures::stream::Iter<Box<dyn ChunkReader>>),
}
