use std::collections::HashMap;
use rusqlite::types::{ToSql, FromSql, ValueRef};
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
use ndn_lib::{ChunkReader,ChunkWriter,NamedDataStore,ChunkReadSeek,ChunkId};
use std::pin::Pin;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreConfig {
    pub restore_location_url: String,
    pub is_clean_restore: bool, // 为true时,恢复后只包含恢复的文件,不包含其他文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params:Option<serde_json::Value>,
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
#[derive(Debug,Clone)]
pub enum BackupItemState {
    New,
    LocalProcessing,
    LocalDone,
    Transmitting,
    Done,
    Failed(String),
}

impl ToSql for BackupItemState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            BackupItemState::New => "NEW".to_string(),
            BackupItemState::LocalProcessing => "LOCAL_PROCESSING".to_string(),
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
            "LOCAL_PROCESSING" => BackupItemState::LocalProcessing,
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
}




#[async_trait]
pub trait IBackupChunkSourceProvider {
    //return json string?
    async fn get_source_info(&self) -> Result<Value>;
    fn get_source_url(&self)->String;
    async fn lock_for_backup(&self,source_url: &str)->Result<()>;
    async fn unlock_for_backup(&self,source_url: &str)->Result<()>;
    async fn open_item(&self, item_id: &str)->Result<Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>>;
    async fn get_item_data(&self, item_id: &str)->Result<Vec<u8>>;
    //async fn close_item(&self, item_id: &str)->Result<()>;
    async fn on_item_backuped(&self, item_id: &str)->Result<()>;

    fn is_local(&self)->bool;
    //返回值的bool表示是否完成
    async fn prepare_items(&self)->Result<(Vec<BackupItem>,bool)>;
    async fn init_for_restore(&self, restore_config:&RestoreConfig)->Result<()>;
    //系统倾向于认为restore一定是一个本地操作
    async fn restore_item_by_reader(&self, item: &BackupItem,mut chunk_reader:ChunkReader,restore_config:&RestoreConfig)->Result<()>;
    //async fn prepare_chunk(&self)->Result<String>;
    //async fn get_support_chunkid_types(&self)->Result<Vec<String>>;

}


//TODO ChunkTarget目前只依赖Chunk和Chunklist的语义，是否需要理解CheckPoint的概念?
#[async_trait]
pub trait IBackupChunkTargetProvider {
    async fn get_target_info(&self) -> Result<String>;
    fn get_target_url(&self)->String;
    async fn get_account_session_info(&self)->Result<String>;
    async fn set_account_session_info(&self, session_info: &str)->Result<()>;
    //fn get_max_chunk_size(&self)->Result<u64>;
    //返回Target上已经存在的Checkpoint列表()
    //async fn get_checkpoint_list(&self)->Result<Vec<String>>;

    //下面的接口将要成为通用的http based的chunk操作接口
    //async fn get_support_chunkid_types(&self)->Result<Vec<String>>;
    
    async fn is_chunk_exist(&self, chunk_id: &ChunkId)->Result<(bool,u64)>;
    //查询多个chunk的状态
    async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<ChunkId>)->Result<()>;

    async fn put_chunklist(&self, chunk_list: HashMap<ChunkId, Vec<u8>>)->Result<()>;
    //上传一个完整的chunk,允许target自己决定怎么使用reader
    async fn put_chunk(&self, chunk_id: &ChunkId, chunk_data: &[u8])->Result<()>;
    async fn append_chunk_data(&self, chunk_id: &ChunkId, offset_from_begin:u64,chunk_data: &[u8], is_completed: bool,chunk_size:Option<u64>)->Result<()>;

    //通过上传chunk diff文件来创建新chunk
    //async fn patch_chunk(&self, chunk_id: &str, chunk_reader: ItemReader)->Result<()>;

    //async fn remove_chunk(&self, chunk_list: Vec<String>)->Result<()>;
    //说明两个chunk id是同一个chunk.实现者可以自己决定是否校验
    //link成功后，查询target_chunk_id和new_chunk_id的状态，应该都是exist
    async fn link_chunkid(&self, target_chunk_id: &ChunkId, new_chunk_id: &ChunkId)->Result<()>;
    async fn open_chunk_reader_for_restore(&self, chunk_id: &ChunkId,quick_hash:Option<ChunkId>)->Result<ChunkReader>;
    
}

#[async_trait]
pub trait IBackupDirSourceProvider {
    async fn get_source_info(&self) -> Result<Value>;
}
#[async_trait]
pub trait IBackupDirTargetProvider {
    async fn get_target_info(&self) -> Result<Value>;
}

pub type BackupChunkSourceProvider = Box<dyn IBackupChunkSourceProvider + Send + Sync>;
pub type BackupChunkTargetProvider = Box<dyn IBackupChunkTargetProvider + Send + Sync>;
pub type BackupDirSourceProvider = Box<dyn IBackupDirSourceProvider + Send + Sync>;
pub type BackupDirTargetProvider = Box<dyn IBackupDirTargetProvider + Send + Sync>;


