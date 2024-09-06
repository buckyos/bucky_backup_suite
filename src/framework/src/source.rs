use crate::{
    checkpoint::{DirReader, LinkInfo},
    engine::{SourceId, SourceInfo, TaskUuid},
    error::BackupResult,
    meta::{CheckPointMeta, CheckPointVersion, StorageItemAttributes},
    task::TaskInfo,
};

#[async_trait::async_trait]
pub trait SourceFactory<
    ServiceCheckPointMeta,
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
>: Send + Sync
{
    async fn from_source_info(
        &self,
        source_info: SourceInfo,
    ) -> BackupResult<
        Box<
            dyn Source<
                ServiceCheckPointMeta,
                ServiceDirMetaType,
                ServiceFileMetaType,
                ServiceLinkMetaType,
                ServiceLogMetaType,
            >,
        >,
    >;
}

#[async_trait::async_trait]
pub trait Source<
    ServiceCheckPointMeta,
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
>: Send + Sync
{
    fn source_id(&self) -> SourceId;
    async fn source_info(&self) -> BackupResult<SourceInfo>;
    async fn source_task(
        &self,
        task_info: TaskInfo,
    ) -> BackupResult<
        Box<
            dyn SourceTask<
                ServiceCheckPointMeta,
                ServiceDirMetaType,
                ServiceFileMetaType,
                ServiceLinkMetaType,
                ServiceLogMetaType,
            >,
        >,
    >;

    async fn update_config(&self, config: &str) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait SourceTask<
    ServiceCheckPointMeta,
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
>: Send + Sync
{
    fn task_uuid(&self) -> &TaskUuid;
    // preserve
    async fn original_state(&self) -> BackupResult<Option<String>>;
    async fn preserved_state(&self) -> BackupResult<Option<String>>;
    async fn restore_state(&self, original_state: &str) -> BackupResult<()>;

    async fn source_checkpoint(
        &self,
        meta: CheckPointMeta<
            ServiceCheckPointMeta,
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    ) -> BackupResult<Box<dyn SourceCheckPoint>>;
}

#[async_trait::async_trait]
pub trait SourceCheckPoint: Send + Sync {
    fn checkpoint_version(&self) -> CheckPointVersion;
    // for checkpoint
    async fn read_dir(&self, path: &[u8]) -> BackupResult<Box<dyn DirReader>>;
    async fn read_file(&self, path: &[u8], offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, version: CheckPointVersion, path: &[u8]) -> BackupResult<LinkInfo>;
    async fn stat(&self, path: &[u8]) -> BackupResult<StorageItemAttributes>;
}

pub trait SourceFactoryEngine: SourceFactory<String, String, String, String, String> {}
impl<T: SourceFactory<String, String, String, String, String>> SourceFactoryEngine for T {}

pub trait SourceEngine: Source<String, String, String, String, String> {}
impl<T: Source<String, String, String, String, String>> SourceEngine for T {}

pub trait SourceTaskEngine: SourceTask<String, String, String, String, String> {}
impl<T: SourceTask<String, String, String, String, String>> SourceTaskEngine for T {}
