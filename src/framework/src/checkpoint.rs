use std::{collections::HashMap, time::SystemTime};

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
    pub last_status_changed_time: SystemTime,
}

#[async_trait::async_trait]
pub trait CheckPoint {
    async fn transfer(&self) -> BackupResult<()>;
    async fn stop(&self) -> BackupResult<()>;
    async fn cancel(&self) -> BackupResult<()>;

    async fn read_dir(&self, path: &[u8]) -> BackupResult<Box<dyn DirReader>>;
    async fn read_file(&self, path: &[u8], offset: u64, length: u32) -> BackupResult<Vec<u8>>;
    async fn read_link(&self, path: &[u8]) -> BackupResult<LinkInfo>;
    async fn stat(&self, path: &[u8]) -> BackupResult<StorageItemAttributes>;
    async fn target_meta(&self) -> BackupResult<Option<Vec<u8>>>;

    async fn transfer_map_by_item_path(
        &self,
        paths: Option<Vec<&[u8]>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>>; // <item-path, target-address, ItemTransferInfo>

    async fn transfer_map_to_target_address(
        &self,
        target_addresses: Option<Vec<&str>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>>; // <target-address, <item-path, ItemTransferInfo>>

    async fn get_all_transfer_target_address(&self) -> BackupResult<Vec<Vec<u8>>>;

    async fn status(&self) -> BackupResult<CheckPointStatus>;
}

// The targets will call these interfaces to notify the new status.
#[async_trait::async_trait]
pub trait CheckPointObserver {
    async fn on_success(&self) -> BackupResult<()>;
    async fn on_stop(&self) -> BackupResult<()>;
    async fn on_failed(&self, err: BackupError) -> BackupResult<()>;
    async fn on_pre_transfer_item(
        &self,
        item_path: &[u8],
        offset: u64,
        length: u64,
        target_address: Option<&[u8]>, // specific target address
        detail: Option<&[u8]>,
    ) -> BackupResult<()>;
    async fn on_item_transfer_done(
        &self,
        item_path: &[u8],
        offset: u64,
        length: u64,
        target_address: Option<&[u8]>, // specific target address
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

pub struct ItemTransferMap {
    pub begin_time: SystemTime,
    pub finish_time: Option<SystemTime>,
    pub offset: u64,
    pub length: u64,
    pub detail: Option<Vec<u8>>, // special parse for different target.
}
