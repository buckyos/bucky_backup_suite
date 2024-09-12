use std::{collections::HashMap, path::Path};

use crate::{
    checkpoint::{
        CheckPoint, CheckPointInfo, CheckPointStatus, DirReader, ItemTransferMap, LinkInfo,
        StorageReader,
    },
    engine::TaskUuid,
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{CheckPointMetaEngine, CheckPointVersion, StorageItemAttributes},
};

pub(crate) struct CheckPointImpl {
    info: CheckPointInfo<CheckPointMetaEngine>,
    engine: Engine,
}

impl CheckPointImpl {
    pub(crate) fn new(info: CheckPointInfo<CheckPointMetaEngine>, engine: Engine) -> Self {
        Self { info, engine }
    }

    pub(crate) fn info(&self) -> &CheckPointInfo<CheckPointMetaEngine> {
        &self.info
    }
}

#[async_trait::async_trait]
impl StorageReader for CheckPointImpl {
    async fn read_dir(&self, path: &Path) -> BackupResult<Box<dyn DirReader>> {
        unimplemented!()
    }
    async fn file_size(&self, path: &Path) -> BackupResult<u64> {
        unimplemented!()
    }
    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        unimplemented!()
    }
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo> {
        unimplemented!()
    }
    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CheckPoint<CheckPointMetaEngine> for CheckPointImpl {
    fn task_uuid(&self) -> &TaskUuid {
        &self.info.meta.task_uuid
    }
    fn version(&self) -> CheckPointVersion {
        self.info.meta.version
    }
    async fn info(&self) -> BackupResult<CheckPointInfo<CheckPointMetaEngine>> {
        Ok(self.info.clone())
    }
    async fn full_meta(&self) -> BackupResult<CheckPointMetaEngine> {
        unimplemented!()
    }
    async fn transfer(&self) -> BackupResult<()> {
        unimplemented!()
    }
    async fn stop(&self) -> BackupResult<()> {
        unimplemented!()
    }

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>> {
        Ok(self.info.target_meta.clone())
    }

    async fn transfer_map_by_item_path(
        &self,
        paths: Option<Vec<&Path>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>> // <item-path, target-address, ItemTransferInfo>
    {
        unimplemented!()
    }

    async fn transfer_map_to_target_address(
        &self,
        target_addresses: Option<Vec<&str>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>> // <target-address, <item-path, ItemTransferInfo>>
    {
        unimplemented!()
    }

    async fn get_all_transfer_target_address(&self) -> BackupResult<Vec<Vec<u8>>> {
        unimplemented!()
    }

    async fn status(&self) -> BackupResult<CheckPointStatus> {
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
impl StorageReader for CheckPointWrapper {
    async fn read_dir(&self, path: &Path) -> BackupResult<Box<dyn DirReader>> {
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

    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes> {
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
impl CheckPoint<CheckPointMetaEngine> for CheckPointWrapper {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }
    fn version(&self) -> CheckPointVersion {
        self.version
    }
    async fn info(&self) -> BackupResult<CheckPointInfo<CheckPointMetaEngine>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.info.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
    async fn full_meta(&self) -> BackupResult<CheckPointMetaEngine> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.full_meta().await?),
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

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.target_meta().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer_map_by_item_path(
        &self,
        paths: Option<Vec<&Path>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>> // <item-path, target-address, ItemTransferInfo>
    {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.transfer_map_by_item_path(paths).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer_map_to_target_address(
        &self,
        target_addresses: Option<Vec<&str>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>> // <target-address, <item-path, ItemTransferInfo>>
    {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.transfer_map_to_target_address(target_addresses).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn get_all_transfer_target_address(&self) -> BackupResult<Vec<Vec<u8>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.get_all_transfer_target_address().await,
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
}
