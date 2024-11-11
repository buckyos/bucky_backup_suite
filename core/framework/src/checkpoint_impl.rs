use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    path::{Display, Path, PathBuf},
    sync::Arc,
    time::SystemTime,
    u64,
};

use sha2::digest::typenum::Length;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};

use crate::{
    checkpoint::{
        CheckPoint, CheckPointInfo, CheckPointStatus, ChunkReader, DirChildType, DirReader,
        FolderReader, ItemEnumerate, ItemId, ItemTransferProgress, LinkInfo, PendingAttribute,
        PendingStatus, PrepareProgress, TransferProgress,
    },
    engine::{FindTaskBy, SourceId, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{Attributes, CheckPointVersion, FileDiffHeader},
    source::{LockedSource, SourceStatus},
    source_wrapper::LockedSourceWrapper,
    status_waiter::{self, Waiter},
    target::TargetStatus,
    task::Task,
    task_impl::TaskWrapper,
};

pub(crate) struct CheckPointImpl {
    task_uuid: TaskUuid,
    version: CheckPointVersion,
    info: Arc<RwLock<CheckPointInfo>>,
    engine: Engine,
    source: LockedSourceWrapper,
    status: Arc<Mutex<crate::status_waiter::Status<CheckPointStatus>>>,
    status_waiter: crate::status_waiter::Waiter<CheckPointStatus>,
    source_status: crate::status_waiter::Status<SourceStatus>,
    source_status_waiter: crate::status_waiter::Waiter<SourceStatus>,
    target_status: crate::status_waiter::Status<TargetStatus>,
    target_status_waiter: crate::status_waiter::Waiter<TargetStatus>,
}

impl std::fmt::Display for CheckPointImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.task_uuid, self.version)
    }
}

impl CheckPointImpl {
    pub(crate) fn new(mut info: CheckPointInfo, source_id: SourceId, engine: Engine) -> Self {
        let task_uuid = info.task_uuid;
        let version = info.version;
        let locked_state_id = info.locked_source_state_id;

        let pending_static = |pending_status: &PendingStatus| -> PendingStatus {
            match pending_status {
                PendingStatus::None | PendingStatus::Pending | PendingStatus::Started => {
                    PendingStatus::None
                }
                PendingStatus::Done | PendingStatus::Failed(_) => pending_status.clone(),
            }
        };

        let source_status = match &info.source_status {
            SourceStatus::StandBy
            | SourceStatus::Finish
            | SourceStatus::Stopped
            | SourceStatus::Failed(_)
            | SourceStatus::Delete => info.source_status,
            SourceStatus::Scaning | SourceStatus::Stoping | SourceStatus::Deleting => {
                SourceStatus::Stopped
            }
        };

        let target_status = match &info.target_status {
            TargetStatus::StandBy
            | TargetStatus::Finish
            | TargetStatus::Stopped
            | TargetStatus::Failed(_)
            | TargetStatus::Delete => info.source_status,
            TargetStatus::Transfering | TargetStatus::Stoping | TargetStatus::Deleting => {
                TargetStatus::Stopped
            }
        };

        let checkpoint_status = match &info.status {
            CheckPointStatus::Standby
            | CheckPointStatus::StopPrepare
            | CheckPointStatus::Success
            | CheckPointStatus::Stop
            | CheckPointStatus::Failed(_)
            | CheckPointStatus::DeleteAbort(_) => info.status,
            CheckPointStatus::Prepare => {
                if let SourceStatus::Finish = source_status {
                    info.status
                } else {
                    CheckPointStatus::StopPrepare
                }
            }
            CheckPointStatus::Start => CheckPointStatus::Stop,
            CheckPointStatus::Delete(delete_from_target) => {
                CheckPointStatus::DeleteAbort(delete_from_target)
            }
        };

        info.status = checkpoint_status;
        info.source_status = source_status;
        info.target_status = target_status;

        let (status, waiter) = crate::status_waiter::StatusWaiter::new(checkpoint_status);
        let (source_status, source_waiter) = crate::status_waiter::StatusWaiter::new(source_status);
        let (target_status, target_waiter) = crate::status_waiter::StatusWaiter::new(target_status);

        Self {
            info: Arc::new(RwLock::new(info)),
            engine: engine.clone(),
            task_uuid,
            version,
            source: LockedSourceWrapper::new(source_id, task_uuid, locked_state_id, engine),
            status,
            status_waiter: Arc::new(Mutex::new(waiter)),
            source_status,
            source_status_waiter: source_waiter,
            target_status: target_status,
            target_status_waiter: target_waiter,
        }
    }

    pub(crate) fn info(&self) -> CheckPointInfo {
        let info = self.info.clone();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move { info.read().await.clone() })
    }

    pub(crate) async fn prepare_without_check_status(&self) -> BackupResult<()> {
        match self.source.prepare().await {
            Ok(_) => {
                // wait
                let checkpoint = self.clone();
                tokio::task::spawn(async move {
                    let waiter = checkpoint.source.status_waiter().await?;
                    let wait_prepare_start = async move {
                        loop {
                            let source_status = waiter
                                .wait(|s| s != crate::source::SourceStatus::StandBy)
                                .await?;
                            let status = match source_status {
                                SourceStatus::StandBy => {
                                    continue;
                                }
                                _ => source_status,
                            };
                            break Ok(status);
                        }
                    };

                    loop {
                        let source_status = match wait_prepare_start.await {
                            Ok(status) => status,
                            Err(err) => PendingStatus::Failed(Some(err)),
                        };

                        let new_checkpoint_status = {
                            let checkpoint_status = self.status.lock().await;
                            self.source_status.set(source_status);

                            if let SourceStatus::Failed(err) = source_status {
                                checkpoint_status.set(CheckPointStatus::Failed(err.clone()));
                                Some(CheckPointStatus::Failed(err))
                            } else {
                                None
                            }
                        };

                        checkpoint
                            .engine
                            .update_checkpoint_status(
                                &checkpoint.task_uuid,
                                checkpoint.version,
                                source_status,
                                None,
                            )
                            .await;

                        if let Some(new_checkpoint_status) = new_checkpoint_status {
                            self.engine
                                .update_checkpoint_status(
                                    &checkpoint.task_uuid,
                                    checkpoint.version,
                                    new_checkpoint_status,
                                    None,
                                )
                                .await;
                        }
                    }
                });

                Ok(())
            }
            Err(err) => {
                let checkpoint_status = self.status.lock().await;
                self.source_status
                    .set(SourceStatus::Failed(Some(err.clone())));
                checkpoint_status.set(CheckPointStatus::Failed(Some(err.clone())));

                self.engine
                    .update_checkpoint_source_status(
                        &self.task_uuid,
                        self.version,
                        SourceStatus::Failed(Some(err.clone())),
                        None,
                    )
                    .await;
                self.engine
                    .update_checkpoint_status(
                        &self.task_uuid,
                        self.version,
                        CheckPointStatus::Failed(Some(err.clone())),
                        None,
                    )
                    .await;

                Err(err)
            }
        }
    }

    pub(crate) async fn transfer_impl(&self) -> BackupResult<()> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl DirReader for CheckPointImpl {
    async fn read_dir(&self, path: &Path) -> BackupResult<Vec<DirChildType>> {
        unimplemented!()
    }
    async fn file_size(&self, path: &Path) -> BackupResult<u64> {
        unimplemented!()
    }
    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        unimplemented!()
    }
    async fn copy_file_to(&self, path: &Path, to: &Path) -> BackupResult<Waiter<PendingStatus>> {
        unimplemented!()
    }
    async fn file_diff_header(&self, path: &Path) -> BackupResult<FileDiffHeader> {
        unimplemented!()
    }
    async fn read_file_diff(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        unimplemented!()
    }
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo> {
        unimplemented!()
    }
    async fn stat(&self, path: &Path) -> BackupResult<Attributes> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CheckPoint for CheckPointImpl {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }
    fn version(&self) -> CheckPointVersion {
        self.version
    }
    async fn info(&self) -> BackupResult<CheckPointInfo> {
        Ok(self.info.read().await.clone())
    }

    async fn prepare(&self) -> BackupResult<()> {
        let update_status_with_check = async move {
            let checkpoint_status = self.status.lock().await;
            let source_status = self.source_status.get_status();
            let check_result = match checkpoint_status.get_status() {
                CheckPointStatus::Standby => Ok(true),
                CheckPointStatus::Prepare | CheckPointStatus::Start => {
                    Ok(if let SourceStatus::Failed(_) = source_status {
                        true
                    } else {
                        false
                    })
                }
                CheckPointStatus::StopPrepare
                | CheckPointStatus::Stop
                | CheckPointStatus::Failed(_) => match source_status {
                    SourceStatus::Stopped | SourceStatus::Failed(_) => Ok(true),
                    SourceStatus::StandBy | SourceStatus::Scaning | SourceStatus::Stoping => {
                        log::warn!("the checkpoint({}) is busy for a `stop` is pending.", self);
                        Err(BackupError::ErrorState(
                            "the checkpoint is busy for a `stop` is pending".to_owned(),
                        ))
                    }
                    SourceStatus::Finish => Ok(false),
                    SourceStatus::Delete | SourceStatus::Deleting => {
                        unreachable!()
                    }
                },
                CheckPointStatus::Success => Ok(false),
                CheckPointStatus::Delete(_) | CheckPointStatus::DeleteAbort(_) => {
                    log::warn!(
                        "the checkpoint({}) is delete, you should not do anything.",
                        self
                    );
                    Err(BackupError::ErrorState(
                        "the checkpoint is delete, you should not do anything".to_owned(),
                    ))
                }
            };

            match check_result? {
                true => {
                    checkpoint_status.set(CheckPointStatus::Prepare);
                    self.source_status.set(SourceStatus::StandBy);
                    Ok(true)
                }
                false => Ok(false),
            }
        };

        if !update_status_with_check? {
            return Ok(());
        }

        self.engine
            .update_checkpoint_status(
                &self.task_uuid,
                self.version,
                CheckPointStatus::Prepare,
                None,
            )
            .await;

        self.prepare_without_check_status().await
    }

    async fn prepare_progress(&self) -> BackupResult<PendingAttribute<PrepareProgress>> {
        unimplemented!()
    }

    async fn transfer(&self) -> BackupResult<()> {
        unimplemented!()
    }

    async fn transfer_progress(&self) -> BackupResult<PendingAttribute<TransferProgress>> {
        unimplemented!()
    }

    async fn item_transfer_progress(
        &self,
        id: &ItemId,
    ) -> BackupResult<PendingAttribute<Vec<ItemTransferProgress>>> {
        unimplemented!()
    }

    async fn stop(&self) -> BackupResult<()> {
        unimplemented!()
        // let info = self.info.read().await.clone();
        // let task = self
        //     .engine
        //     .get_task_impl(&FindTaskBy::Uuid(info.meta.task_uuid))
        //     .await?
        //     .map_or(
        //         Err(BackupError::NotFound(format!(
        //             "task({}) has been removed.",
        //             self.task_uuid
        //         ))),
        //         |task| Ok(task),
        //     )?;

        // match info.status {
        //     CheckPointStatus::Success => {
        //         return Err(BackupError::ErrorState(format!(
        //             "the checkpoint({}-{:?}) has successed.",
        //             self.task_uuid, self.version
        //         )))
        //     }
        //     CheckPointStatus::Transfering => {
        //         let target_checkpoint = self
        //             .engine
        //             .get_target_checkpoint_impl(
        //                 task.info().target_id,
        //                 self.task_uuid(),
        //                 self.version(),
        //             )
        //             .await?;
        //         target_checkpoint.stop().await?;
        //     }
        //     CheckPointStatus::Standby => {
        //         return Err(BackupError::ErrorState(format!(
        //             "the checkpoint({}-{:?}) has not started.",
        //             self.task_uuid, self.version
        //         )));
        //     }
        //     _ => {}
        // }

        // self.engine
        //     .update_checkpoint_status(self.task_uuid(), self.version(), CheckPointStatus::Stop)
        //     .await?;
        // self.info.write().await.status = CheckPointStatus::Stop;
        // Ok(())
    }

    async fn enumerate_or_generate_chunk(
        &self,
        capacities: &[u64],
    ) -> BackupResult<futures::stream::Iter<Box<dyn ChunkReader>>> {
        unimplemented!()
    }

    async fn enumerate_or_generate_folder(
        &self,
    ) -> BackupResult<futures::stream::Iter<FolderReader>> {
        unimplemented!()
    }

    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate> {
        unimplemented!()
    }

    async fn status(&self) -> BackupResult<CheckPointStatus> {
        Ok(self.info.read().await.status.clone())
    }

    async fn status_waiter(&self) -> BackupResult<Waiter<CheckPointStatus>> {
        unimplemented!()
    }
}

pub(crate) struct CheckPointWrapper {
    task_uuid: TaskUuid,
    version: CheckPointVersion,
    engine: Engine,
}

impl CheckPointWrapper {
    pub(crate) fn new(task_uuid: TaskUuid, version: CheckPointVersion, engine: Engine) -> Self {
        Self {
            task_uuid,
            version,
            engine,
        }
    }
}

#[async_trait::async_trait]
impl DirReader for CheckPointWrapper {
    async fn read_dir(&self, path: &Path) -> BackupResult<Vec<DirChildType>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_dir(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn file_size(&self, path: &Path) -> BackupResult<u64> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.file_size(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_file(path, offset, length).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn copy_file_to(&self, path: &Path, to: &Path) -> BackupResult<Waiter<PendingStatus>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.copy_file_to(path, to).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
    async fn file_diff_header(&self, path: &Path) -> BackupResult<FileDiffHeader> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.file_diff_header(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
    async fn read_file_diff(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_file_diff(path, offset, length).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_link(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn stat(&self, path: &Path) -> BackupResult<Attributes> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.stat(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
}

#[async_trait::async_trait]
impl CheckPoint for CheckPointWrapper {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }

    fn version(&self) -> CheckPointVersion {
        self.version
    }

    async fn info(&self) -> BackupResult<CheckPointInfo> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.info.read().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn prepare(&self) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.prepare().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
    async fn prepare_progress(&self) -> BackupResult<PendingAttribute<PrepareProgress>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.prepare_progress().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer(&self) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.transfer().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer_progress(&self) -> BackupResult<PendingAttribute<TransferProgress>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.transfer_progress().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn item_transfer_progress(
        &self,
        id: &ItemId,
    ) -> BackupResult<PendingAttribute<Vec<ItemTransferProgress>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.item_transfer_progress(id).await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn stop(&self) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.stop().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn enumerate_or_generate_chunk(
        &self,
        capacities: &[u64],
    ) -> BackupResult<futures::stream::Iter<Box<dyn ChunkReader>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.enumerate_or_generate_chunk(capacities).await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn enumerate_or_generate_folder(
        &self,
    ) -> BackupResult<futures::stream::Iter<FolderReader>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.enumerate_or_generate_folder().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn enumerate_item(&self) -> BackupResult<ItemEnumerate> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.enumerate_item().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn status(&self) -> BackupResult<CheckPointStatus> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.status().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn status_waiter(&self) -> BackupResult<Waiter<CheckPointStatus>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.status_waiter().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
}
