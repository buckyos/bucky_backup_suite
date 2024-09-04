use crate::{
    checkpoint::{DirReader, LinkInfo},
    error::BackupResult,
    meta::{CheckPointVersion, StorageItemAttributes},
    task::TaskInfo,
};

#[async_trait::async_trait]
pub trait SourceFactory {
    async fn from_task(task_info: TaskInfo) -> BackupResult<Box<dyn Source>>;
}

#[async_trait::async_trait]
pub trait Source {
    // preserve
    async fn original_state(&self) -> BackupResult<Option<String>>;
    async fn preserved_state(&self) -> BackupResult<Option<String>>;
    async fn restore_state(&self, original_state: String) -> BackupResult<()>;

    async fn update_config(&self, config: &str) -> BackupResult<()>;

    // for checkpoint
    async fn read_dir(
        &self,
        version: CheckPointVersion,
        path: &[u8],
    ) -> BackupResult<Box<dyn DirReader>>;
    async fn read_file(
        &self,
        version: CheckPointVersion,
        path: &[u8],
        offset: u64,
        length: u32,
    ) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, version: CheckPointVersion, path: &[u8]) -> BackupResult<LinkInfo>;
    async fn stat(
        &self,
        version: CheckPointVersion,
        path: &[u8],
    ) -> BackupResult<StorageItemAttributes>;
}
