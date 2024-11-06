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
    meta::{
        CheckPointVersion, FileDiffHeader, FolderItemAttributes, LockedSourceStateId,
        StorageItemAttributes,
    },
};

#[derive(Clone)]
pub struct CheckPointInfo {
    pub task_uuid: String,
    pub task_friendly_name: String,
    pub version: CheckPointVersion,
    pub prev_versions: Vec<CheckPointVersion>, // all versions this checkpoint depends on
    pub complete_time: Option<SystemTime>,
    pub locked_source_state_id: Option<LockedSourceStateId>,
    pub status: CheckPointStatus,
    pub last_status_changed_time: SystemTime,
}

pub enum PendingStatus {
    // operation has been post
    Pending,
    // A time-consuming operation has already started
    Started,
    // operation has done
    Done,
    // operation is failed
    Failed(Option<BackupError>),
}

pub type SourcePendingStatus = PendingStatus;
pub type TargetPendingStatus = PendingStatus;

pub enum DeleteFromTarget {
    Reserve,
    Delete,
}

#[derive(Clone)]
pub enum CheckPointStatus {
    Standby,
    Prepare(SourcePendingStatus),
    Start(SourcePendingStatus, TargetPendingStatus),
    Stop(SourcePendingStatus, TargetPendingStatus),
    Success,
    Failed(Option<BackupError>),
    Delete(SourcePendingStatus, TargetPendingStatus, DeleteFromTarget),
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
pub trait CheckPoint<MetaType>: DirReader {
    fn task_uuid(&self) -> &TaskUuid;
    fn version(&self) -> CheckPointVersion;

    async fn prepare(&self) -> BackupResult<MetaType>;
    async fn prepare_progress(&self) -> BackupResult<PendingAttribute<PrepareProgress>>;

    async fn transfer(&self, is_compress: bool) -> BackupResult<()>;
    async fn transfer_progress(&self) -> BackupResult<PendingAttribute<TransferProgress>>;
    async fn item_transfer_progress(
        &self,
        id: &ItemId,
    ) -> BackupResult<PendingAttribute<Vec<ItemTransferProgress>>>;

    async fn stop(&self) -> BackupResult<()>;

    async fn enumerate_or_generate_chunk(
        &self,
        capacities: &[u64],
    ) -> BackupResult<futures::stream::Iter<dyn ChunkReader>>;

    async fn enumerate_or_generate_folder(
        &self,
    ) -> BackupResult<futures::stream::Iter<FolderReader>>;

    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate>;

    async fn status(&self) -> BackupResult<CheckPointStatus>;
    async fn wait_status<F>(&self) -> BackupResult<StatusWaitor<CheckPointStatus>>;
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
    async fn copy_file_to(
        &self,
        path: &Path,
        to: &Path,
    ) -> BackupResult<StatusWaitor<PendingStatus>>;
    async fn file_diff_header(&self, path: &Path) -> BackupResult<FileDiffHeader>;
    async fn read_file_diff(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo>;
    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes>;
}

pub trait FileContentReader: Send + Sync {
    async fn read(&self, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn copy_to(&self, to: &Path) -> BackupResult<StatusWaitor<PendingStatus>>;
}

pub trait FileDiffContentReader: Send + Sync {
    async fn file_size(&self) -> BackupResult<u64>;
    async fn read(&self, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn copy_to(&self, to: &Path) -> BackupResult<StatusWaitor<PendingStatus>>;
}

pub enum FolderItemContentReader {
    Dir(Box<dyn DirReader>),
    File(Box<dyn FileContentReader>),
    FileDiff(Box<dyn FileDiffContentReader>),
    Link(LinkInfo),
}

pub struct FolderReader {
    size: u64,
    attr: FolderItemAttributes,
    path: PathBuf,
    reader: Option<FolderItemContentReader>,
}

// pub struct FileStreamReader<'a> {
//     reader: &'a dyn FolderReader,
//     path: &'a Path,
//     pos: u64,
//     chunk_size: u32,
// }

// impl<'a> FileStreamReader<'a> {
//     pub fn new(reader: &'a dyn FolderReader, path: &'a Path, pos: u64, chunk_size: u32) -> Self {
//         Self {
//             reader,
//             pos,
//             chunk_size,
//             path,
//         }
//     }

//     pub fn pos(&self) -> u64 {
//         self.pos
//     }

//     pub async fn file_size(&self) -> BackupResult<u64> {
//         self.reader.file_size(self.path).await
//     }

//     pub async fn read_next(&mut self) -> BackupResult<Vec<u8>> {
//         let data = self
//             .reader
//             .read_file(&self.path, self.pos, self.chunk_size)
//             .await?;

//         self.pos += data.len() as u64;
//         Ok(data)
//     }

//     pub async fn hash(&mut self) -> BackupResult<String> {
//         let mut hasher = Sha256::new();
//         loop {
//             let chunk = self.read_next().await?;
//             if chunk.is_empty() {
//                 break;
//             }
//             hasher.update(chunk);
//         }
//         let hash = hasher.finalize();
//         let hash = hash.as_slice().to_base58();
//         Ok(hash)
//     }
// }

#[async_trait::async_trait]
pub trait ChunkReader: Send + Sync {
    async fn capacity(&self) -> BackupResult<u64>;
    async fn len(&self) -> BackupResult<u64>;
    async fn read(&self, offset: u64, length_limit: u32) -> BackupResult<Vec<u8>>;
    async fn copy_file_to(&self, to: &Path) -> BackupResult<StatusWaitor<PendingStatus>>;
}

pub enum ItemEnumerate {
    Folder(futures::stream::Iter<FolderReader>),
    Chunk(futures::stream::Iter<Box<dyn ChunkReader>>),
}
