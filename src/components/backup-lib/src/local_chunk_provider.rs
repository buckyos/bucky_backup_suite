#![allow(unused)]

use crate::BackupCheckpoint;
use crate::BackupChunkItem;
use crate::BackupItemState;
use crate::BackupResult;
use crate::BuckyBackupError;
use crate::CHECKPOINT_TYPE_CHUNK;
use crate::CheckPointState;
use crate::RemoteBackupCheckPointItemStatus;
use async_trait::async_trait;
use log::*;
use ndn_lib::*;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{
    fs::{self, File, OpenOptions},
    io::{self, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt},
};
use url::{form_urlencoded::Target, Url};

use crate::provider::*;

//待备份的chunk都以文件的形式平摊的保存目录下
pub struct LocalDirChunkProvider {
    pub dir_path: PathBuf,
    pub named_mgr_id: String,
    pub is_strict_mode: bool,
}

impl LocalDirChunkProvider {
    pub async fn new(dir_path: String, named_mgr_id: String) -> BackupResult<Self> {
        info!("new local dir chunk provider, dir_path: {}", dir_path);
        Ok(LocalDirChunkProvider { dir_path: PathBuf::from(dir_path), named_mgr_id, is_strict_mode: false })
    }

}

#[async_trait]
impl IBackupChunkSourceProvider for LocalDirChunkProvider {
    async fn get_source_info(&self) -> BackupResult<Value> {
        let result = json!({
            "name": "local_chunk_source",
            "desc": "local chunk source provider",
            "type_id": "local_chunk_source",
            "abilities": [ABILITY_LOCAL],
            "dir_path": self.dir_path,
        });
        Ok(result)
    }

    fn is_support(&self, ability:&str)->bool {
        ability == ABILITY_LOCAL
    }

    fn is_local(&self) -> bool {
        true
    }

    fn get_source_url(&self) -> String {
        format!("file:///{}", self.dir_path.to_string_lossy())
    }


    async fn prepare_items(&self, checkpoint_id: &str, callback: Option<Arc<Mutex<NdnProgressCallback>>>) -> BackupResult<(Vec<BackupChunkItem>, u64,bool)> {
        let items = Arc::new(Mutex::new(Vec::<BackupChunkItem>::new()));
        let ndn_mgr_id = Some(self.named_mgr_id.as_str());
        let file_obj_template = FileObject::new("".to_string(), 0, "".to_string());
        let mut check_mode = CheckMode::ByQCID;
        if self.is_strict_mode {
            check_mode = CheckMode::ByFullHash;
        }

        let base_dir = self.dir_path.to_string_lossy().to_string();
        let base_dir_path = PathBuf::from(base_dir);

        let items_clone = items.clone();
        let base_dir_path_clone = base_dir_path.clone();
        let mut total_size = 0;
        let ndn_callback: Option<Arc<Mutex<NdnProgressCallback>>> = Some(Arc::new(Mutex::new(Box::new(move |inner_path:String,action:NdnAction| {
            let items_ref = items_clone.clone();
            let base_dir_path = base_dir_path_clone.clone();
            let callback_clone = callback.clone();
            Box::pin(async move {
                debug!("ndn_callback: {} {}", inner_path, action.to_string());
                let now = buckyos_kit::buckyos_get_unix_timestamp();
                match action {
                    NdnAction::ChunkOK(chunk_id, chunk_size) => {
                        //将inner_path转换为相对路径,路径看起来是 
                        // dirA/fileA/start:end -> chunk_id (大文件)
                        // dirA/fileB -> chunk_id (小文件)
                        let relative_path = Path::new(&inner_path).strip_prefix(&base_dir_path);
                        if relative_path.is_err() {
                            return Err(NdnError::InvalidState(format!("relative path error: {}", inner_path)));
                        }
                        let relative_path = relative_path.unwrap().to_string_lossy().to_string();
                        let backup_item = BackupChunkItem {
                            item_id: relative_path,
                            chunk_id: chunk_id,
                            local_chunk_id: None,
                            state: BackupItemState::New,
                            size: chunk_size,
                            last_update_time: now,
                        };

                        items_ref.lock().await.push(backup_item);
                        total_size += chunk_size;
                        Ok(ProgressCallbackResult::Continue)
                    },
                    NdnAction::FileOK(file_id, file_size) => {
                        if callback_clone.is_some() {
                            let callback_clone = callback_clone.unwrap();
                            let mut callback_clone = callback_clone.lock().await;
                            let ret = callback_clone(inner_path, NdnAction::FileOK(file_id, file_size)).await?;
                            drop(callback_clone);
                            return Ok(ret);
                        }
                        Ok(ProgressCallbackResult::Continue)
                    },
                    _ => {
                        return Ok(ProgressCallbackResult::Continue);
                    }
                }
    
            }) as Pin<Box<dyn std::future::Future<Output = NdnResult<ProgressCallbackResult>> + Send + 'static>>
        }))));

        let ret = cacl_dir_object(ndn_mgr_id, &self.dir_path.as_path(), &file_obj_template, &check_mode,StoreMode::new_local(),ndn_callback).await;
        if ret.is_err() {
            return Err(BuckyBackupError::Failed(ret.err().unwrap().to_string()));
        }

        // 尝试直接取出 Vec，避免克隆
        let items_vec = match Arc::try_unwrap(items) {
            Ok(mutex) => mutex.into_inner(),
            Err(arc) => {
                // 如果还有多个引用（闭包可能仍持有），使用 mem::take 在锁内取出内容
                mem::take(&mut *arc.lock().await)
            }
        };
        Ok((items_vec, total_size, true))

    }

    async fn open_item_chunk_reader(
        &self,
        checkpoint_id: &str,
        backup_item: &BackupChunkItem,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let reader = NamedDataMgr::open_chunk_reader(Some(&self.named_mgr_id.as_str()), &backup_item.chunk_id, offset, true)
            .await
            .map_err(|e| {
                warn!("open_item_chunk_reader error:{}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;
        Ok(Box::pin(reader.0))
    }

    async fn open_chunk_reader(
        &self,
        chunk_id: &ChunkId,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let reader = NamedDataMgr::open_chunk_reader(Some(&self.named_mgr_id.as_str()), chunk_id, offset, true)
            .await
            .map_err(|e| {
                warn!("open_chunk_reader error:{}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;
        Ok(Box::pin(reader.0))
    }

    //for resotre
    async fn add_checkpoint(&self, checkpoint: &BackupCheckpoint)->BackupResult<()> {
        unimplemented!()
    }

    
    async fn init_for_restore(&self, restore_config: &RestoreConfig, checkpoint_id: &str) -> BackupResult<String> {
        unimplemented!()
    }

    async fn open_writer_for_restore(
        &self,
        restore_target_id: &str,
        item: &BackupChunkItem,
        restore_config: &RestoreConfig,
        offset: u64,
    ) -> BackupResult<(ChunkWriter, u64)> {
        unimplemented!()
    }
}

pub struct LocalChunkTargetProvider {
    pub dir_path: String,
    pub named_mgr_id: String, 
}

impl LocalChunkTargetProvider {
    pub async fn new(dir_path: String, named_mgr_id: String) -> BackupResult<Self> {
        let root_path: PathBuf = PathBuf::from(dir_path.clone());
        let mgr_map = NAMED_DATA_MGR_MAP.lock().await;
        let the_named_mgr = mgr_map.get(named_mgr_id.as_str());
        if the_named_mgr.is_some() {
            let the_named_mgr = the_named_mgr.unwrap();
            let the_named_mgr = the_named_mgr.lock().await;
            if the_named_mgr.get_base_dir() != root_path {
                return Err(BuckyBackupError::Failed(format!("named_mgr {} base_dir not match: {}", named_mgr_id, the_named_mgr.get_base_dir().to_string_lossy())));
            }
        } else {
            drop(mgr_map);
            let named_mgr = NamedDataMgr::get_named_data_mgr_by_path(root_path).await
                .map_err(|e| BuckyBackupError::NeedProcess(format!("get named data mgr by path error: {}", e.to_string())))?;
            NamedDataMgr::set_mgr_by_id(Some(&named_mgr_id.as_str()), named_mgr).await
                .map_err(|e| BuckyBackupError::NeedProcess(format!("set named data mgr by id error: {}", e.to_string())))?;
        }

        //create named data mgr ,if exists, return error.
        info!("new local chunk target provider, dir_path: {}", dir_path);
        Ok(LocalChunkTargetProvider {
            dir_path,
            named_mgr_id,
        })
    }
}

#[async_trait]
impl IBackupChunkTargetProvider for LocalChunkTargetProvider {
    async fn get_target_info(&self) -> BackupResult<String> {
        let result = json!({
            "type": "local_chunk_target",
            "dir_path": self.dir_path,
            "named_mgr_id": self.named_mgr_id,
        });
        Ok(result.to_string())
    }

    fn get_target_url(&self) -> String {
        format!("file:///{}", self.dir_path)
    }

    async fn get_account_session_info(&self) -> BackupResult<String> {
        Ok(String::new())
    }
    async fn set_account_session_info(&self, session_info: &str) -> BackupResult<()> {
        Ok(())
    }

    async fn alloc_checkpoint(&self, checkpoint: &BackupCheckpoint)->BackupResult<()> {
        //check free space
        //if free space is not enough, return error
        return Ok(());
    }

    async fn add_backup_item(&self, checkpoint_id: &str, backup_items: &Vec<BackupChunkItem>)->BackupResult<()> {
        return Ok(());
    }

    async fn query_check_point_state(&self, checkpoint_id: &str)->BackupResult<(BackupCheckpoint,RemoteBackupCheckPointItemStatus)> {
        //return Ok((BackupCheckpoint::new(), RemoteBackupCheckPointItemStatus::NotSupport));
        let checkpoint = BackupCheckpoint {
            checkpoint_type: CHECKPOINT_TYPE_CHUNK.to_string(),
            checkpoint_name: checkpoint_id.to_string(),
            prev_checkpoint_id: None,
            state: CheckPointState::Working,
            extra_info: "".to_string(),
            create_time: 0,
            last_update_time: 0,
            item_list_id: "".to_string(),
            item_count: 0,
            total_size: 0,
        };
        Ok((checkpoint, RemoteBackupCheckPointItemStatus::NotSupport))
    }

    async fn remove_checkpoint(&self, checkpoint_id: &str)->BackupResult<()> {
        unimplemented!()
    }

    async fn open_chunk_writer(
        &self,
        checkpoint_id: &str,
        chunk_id: &ChunkId,
        chunk_size: u64,
    ) -> BackupResult<(ChunkWriter, u64)> {
        let (writer, _progress) = NamedDataMgr::open_chunk_writer(Some(&self.named_mgr_id.as_str()), chunk_id, 0, 0)
            .await
            .map_err(|e| match e {
                NdnError::AlreadyExists(msg) => BuckyBackupError::AlreadyDone(msg),
                _ => BuckyBackupError::Failed(e.to_string()),
            })?;
        Ok((writer, 0))
        
    }

    async fn complete_chunk_writer(&self, checkpoint_id: &str, chunk_id: &ChunkId) -> BackupResult<()> {
        NamedDataMgr::complete_chunk_writer(Some(&self.named_mgr_id.as_str()), chunk_id)
            .await
            .map_err(|e| {
                warn!("complete_chunk_writer error:{}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })
    }

    async fn open_chunk_reader_for_restore(
        &self,
        chunk_id: &ChunkId,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let reader = NamedDataMgr::open_chunk_reader(Some(&self.named_mgr_id.as_str()), chunk_id, offset, false)
            .await
            .map_err(|e| {
                warn!("open_chunk_reader_for_restore error:{}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;
        
        Ok(Box::pin(reader.0))
    }
}
