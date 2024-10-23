use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

use base58::ToBase58;
use sha2::{Digest, Sha256};

use crate::{
    engine::TaskUuid,
    error::{BackupError, BackupResult},
    meta::{CheckPointVersion, PreserveStateId, StorageItemAttributes},
};

#[derive(Clone)]
pub enum CheckPointStatus {
    Standby,
    Start,
    Transfering,
    Stop,
    Success,
    Failed(Option<BackupError>),
}

#[derive(Clone)]
pub struct CheckPointInfo<MetaType> {
    pub meta: MetaType,
    pub target_meta: Option<Vec<String>>,
    pub preserved_source_state_id: Option<PreserveStateId>,
    pub status: CheckPointStatus,
    pub last_status_changed_time: SystemTime,
}

#[async_trait::async_trait]
pub trait CheckPoint<MetaType>: StorageReader {
    fn task_uuid(&self) -> &TaskUuid;
    fn version(&self) -> CheckPointVersion;
    async fn info(&self) -> BackupResult<CheckPointInfo<MetaType>>;
    // if is_delta: SUM(prev-checkpoints[])
    // otherwise: self.info().meta
    async fn full_meta(&self) -> BackupResult<MetaType>;
    async fn transfer(&self) -> BackupResult<()>;
    async fn stop(&self) -> BackupResult<()>;

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>>;

    async fn transfer_map_by_item_path(
        &self,
        paths: Option<Vec<&Path>>,
    ) -> BackupResult<HashMap<PathBuf, HashMap<Vec<u8>, Vec<ChunkTransferInfo>>>>; // <item-path, target-address, ItemTransferInfo>

    async fn transfer_map_to_target_address(
        &self,
        target_addresses: Option<Vec<&[u8]>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<PathBuf, Vec<ChunkTransferInfo>>>>; // <target-address, <item-path, ItemTransferInfo>>

    async fn get_all_transfer_target_address(&self) -> BackupResult<Vec<Vec<u8>>>;

    async fn status(&self) -> BackupResult<CheckPointStatus>;
}

pub struct PrepareTransferChunkResult {
    pub prepared_chunk_id: u64,
    pub target_address: Vec<u8>,
    pub info: ChunkTransferInfo,
}

// The targets will call these interfaces to notify the new status.
#[async_trait::async_trait]
pub trait CheckPointObserver {
    async fn on_success(&self) -> BackupResult<()>;
    async fn on_failed(&self, err: BackupError) -> BackupResult<()>;
    async fn on_prepare_transfer_chunk(
        &self,
        item_full_path: &Path,
        offset: u64,
        length: u64,
        target_address: Option<&[u8]>, // specific target address defined by target
        detail: Option<&[u8]>,
    ) -> BackupResult<PrepareTransferChunkResult>;
    async fn on_item_transfer_done(
        &self,
        prepared_chunk_id: u64,
        target_address: Option<&[u8]>, // specific target address defined by target
        detail: Option<&[u8]>,
    ) -> BackupResult<()>;
    /*
       Save values for key to avoid some status loss.

       examples:
       let value = checkpoint.get_key_value("my-key").await?;
       let value = match value {
           Some(v) => {
               v
           }
           None => {
               let value = generate_value();
               checkpoint.save_key_value("my-key", &value, true).await?;
               value
           }
       };

       // We can reuse the `value` next time for the `consume_value` failed or crashed.
       // Otherwise, we may not know if the value is consumed success or not when the `consume_value` timeout or crashed.
       let result = consume_value(value).await?;
    */
    async fn save_key_value(&self, key: &str, value: &[u8], is_replace: bool) -> BackupResult<()>;
    async fn get_key_value(&self, key: &str) -> BackupResult<Option<Vec<u8>>>;
    async fn delete_key_value(&self, key: &str) -> BackupResult<()>;
}

pub enum DirChildType {
    File(PathBuf),
    Dir(PathBuf),
    Link(PathBuf),
}

impl DirChildType {
    pub fn path(&self) -> &Path {
        match self {
            DirChildType::File(path) => path,
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

#[derive(Clone)]
pub struct ChunkTransferInfo {
    pub prepared_chunk_id: u64,
    pub begin_time: SystemTime,
    pub finish_time: Option<SystemTime>,
    pub offset: u64,
    pub length: u64,
    pub detail: Option<Vec<u8>>, // special parse for different target.
}

#[async_trait::async_trait]
pub trait StorageReader: Send + Sync {
    async fn read_dir(&self, path: &Path) -> BackupResult<Vec<DirChildType>>;
    async fn file_size(&self, path: &Path) -> BackupResult<u64>;
    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo>;
    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes>;
}

pub struct FileStreamReader<'a> {
    reader: &'a dyn StorageReader,
    path: &'a Path,
    pos: u64,
    chunk_size: u32,
}

impl<'a> FileStreamReader<'a> {
    pub fn new(reader: &'a dyn StorageReader, path: &'a Path, pos: u64, chunk_size: u32) -> Self {
        Self {
            reader,
            pos,
            chunk_size,
            path,
        }
    }

    pub fn pos(&self) -> u64 {
        self.pos
    }

    pub async fn file_size(&self) -> BackupResult<u64> {
        self.reader.file_size(self.path).await
    }

    pub async fn read_next(&mut self) -> BackupResult<Vec<u8>> {
        let data = self
            .reader
            .read_file(&self.path, self.pos, self.chunk_size)
            .await?;

        self.pos += data.len() as u64;
        Ok(data)
    }

    pub async fn hash(&mut self) -> BackupResult<String> {
        let mut hasher = Sha256::new();
        loop {
            let chunk = self.read_next().await?;
            if chunk.is_empty() {
                break;
            }
            hasher.update(chunk);
        }
        let hash = hasher.finalize();
        let hash = hash.as_slice().to_base58();
        Ok(hash)
    }
}
