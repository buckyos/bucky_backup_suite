use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
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

#[derive(Debug,Clone)]
pub enum BackupItemType {
    Chunk,
    File,
    Directory,
}

#[derive(Debug,Clone)]
pub struct BackupItem {
    pub item_id: String,//对source来说，可以用item_id来唯一的标示一个待备份的item,一般是文件的相对路径
    pub item_type:BackupItemType,//文件，目录
    pub chunk_id:Option<String>,
    pub quick_hash:Option<String>,
    pub state:BackupItemState,
    pub size:u64,
    pub last_modify_time:u64,//文件的最后修改时间
    pub create_time:u64, //item在系统里的创建时间，不是文件的创建时间
}


#[async_trait]
pub trait IItemReader {
    async fn read(&self, buffer: &mut [u8])->Result<usize>;
    async fn seek(&self, offset: u64)->Result<()>;
    async fn tell(&self)->Result<u64>;
    async fn read_all(&self)->Result<Vec<u8>>;
}

pub type ItemReader = Box<dyn IItemReader+Send+Sync>;


#[async_trait]
pub trait IBackupChunkSourceProvider {
    //return json string?
    async fn get_source_info(&self) -> Result<Value>;
    fn get_source_url(&self)->String;
    async fn lock_for_backup(&self,source_url: &str)->Result<()>;
    async fn unlock_for_backup(&self,source_url: &str)->Result<()>;
    async fn open_item(&self, item_id: &str)->Result<ItemReader>;
    //async fn close_item(&self, item_id: &str)->Result<()>;
    async fn on_item_backuped(&self, item_id: &str)->Result<()>;

    fn is_local(&self)->bool;
    async fn prepare_items(&self)->Result<Vec<BackupItem>>;

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
    
    async fn is_chunk_exist(&self, chunk_id: &str)->Result<bool>;
    async fn get_chunk_state(&self, chunk_id: &str)->Result<String>;
    //查询多个chunk的状态
    async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<String>)->Result<()>;

    async fn put_chunklist(&self, chunk_list: HashMap<String, Vec<u8>>)->Result<()>;
    //上传一个完整的chunk,允许target自己决定怎么使用reader
    async fn put_chunk(&self, chunk_id: &str, offset: u64, chunk_data: &[u8])->Result<()>;
    //使用reader上传，允许target自己决定怎么使用reader
    async fn put_chunk_by_reader(&self, chunk_id: &str, chunk_reader: ItemReader)->Result<()>;
    //通过上传chunk diff文件来创建新chunk
    async fn patch_chunk(&self, chunk_id: &str, chunk_reader: ItemReader)->Result<()>;

    async fn remove_chunk(&self, chunk_list: Vec<String>)->Result<()>;
    //说明两个chunk id是同一个chunk.实现者可以自己决定是否校验
    //link成功后，查询target_chunk_id和new_chunk_id的状态，应该都是exist
    async fn link_chunkid(&self, target_chunk_id: &str, new_chunk_id: &str)->Result<()>;

    
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

