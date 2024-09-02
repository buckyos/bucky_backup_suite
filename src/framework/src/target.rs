use crate::{error::BackupResult, meta::CheckPointMeta};

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
}
