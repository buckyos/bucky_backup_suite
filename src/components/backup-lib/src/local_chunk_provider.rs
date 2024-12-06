#![allow(unused)]
use anyhow::Ok;
use serde_json::Value;
use async_trait::async_trait;
use anyhow::Result;
use log::info;
use tokio::{
    fs::{self, File,OpenOptions}, 
    io::{self, AsyncRead,AsyncWrite, AsyncReadExt, AsyncWriteExt, AsyncSeek, AsyncSeekExt}, 
};
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use std::pin::Pin;
use tokio::sync::Mutex;
use serde_json::json;
use url::Url;
use ndn_lib::ChunkHasher;
use log::*;


use ndn_lib::{ChunkReadSeek,ChunkStore,ChunkId};
use crate::provider::*;

//待备份的chunk都以文件的形式平摊的保存目录下
pub struct LocalDirChunkProvider {
    pub dir_path: String,

}

impl LocalDirChunkProvider {
    pub async fn new(dir_path: String)->Result<Self>{
        info!("new local dir chunk provider, dir_path: {}", dir_path);
        Ok(LocalDirChunkProvider {
            dir_path
        })
    }
}

#[async_trait]
impl IBackupChunkSourceProvider for LocalDirChunkProvider {


    async fn get_source_info(&self) -> Result<Value> {
        let result = json!({
            "type": "local_chunk_source",
            "dir_path": self.dir_path,
        });
        Ok(result)
    }

    fn get_source_url(&self)->String {
        return format!("file:///{}",self.dir_path);
    }

    async fn lock_for_backup(&self,source_url: &str)->Result<()> {
        Ok(())
    }
    
    async fn unlock_for_backup(&self,source_url: &str)->Result<()> {
        Ok(())
    }

    async fn open_item(&self, item_id: &str)->Result<Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>> {
        let file_path = Path::new(&self.dir_path).join(item_id);
        let file = File::open(&file_path).await.map_err(|e| {
            warn!("open_item: open file failed! {}", e.to_string());
            anyhow::anyhow!("{}",e)
        })?;      
        Ok(Box::pin(file))
    }

    async fn get_item_data(&self, item_id: &str)->Result<Vec<u8>> {
        let file_path = Path::new(&self.dir_path).join(item_id);
        let mut file = File::open(&file_path).await.map_err(|e| anyhow::anyhow!("{}",e))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await.map_err(|e| anyhow::anyhow!("{}",e))?;
        Ok(buffer)
    }
    //async fn close_item(&self, item_id: &str)->Result<()>;
    async fn on_item_backuped(&self, item_id: &str)->Result<()> {
        Ok(())
    }

    fn is_local(&self)->bool {
        true
    }

    async fn prepare_items(&self)->Result<(Vec<BackupItem>,bool)> {
        //遍历dir_path目录下的所有文件，生成BackupItem列表

        let mut backup_items = Vec::new();

        // Read the directory
        let mut entries = fs::read_dir(&self.dir_path).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        //info!("prepare items, dir_path: {}", self.dir_path);
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            //info!("prepare item: {:?}", path);
            // Check if the entry is a file
            if path.is_file() {
                // Create a BackupItem for each file
                let metadata = fs::metadata(&path).await?;
                info!("prepare item: {:?}, size: {}", path, metadata.len());
                let backup_item = BackupItem {
                    item_id: path.file_name().unwrap().to_string_lossy().to_string(),
                    item_type:BackupItemType::Chunk,
                    chunk_id: None,
                    quick_hash: None,
                    state: BackupItemState::New,
                    size: metadata.len(),
                    last_modify_time: metadata.modified()?.elapsed()?.as_secs(),
                    create_time: now,
                };
                backup_items.push(backup_item);
            }
           
        }

        Ok((backup_items,true))
    }

    async fn init_for_restore(&self, restore_config:&RestoreConfig)->Result<()> {
        let restore_url:Url = Url::parse(restore_config.restore_location_url.as_str())
            .map_err(|e| anyhow::anyhow!("{}",e))?;

        if restore_url.scheme() != "file" {
            return Err(anyhow::anyhow!("restore_url scheme must be file"));
        }

        let restore_path = restore_url.path();  
        //TODO : clean up restore_path
        Ok(())
    }

    async fn restore_item_by_reader(&self, item: &BackupItem,mut chunk_reader:Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>,restore_config:&RestoreConfig)->Result<()> {
        let restore_url:Url = Url::parse(restore_config.restore_location_url.as_str())
           .map_err(|e| anyhow::anyhow!("{}",e))?;

        if restore_url.scheme() != "file" {
            return Err(anyhow::anyhow!("restore_url scheme must be file"));
        }

        let restore_path = restore_url.path();
        let file_path = Path::new(&restore_path).join(&item.item_id);
        //TODO: use ndn-client to get chunk would be better
        //文件不存在很简单
        if !file_path.exists() {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&file_path)
                .await
                .map_err(|e| {
                    warn!("restore_item_by_reader: create file failed! {}", e.to_string());
                    anyhow::anyhow!("{}",e)
                })?;
            let mut chunk_reader = chunk_reader.as_mut();
            tokio::io::copy(&mut chunk_reader, &mut file).await.map_err(|e| anyhow::anyhow!("{}",e))?;
        } else {
            let file_info = fs::metadata(&file_path).await.map_err(|e| anyhow::anyhow!("{}",e))?;
            let file_size = file_info.len();
            if file_size == item.size {
                //文件大小相同,TODO:检查quick_hash
                return Ok(());
            } else {
                //文件大小不同,删除文件
                if file_size > item.size  {
                    //文件大于chunk,删除文件
                    fs::remove_file(&file_path).await.map_err(|e| anyhow::anyhow!("{}",e))?;
                    let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&file_path)
                    .await
                    .map_err(|e| {
                        warn!("restore_item_by_reader: create file failed! {}", e.to_string());
                        anyhow::anyhow!("{}",e)
                    })?;
                    let mut chunk_reader = chunk_reader.as_mut();
                    tokio::io::copy(&mut chunk_reader, &mut file).await.map_err(|e| anyhow::anyhow!("{}",e))?;
                } else {
                    chunk_reader.seek(SeekFrom::Start(file_size)).await.map_err(|e| anyhow::anyhow!("{}",e))?;
                    let mut file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .open(&file_path)
                    .await
                    .map_err(|e| {
                        warn!("restore_item_by_reader: open file failed! {}", e.to_string());
                        anyhow::anyhow!("{}",e)
                    })?;
                    let mut chunk_reader = chunk_reader.as_mut();
                    tokio::io::copy(&mut chunk_reader, &mut file).await.map_err(|e| anyhow::anyhow!("{}",e))?;
                }
            }
        }
        Ok(())
    }
}

pub struct LocalChunkTargetProvider {
    pub dir_path: String,
    pub chunk_store:ChunkStore,
}

impl LocalChunkTargetProvider {
    pub async fn new(dir_path: String)->Result<Self>{
        let chunk_store = ChunkStore::new(dir_path.clone()).await.map_err(|e| anyhow::anyhow!("{}",e))?;
        info!("new local chunk target provider, dir_path: {}", dir_path);
        Ok(LocalChunkTargetProvider { 
            dir_path,
            chunk_store 
        })
    }
}

#[async_trait]
impl IBackupChunkTargetProvider for LocalChunkTargetProvider {
    async fn get_target_info(&self) -> Result<String> {
       let result = json!({
            "type": "local_chunk_target",
            "dir_path": self.dir_path,
        });
        Ok(result.to_string())
    }

    fn get_target_url(&self)->String{
        format!("file:///{}",self.dir_path)
    }

    async fn get_account_session_info(&self)->Result<String>{
        Ok(String::new())
    }
    async fn set_account_session_info(&self, session_info: &str)->Result<()>{
        Ok(())
    }
    
    async fn is_chunk_exist(&self, chunk_id: &ChunkId)->Result<(bool,u64)> {
        self.chunk_store.is_chunk_exist(chunk_id,None).await.map_err(|e| anyhow::anyhow!("{}",e))
    }
    //查询多个chunk的状态
    async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<ChunkId>)->Result<()> {
        unimplemented!()
    }

    async fn put_chunklist(&self, chunk_list: HashMap<ChunkId, Vec<u8>>)->Result<()> {
        self.chunk_store.put_chunklist(chunk_list,false).await.map_err(|e| anyhow::anyhow!("{}",e))
    }
    //上传一个完整的chunk,允许target自己决定怎么使用reader
    async fn put_chunk(&self, chunk_id: &ChunkId, chunk_data: &[u8])->Result<()> {
        self.chunk_store.put_chunk(chunk_id,chunk_data,false).await.map_err(|e| anyhow::anyhow!("{}",e))
    }
    async fn append_chunk_data(&self, chunk_id: &ChunkId, offset_from_begin:u64,chunk_data: &[u8], is_completed: bool,chunk_size:Option<u64>)->Result<()> {
        self.chunk_store.append_chunk_data(chunk_id,offset_from_begin,chunk_data,is_completed,chunk_size).await.map_err(|e| anyhow::anyhow!("{}",e))
    }
    //使用reader上传，允许target自己决定怎么使用reader
    async fn put_by_reader(&self, chunk_id: &ChunkId, chunk_reader: Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>)->Result<()> {
        self.chunk_store.put_by_reader(chunk_id,chunk_reader,false).await.map_err(|e| anyhow::anyhow!("{}",e))
    }
    //通过上传chunk diff文件来创建新chunk
    //async fn patch_chunk(&self, chunk_id: &str, chunk_reader: ItemReader)->Result<()>;

    //async fn remove_chunk(&self, chunk_list: Vec<String>)->Result<()>;
    //说明两个chunk id是同一个chunk.实现者可以自己决定是否校验
    //link成功后，查询target_chunk_id和new_chunk_id的状态，应该都是exist
    async fn link_chunkid(&self, target_chunk_id: &ChunkId, new_chunk_id: &ChunkId)->Result<()> {
        self.chunk_store.link_chunkid(target_chunk_id,new_chunk_id).await.map_err(|e| anyhow::anyhow!("{}",e))
    }

    async fn open_chunk_reader_for_restore(&self, chunk_id: &ChunkId,quick_hash:Option<ChunkId>)->Result<Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>> {
        let reader = self.chunk_store.get_chunk_reader(&chunk_id).await;
        if reader.is_ok() {
            let (reader,content_length) = reader.unwrap();
            return Ok(reader);
        }

        if quick_hash.is_some() {
            let quick_hash = quick_hash.unwrap();
            let reader = self.chunk_store.get_chunk_reader(&quick_hash).await;
            if reader.is_err() {
                return Err(anyhow::anyhow!("no chunk found for chunk_id: {} or quick_hash: {}", chunk_id.to_string(), quick_hash.to_string()));
            }
            let (reader,content_length) = reader.unwrap();
            return Ok(reader);
        }
        
        Err(anyhow::anyhow!("no chunk found for chunk_id: {}", chunk_id.to_string()))
    }

}


