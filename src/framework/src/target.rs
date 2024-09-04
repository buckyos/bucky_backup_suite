use crate::{
    checkpoint::{DirReader, LinkInfo},
    error::BackupResult,
    meta::{CheckPointMeta, CheckPointVersion, StorageItemAttributes},
    task::TaskInfo,
};

#[async_trait::async_trait]
pub trait TargetFactory<
    ServiceCheckPointMeta,
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
>
{
    async fn from_task(
        task_info: TaskInfo,
    ) -> BackupResult<
        Box<
            dyn Target<
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
pub trait Target<
    ServiceCheckPointMeta,
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
>
{
    async fn fill_target_meta(
        &self,
        meta: &mut CheckPointMeta<
            ServiceCheckPointMeta,
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    ) -> BackupResult<Vec<String>>;

    async fn transfer(
        &self,
        meta: &CheckPointMeta<
            ServiceCheckPointMeta,
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
        target_meta: &[String],
    ) -> BackupResult<()>;

    async fn update_config(&self, config: &str) -> BackupResult<()>;

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
