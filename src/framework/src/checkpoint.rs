use crate::{
    error::{BackupError, BackupResult},
    meta::{PreserveStateId, StorageItemAttributes},
};

pub enum CheckPointStatus {
    Standby,
    Start,
    Stop,
    Success,
    Failed(BackupError),
}

pub struct CheckPointInfo<MetaType> {
    pub meta: MetaType,
    pub target_meta: Option<Vec<String>>,
    pub preserved_source_state_id: PreserveStateId,
    pub status: CheckPointStatus,
}

#[async_trait::async_trait]
pub trait CheckPoint {
    async fn restore(&self) -> BackupResult<()>;

    async fn read_dir(&self, path: &[u8]) -> BackupResult<Box<dyn DirReader>>;
    async fn read_file(&self, path: &[u8], offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, path: &[u8]) -> BackupResult<LinkInfo>;
    async fn stat(&self, path: &[u8]) -> BackupResult<StorageItemAttributes>;
    async fn target_meta(&self) -> BackupResult<Option<Vec<u8>>>;

    async fn status(&self) -> BackupResult<CheckPointStatus>;
}

pub enum DirChildType {
    File(Vec<u8>),
    Dir(Vec<u8>),
    Link(Vec<u8>),
}

impl DirChildType {
    pub fn path(&self) -> &[u8] {
        match self {
            DirChildType::File(path) => path,
            DirChildType::Dir(path) => path,
            DirChildType::Link(path) => path,
        }
    }
}

#[async_trait::async_trait]
pub trait DirReader {
    fn path(&self) -> &[u8];
    fn next(&mut self) -> BackupResult<Option<DirChildType>>;
}

pub struct LinkInfo {
    pub target: Vec<u8>,
    pub is_hard: bool,
}
