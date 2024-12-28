#![allow(unused)]

use serde_json::Value;
use async_trait::async_trait;
use anyhow::Result;
use tokio::{
    fs::{self, File,OpenOptions}, 
    io::{self, AsyncRead,AsyncWrite, AsyncReadExt, AsyncWriteExt, AsyncSeek, AsyncSeekExt}, 
};
use std::{collections::HashMap};
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use std::pin::Pin;
use tokio::sync::Mutex;
use serde_json::json;
use url::{form_urlencoded::Target, Url};
use ndn_lib::{ChunkId, ChunkReader, ChunkWriter, NamedDataStore, NdnError};
use ndn_lib::{ChunkHasher, ChunkReadSeek};
use log::*;

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
        format!("file:///{}",self.dir_path)
    }

    async fn open_item(&self, item_id: &str)->BackupResult<Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>> {
        let file_path = Path::new(&self.dir_path).join(item_id);
        let file = OpenOptions::new()
            .read(true)
            .open(&file_path)
            .await
            .map_err(|e| {
                warn!("open_item: open file failed! {}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;

        Ok(Box::pin(file))
    }

    async fn open_item_chunk_reader(&self, item_id: &str,offset:u64)->BackupResult<ChunkReader> {
        let file_path = Path::new(&self.dir_path).join(item_id);
        let mut file = OpenOptions::new()
            .read(true)
            .open(&file_path)
            .await
            .map_err(|e| {
                warn!("open_item_chunk_reader: open file failed! {}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;      
    
        if offset > 0 {
            file.seek(SeekFrom::Start(offset)).await.map_err(|e| {
                warn!("open_item_chunk_reader: seek file failed! {}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;
        }
        Ok(Box::pin(file))
    }
    //async fn close_item(&self, item_id: &str)->Result<()>;
    async fn on_item_backuped(&self, item_id: &str)->Result<()> {
        Ok(())
    }

    fn is_local(&self)->bool {
        true
    }

    async fn prepare_items(&self)->BackupResult<(Vec<BackupItem>,bool)> {
        //遍历dir_path目录下的所有文件，生成BackupItem列表

        let mut backup_items = Vec::new();

        // Read the directory
        let mut entries = fs::read_dir(&self.dir_path).await
            .map_err(|e| {
                warn!("prepare_items error:{}",e.to_string());
                BuckyBackupError::Internal(e.to_string())
            })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        loop {
            let entry = entries.next_entry().await
                .map_err(|e| {
                    warn!("prepare_items error:{}",e.to_string());
                    BuckyBackupError::Internal(e.to_string())
                })?;

            if entry.is_none() {
                break;
            }
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                // Create a BackupItem for each file
                let metadata = fs::metadata(&path).await
                    .map_err(|e| {
                        warn!("prepare_items error:{}",e.to_string());
                        BuckyBackupError::Internal(e.to_string())
                    })?;
                
                let last_modify_time = metadata.modified()
                    .map_err(|e| {
                        warn!("prepare_items error:{}",e.to_string());
                        BuckyBackupError::Internal(e.to_string())
                    })?
                    .elapsed()
                    .map_err(|e| {
                        warn!("prepare_items error:{}",e.to_string());
                        BuckyBackupError::Internal(e.to_string())
                    })?
                    .as_secs();

                info!("prepare item: {:?}, size: {}", path, metadata.len());
                let backup_item = BackupItem {
                    item_id: path.file_name().unwrap().to_string_lossy().to_string(),
                    item_type:BackupItemType::Chunk,
                    chunk_id: None,
                    quick_hash: None,
                    state: BackupItemState::New,
                    size: metadata.len(),
                    last_modify_time,
                    create_time: now,
                    have_cache: false,
                    progress: "".to_string(),
                    diff_info:None,
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

    async fn open_writer_for_restore(&self, item: &BackupItem,restore_config:&RestoreConfig,offset:u64)->BackupResult<(ChunkWriter,u64)> {
        let restore_url:Url = Url::parse(restore_config.restore_location_url.as_str())
           .map_err(|e| {
            warn!("open_writer_for_restore error:{}",e.to_string());
            BuckyBackupError::Failed(e.to_string())
           })?;

        if restore_url.scheme() != "file" {
            return Err(BuckyBackupError::Failed("restore_url scheme must be file".to_string()));
        }

        let restore_path = restore_url.path();
        let file_path = Path::new(&restore_path).join(&item.item_id);
        let mut real_offset = offset;

        //先判断文件是否存在
        if !file_path.exists() {
            if offset > 0 {
                return Err(BuckyBackupError::Failed(format!("file not found: {}", file_path.to_string_lossy())));
            }

            return Ok((Box::pin(OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&file_path)
                .await
                .map_err(|e| {
                    warn!("open_writer_for_restore error:{}", e.to_string());
                    BuckyBackupError::TryLater(e.to_string())
                })?), 0));
        }

        let file_meta = fs::metadata(&file_path).await.map_err(|e| {
            warn!("restore_item_by_reader: get metadata failed! {}", e.to_string());
            BuckyBackupError::TryLater(e.to_string())
        })?;

        let file_size = file_meta.len();
        if offset > file_size {
            real_offset = file_size;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .open(&file_path)
            .await
            .map_err(|e| {
                warn!("open file failed! {}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;

        if offset > 0 {
            file.seek(SeekFrom::Start(real_offset)).await.map_err(|e| {
                warn!("seek file failed! {}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;
        }
        Ok((Box::pin(file),real_offset))
    }
}

pub struct LocalChunkTargetProvider {
    pub dir_path: String,
    pub chunk_store:NamedDataStore,
}

impl LocalChunkTargetProvider {
    pub async fn new(dir_path: String)->Result<Self>{
        let chunk_store = NamedDataStore::new(dir_path.clone()).await.map_err(|e| anyhow::anyhow!("{}",e))?;
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
    

    // //查询多个chunk的状态
    // async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<ChunkId>)->Result<()> {
    //     unimplemented!()
    // }

    // async fn put_chunklist(&self, chunk_list: HashMap<ChunkId, Vec<u8>>)->Result<()> {
    //     self.chunk_store.put_chunklist(chunk_list,false).await.map_err(|e| anyhow::anyhow!("{}",e))
    // }
    async fn is_chunk_exist(&self, chunk_id: &ChunkId)->Result<(bool,u64)> {
        self.chunk_store.is_chunk_exist(chunk_id,None).await.map_err(|e| anyhow::anyhow!("{}",e))
    }

    async fn open_chunk_writer(&self, chunk_id: &ChunkId,offset:u64,size:u64)->BackupResult<(ChunkWriter,u64)> {
        let (mut writer,process) = self.chunk_store.open_chunk_writer(chunk_id,size,offset)
            .await.map_err(|e| {
                match e {
                    NdnError::AlreadyExists(msg) => {
                        BuckyBackupError::AlreadyDone(msg)
                    },
                    _ => {
                        warn!("open_chunk_writer error:{}",e.to_string());
                        BuckyBackupError::TryLater(e.to_string())
                    }
                }
            })?;

        let mut offset = offset;
        if process.len() > 2 {
            let json_value : serde_json::Value = serde_json::from_str(&process).map_err(|e| {
                warn!("can't load process info:{}",e.to_string());
                BuckyBackupError::Failed(e.to_string())
            })?;
            offset = json_value.get("pos").unwrap().as_u64().unwrap();
        } 
        Ok((writer,offset))
    }

    async fn complete_chunk_writer(&self, chunk_id: &ChunkId)->BackupResult<()> {
        self.chunk_store.complete_chunk_writer(chunk_id).await.map_err(|e| {
            warn!("complete_chunk_writer error:{}",e.to_string());
            BuckyBackupError::TryLater(e.to_string())
        })
    }


    //说明两个chunk id是同一个chunk.实现者可以自己决定是否校验
    //link成功后，查询target_chunk_id和new_chunk_id的状态，应该都是exist
    async fn link_chunkid(&self, source_chunk_id: &ChunkId, new_chunk_id: &ChunkId)->BackupResult<()> {
        let from_obj_id = new_chunk_id.to_obj_id();
        let to_obj_id = source_chunk_id.to_obj_id();
        info!("link chunkid from(new): {} to(old): {}", from_obj_id.to_string(), to_obj_id.to_string());
        self.chunk_store.link_object(&from_obj_id,&to_obj_id).await.map_err(|e| {
            warn!("link_chunkid error:{}",e.to_string());
            BuckyBackupError::TryLater(e.to_string())
        })
    }

    async fn query_link_target(&self, source_chunk_id: &ChunkId)->BackupResult<Option<ChunkId>> {
        let obj_id = source_chunk_id.to_obj_id();
        let target_chunk_ids = self.chunk_store.query_link_refs(&obj_id).await.map_err(|e| {
            warn!("query_link_target error:{}",e.to_string());
            BuckyBackupError::Failed(e.to_string())
        })?;

        for target_chunk_id in target_chunk_ids {
            if target_chunk_id.obj_type.as_str() != "qcid" {
                return Ok(Some(ChunkId::from_obj_id(&target_chunk_id)));
            }
        }

        Ok(None)
    }

    async fn open_chunk_reader_for_restore(&self, chunk_id: &ChunkId,offset:u64)->BackupResult<ChunkReader> {
        let reader = self.chunk_store.open_chunk_reader(chunk_id,SeekFrom::Start(offset)).await;
        if reader.is_ok() {
            let (reader,content_length) = reader.unwrap();
            return Ok(reader);
        }
        warn!("no chunk found for chunk_id: {}", chunk_id.to_string());
        Err(BuckyBackupError::Failed(format!("no chunk found for chunk_id: {}", chunk_id.to_string())))
    }

}


