use crate::{error::BackupResult, meta::CheckPointMeta, task::TaskInfo};

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
}
