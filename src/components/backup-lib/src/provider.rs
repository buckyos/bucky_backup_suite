use crate::{BackupCheckpoint, BackupChunkItem, BackupResult, RemoteBackupCheckPointItemStatus};
use async_trait::async_trait;
use ndn_lib::{ChunkId, ChunkReadSeek, ChunkReader, ChunkWriter, NdnProgressCallback, ObjId};
use rusqlite::types::{FromSql, ToSql, ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::{future::Future, pin::Pin};
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreConfig {
    pub restore_location_url: String,
    pub is_clean_restore: bool, // 为true时,恢复后只包含恢复的文件,不包含其他文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

pub struct BackupConfig {
    pub crypto_config: String,
}

impl ToSql for RestoreConfig {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = serde_json::to_string(self)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(s.into())
    }
}

impl FromSql for RestoreConfig {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str().unwrap();
        let config: RestoreConfig = serde_json::from_str(s)
            .map_err(|e| rusqlite::types::FromSqlError::Other(Box::new(e)))?;
        Ok(config)
    }
}

pub const ABILITY_LOCAL: &str = "local";
pub const ABILITY_CHUNK_LIST: &str = "chunk_list";

#[async_trait]
pub trait IBackupChunkSourceProvider {
    //return json string?
    async fn get_source_info(&self) -> BackupResult<Value>;
    fn get_source_url(&self) -> String;
    fn is_local(&self) -> bool;
    fn is_support(&self, ability: &str) -> bool;
    //async fn lock_for_backup(&self,source_url: &str)->BackupResult<()>;
    //async fn unlock_for_backup(&self,source_url: &str)->BackupResult<()>;
    //async fn create_checkpoint(&self, checkpoint_id: &str)->BackupResult<BackupCheckpoint>;
    async fn prepare_items(
        &self,
        checkpoint_id: &str,
        callback: Option<Arc<Mutex<NdnProgressCallback>>>,
    ) -> BackupResult<(Vec<BackupChunkItem>, u64, bool)>;
    //async fn open_item(&self, item_id: &str)->BackupResult<Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>>;
    async fn open_item_chunk_reader(
        &self,
        checkpoint_id: &str,
        backup_item: &BackupChunkItem,
        offset: u64,
    ) -> BackupResult<ChunkReader>;
    async fn open_chunk_reader(&self, chunk_id: &ChunkId, offset: u64)
        -> BackupResult<ChunkReader>;
    //async fn on_item_backuped(&self, item_id: &str)->BackupResult<()>;
    //restore
    async fn add_checkpoint(&self, checkpoint: &BackupCheckpoint) -> BackupResult<()>;
    async fn init_for_restore(
        &self,
        restore_config: &RestoreConfig,
        checkpoint_id: &str,
    ) -> BackupResult<String>;
    async fn open_writer_for_restore(
        &self,
        restore_target_id: &str,
        item: &BackupChunkItem,
        restore_config: &RestoreConfig,
        offset: u64,
    ) -> BackupResult<(ChunkWriter, u64)>;
}

//TODO ChunkTarget目前只依赖Chunk和Chunklist的语义，是否需要理解CheckPoint的概念?
#[async_trait]
pub trait IBackupChunkTargetProvider {
    async fn get_target_info(&self) -> BackupResult<String>;
    fn get_target_url(&self) -> String;
    async fn get_account_session_info(&self) -> BackupResult<String>;
    async fn set_account_session_info(&self, session_info: &str) -> BackupResult<()>;
    //fn get_max_chunk_size(&self)->Result<u64>;
    //返回Target上已经存在的Checkpoint列表()
    //async fn get_checkpoint_list(&self)->Result<Vec<String>>;

    //下面的接口将要成为通用的http based的chunk操作接口
    //async fn get_support_chunkid_types(&self)->Result<Vec<String>>;
    async fn alloc_checkpoint(&self, checkpoint: &BackupCheckpoint) -> BackupResult<()>;
    async fn add_backup_item(
        &self,
        checkpoint_id: &str,
        backup_items: &Vec<BackupChunkItem>,
    ) -> BackupResult<()>;
    async fn query_check_point_state(
        &self,
        checkpoint_id: &str,
    ) -> BackupResult<(BackupCheckpoint, RemoteBackupCheckPointItemStatus)>;
    //async fn query_not_complete_backup_items(&self, checkpoint_id: &str)->BackupResult<Vec<BackupChunkItem>>;
    async fn remove_checkpoint(&self, checkpoint_id: &str) -> BackupResult<()>;

    //async fn is_chunk_exist(&self, chunk_id: &ChunkId)->BackupResult<(bool,u64)>;
    async fn open_chunk_writer(
        &self,
        checkpoint_id: &str,
        chunk_id: &ChunkId,
        chunk_size: u64,
    ) -> BackupResult<(ChunkWriter, u64)>;
    async fn complete_chunk_writer(
        &self,
        checkpoint_id: &str,
        chunk_id: &ChunkId,
    ) -> BackupResult<()>;

    // restore
    async fn open_chunk_reader_for_restore(
        &self,
        chunk_id: &ChunkId,
        offset: u64,
    ) -> BackupResult<ChunkReader>;
}

//不需要定义dir source/target ? 完全基于named data mgr来管理？
// #[async_trait]
// pub trait IBackupDirSourceProvider {
//     async fn get_source_info(&self) -> BackupResult<Value>;
//     async fn list_dirs(&self)->BackupResult<Vec<String>>;
//     //return url for backup target
//     async fn prepare_dir_for_backup(&self, dir_id: &str,backup_config:&BackupConfig)->BackupResult<()>;
//     async fn start_restore_dir(&self, dir_id: &str,dir_obj_id:&ObjId,restore_config:&RestoreConfig)->BackupResult<()>;
//     async fn query_restore_task_info(&self, dir_obj_id: &ObjId)->BackupResult<String>;
// }
#[async_trait]
pub trait IBackupDirTargetProvider {
    async fn get_target_info(&self) -> BackupResult<String>;
    async fn list_dirs(&self) -> BackupResult<Vec<String>>;
    async fn prepare_dir_for_restore(
        &self,
        dir_id: &str,
        restore_config: &RestoreConfig,
    ) -> BackupResult<()>;

    async fn start_backup_dir(&self, dir_id: &str, dir_obj_id: &ObjId) -> BackupResult<()>;
    async fn query_backup_task_info(&self, dir_obj_id: &ObjId) -> BackupResult<String>;
}

pub type BackupChunkSourceProvider = Box<dyn IBackupChunkSourceProvider + Send + Sync>;
pub type BackupChunkTargetProvider = Box<dyn IBackupChunkTargetProvider + Send + Sync>;
//pub type BackupDirSourceProvider = Box<dyn IBackupDirSourceProvider + Send + Sync>;
pub type BackupDirTargetProvider = Box<dyn IBackupDirTargetProvider + Send + Sync>;

pub type BackupChunkSourceCreateFunc = Box<
    dyn FnMut(
            String,
        ) -> Pin<
            Box<dyn Future<Output = BackupResult<BackupChunkSourceProvider>> + Send + 'static>,
        > + Send,
>;
pub type BackupChunkTargetCreateFunc = Box<
    dyn FnMut(
            String,
        ) -> Pin<
            Box<dyn Future<Output = BackupResult<BackupChunkTargetProvider>> + Send + 'static>,
        > + Send,
>;

pub struct BackupSourceProviderDesc {
    pub name: String,
    pub desc: String,
    pub type_id: String,
    pub abilities: Vec<String>,
}

pub struct BackupTargetProviderDesc {
    pub name: String,
    pub desc: String,
    pub type_id: String,
    pub abilities: Vec<String>,
}
