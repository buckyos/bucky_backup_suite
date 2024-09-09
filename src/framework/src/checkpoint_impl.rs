use std::collections::HashMap;

use crate::{
    checkpoint::{
        CheckPoint, CheckPointInfo, CheckPointStatus, DirReader, ItemTransferMap, LinkInfo,
        StorageReader,
    },
    engine::TaskUuid,
    engine_impl::Engine,
    error::BackupResult,
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
    async fn read_dir(&self, path: &[u8]) -> BackupResult<Box<dyn DirReader>> {
        unimplemented!()
    }
    async fn read_file(&self, path: &[u8], offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        unimplemented!()
    }
    async fn read_link(&self, path: &[u8]) -> BackupResult<LinkInfo> {
        unimplemented!()
    }
    async fn stat(&self, path: &[u8]) -> BackupResult<StorageItemAttributes> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CheckPoint for CheckPointImpl {
    fn task_uuid(&self) -> &TaskUuid {
        &self.info.meta.task_uuid
    }
    fn version(&self) -> CheckPointVersion {
        self.info.meta.version
    }
    async fn transfer(&self) -> BackupResult<()> {
        unimplemented!()
    }
    async fn stop(&self) -> BackupResult<()> {
        unimplemented!()
    }
    async fn cancel(&self) -> BackupResult<()> {
        unimplemented!()
    }

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>> {
        Ok(self.info.target_meta.clone())
    }

    async fn transfer_map_by_item_path(
        &self,
        paths: Option<Vec<&[u8]>>,
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
    async fn read_dir(&self, path: &[u8]) -> BackupResult<Box<dyn DirReader>> {
        unimplemented!()
    }
    async fn read_file(&self, path: &[u8], offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        unimplemented!()
    }
    async fn read_link(&self, path: &[u8]) -> BackupResult<LinkInfo> {
        unimplemented!()
    }
    async fn stat(&self, path: &[u8]) -> BackupResult<StorageItemAttributes> {
        unimplemented!()
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
    async fn transfer(&self) -> BackupResult<()> {
        unimplemented!()
    }
    async fn stop(&self) -> BackupResult<()> {
        unimplemented!()
    }
    async fn cancel(&self) -> BackupResult<()> {
        unimplemented!()
    }

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>> {
        unimplemented!()
    }

    async fn transfer_map_by_item_path(
        &self,
        paths: Option<Vec<&[u8]>>,
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
