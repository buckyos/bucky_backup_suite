
use rusqlite::types::{ToSql, FromSql, ValueRef};
use async_trait::async_trait;
use serde_json::Value;
use ndn_lib::{ChunkId, ChunkReadSeek, ChunkReader, ChunkWriter, ObjId};
use std::pin::Pin;
use serde::{Serialize, Deserialize};
use crate::BackupResult;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreConfig {
    pub restore_location_url: String,
    pub is_clean_restore: bool, // 为true时,恢复后只包含恢复的文件,不包含其他文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params:Option<serde_json::Value>,
}

pub struct BackupConfig {
    pub crypto_config:String
}

impl ToSql for RestoreConfig {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = serde_json::to_string(self).map_err(|e| 
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        )?;
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

//use tokio::fs::AsyncReadExt;
#[derive(Debug,Clone,PartialEq)]
pub enum BackupItemState {
    New,
    LocalDone,
    Transmitting,
    Done,
    Failed(String),
}

impl ToSql for BackupItemState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            BackupItemState::New => "NEW".to_string(),
            BackupItemState::LocalDone => "LOCAL_DONE".to_string(),
            BackupItemState::Transmitting => "TRANSMITTING".to_string(),
            BackupItemState::Done => "DONE".to_string(),
            BackupItemState::Failed(msg) => format!("FAILED:{}", msg),
        };

        Ok(s.into())
    }
}

impl FromSql for BackupItemState {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "NEW" => BackupItemState::New,
            "LOCAL_DONE" => BackupItemState::LocalDone,
            "TRANSMITTING" => BackupItemState::Transmitting,
            "DONE" => BackupItemState::Done,
            _ => {
                if s.starts_with("FAILED:") {
                    BackupItemState::Failed(s.to_string())
                } else {
                    BackupItemState::New
                }
            }
        })
    }
}

#[derive(Debug,Clone)]
pub enum BackupItemType {
    Chunk,
    File,
    Directory,
}

impl ToSql for BackupItemType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            BackupItemType::Chunk => "CHUNK".to_string(),
            BackupItemType::File => "FILE".to_string(),
            BackupItemType::Directory => "DIRECTORY".to_string(),
        };
        Ok(s.into())
    }
}

impl FromSql for BackupItemType {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "CHUNK" => BackupItemType::Chunk,
            "FILE" => BackupItemType::File,
            "DIRECTORY" => BackupItemType::Directory,
            _ => BackupItemType::File, // 默认文件类型
        })
    }
}



#[derive(Debug,Clone)]
pub struct BackupItem {
    pub item_id: String,//对source来说，可以用item_id来唯一的标示一个待备份的item,一般是文件的相对路径
    pub item_type:BackupItemType,//文件，目录, Piece?
    pub chunk_id:Option<String>,
    pub quick_hash:Option<String>,
    pub state:BackupItemState,
    pub size:u64,
    pub last_modify_time:u64,//文件的最后修改时间
    pub create_time:u64, //item在系统里的创建时间，不是文件的创建时间
    pub progress:String,
    pub have_cache:bool,//是否已经缓存到本地
    pub diff_info:Option<String>,//diff信息
}

#[async_trait]
pub trait IBackupChunkSourceProvider {
    //return json string?
    async fn get_source_info(&self) -> BackupResult<Value>;
    fn get_source_url(&self)->String;
    fn is_local(&self)->bool;
    //async fn lock_for_backup(&self,source_url: &str)->BackupResult<()>;
    //async fn unlock_for_backup(&self,source_url: &str)->BackupResult<()>;
    async fn prepare_items(&self)->BackupResult<(Vec<BackupItem>,bool)>;
    async fn open_item(&self, item_id: &str)->BackupResult<Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>>;
    async fn open_item_chunk_reader(&self, item_id: &str,offset:u64)->BackupResult<ChunkReader>;
    async fn on_item_backuped(&self, item_id: &str)->BackupResult<()>;
    //restore
    async fn init_for_restore(&self, restore_config:&RestoreConfig)->BackupResult<()>;
    async fn open_writer_for_restore(&self, item: &BackupItem,restore_config:&RestoreConfig,offset:u64)->BackupResult<(ChunkWriter,u64)>;
}


//TODO ChunkTarget目前只依赖Chunk和Chunklist的语义，是否需要理解CheckPoint的概念?
#[async_trait]
pub trait IBackupChunkTargetProvider {
    async fn get_target_info(&self) -> BackupResult<String>;
    fn get_target_url(&self)->String;
    async fn get_account_session_info(&self)->BackupResult<String>;
    async fn set_account_session_info(&self, session_info: &str)->BackupResult<()>;
    //fn get_max_chunk_size(&self)->Result<u64>;
    //返回Target上已经存在的Checkpoint列表()
    //async fn get_checkpoint_list(&self)->Result<Vec<String>>;

    //下面的接口将要成为通用的http based的chunk操作接口
    //async fn get_support_chunkid_types(&self)->Result<Vec<String>>;
    
    async fn is_chunk_exist(&self, chunk_id: &ChunkId)->BackupResult<(bool,u64)>;
    async fn open_chunk_writer(&self, chunk_id: &ChunkId,offset:u64,size:u64)->BackupResult<(ChunkWriter,u64)>;
    async fn complete_chunk_writer(&self, chunk_id: &ChunkId)->BackupResult<()>;
    async fn link_chunkid(&self, source_chunk_id: &ChunkId, new_chunk_id: &ChunkId)->BackupResult<()>;
    async fn query_link_target(&self, source_chunk_id: &ChunkId)->BackupResult<Option<ChunkId>>;
    //查询多个chunk的状态
    //async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<ChunkId>)->Result<()>;
    //async fn put_chunklist(&self, chunk_list: HashMap<ChunkId, Vec<u8>>)->Result<()>;
    // restore
    async fn open_chunk_reader_for_restore(&self, chunk_id: &ChunkId,offset:u64)->BackupResult<ChunkReader>;
    
}

//不需要定义dir source/target ? 完全基于named data mgr来管理？
#[async_trait]
pub trait IBackupDirSourceProvider {
    async fn get_source_info(&self) -> BackupResult<Value>;
    async fn list_dirs(&self)->BackupResult<Vec<String>>;
    //return url for backup target
    async fn prepare_dir_for_backup(&self, dir_id: &str,backup_config:&BackupConfig)->BackupResult<()>;
    async fn start_restore_dir(&self, dir_id: &str,dir_obj_id:&ObjId,restore_config:&RestoreConfig)->BackupResult<()>;
    async fn query_restore_task_info(&self, dir_obj_id: &ObjId)->BackupResult<String>;
}
#[async_trait]
pub trait IBackupDirTargetProvider {
    async fn get_target_info(&self) -> BackupResult<String>;
    async fn list_dirs(&self)->BackupResult<Vec<String>>;
    async fn prepare_dir_for_restore(&self, dir_id: &str,restore_config:&RestoreConfig)->BackupResult<()>;

    async fn start_backup_dir(&self, dir_id: &str,dir_obj_id:&ObjId)->BackupResult<()>;
    async fn query_backup_task_info(&self, dir_obj_id: &ObjId)->BackupResult<String>;
}

pub type BackupChunkSourceProvider = Box<dyn IBackupChunkSourceProvider + Send + Sync>;
pub type BackupChunkTargetProvider = Box<dyn IBackupChunkTargetProvider + Send + Sync>;
pub type BackupDirSourceProvider = Box<dyn IBackupDirSourceProvider + Send + Sync>;
pub type BackupDirTargetProvider = Box<dyn IBackupDirTargetProvider + Send + Sync>;





