#![allow(unused)]

use crate::BackupCheckpoint;
use crate::BackupChunkItem;
use crate::BackupResult;
use crate::BuckyBackupError;
use crate::RemoteBackupCheckPointItemStatus;
use async_trait::async_trait;
use log::*;
use ndn_lib::NdnProgressCallback;
use ndn_lib::{ChunkHasher, ChunkReadSeek};
use ndn_lib::{ChunkId, ChunkReader, ChunkWriter, NamedDataMgr, NdnError};
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::Path;
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
    pub dir_path: String,
}

impl LocalDirChunkProvider {
    pub async fn new(dir_path: String) -> BackupResult<Self> {
        info!("new local dir chunk provider, dir_path: {}", dir_path);
        Ok(LocalDirChunkProvider { dir_path })
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
        format!("file:///{}", self.dir_path)
    }

    async fn create_checkpoint(&self, checkpoint_id: &str)->BackupResult<BackupCheckpoint> {
        unimplemented!()
    }

    async fn prepare_items(&self, checkpoint_id: &str, callback: Option<Arc<Mutex<NdnProgressCallback>>>) -> BackupResult<(Vec<BackupChunkItem>, bool)> {
        unimplemented!();
    }

    async fn open_item_chunk_reader(
        &self,
        checkpoint_id: &str,
        backup_item: &BackupChunkItem,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let item_id = backup_item.item_id.clone();
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

    async fn open_chunk_reader(
        &self,
        chunk_id: &ChunkId,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        unimplemented!()
    }


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
        let restore_url: Url =
            Url::parse(restore_config.restore_location_url.as_str()).map_err(|e| {
                warn!("open_writer_for_restore error:{}", e.to_string());
                BuckyBackupError::Failed(e.to_string())
            })?;

        if restore_url.scheme() != "file" {
            return Err(BuckyBackupError::Failed(
                "restore_url scheme must be file".to_string(),
            ));
        }

        let mut restore_path = restore_url.path();

        #[cfg(windows)]
        {
            restore_path = restore_path.trim_start_matches('/');
        }

        let file_path = Path::new(&restore_path).join(&item.item_id);
        let mut real_offset = offset;

        //先判断文件是否存在
        if !file_path.exists() {
            if offset > 0 {
                return Err(BuckyBackupError::Failed(format!(
                    "file not found: {}",
                    file_path.to_string_lossy()
                )));
            }

            return Ok((
                Box::pin(
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&file_path)
                        .await
                        .map_err(|e| {
                            warn!("open_writer_for_restore error:{}", e.to_string());
                            BuckyBackupError::TryLater(e.to_string())
                        })?,
                ),
                0,
            ));
        }

        let file_meta = fs::metadata(&file_path).await.map_err(|e| {
            warn!(
                "restore_item_by_reader: get metadata failed! {}",
                e.to_string()
            );
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
        Ok((Box::pin(file), real_offset))
    }
}

pub struct LocalChunkTargetProvider {
    pub dir_path: String,
    pub named_mgr_id: String, 
}

impl LocalChunkTargetProvider {
    pub async fn new(dir_path: String, named_mgr_id: String) -> BackupResult<Self> {

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
        unimplemented!()
    }

    async fn query_check_point_state(&self, checkpoint_id: &str)->BackupResult<(BackupCheckpoint,RemoteBackupCheckPointItemStatus)> {
        unimplemented!()
    }

    async fn remove_checkpoint(&self, checkpoint_id: &str)->BackupResult<()> {
        unimplemented!()
    }

    async fn open_chunk_writer(
        &self,
        checkpoint_id: &str,
        chunk_id: &ChunkId
    ) -> BackupResult<(ChunkWriter, u64)> {
        unimplemented!()
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
        let reader = NamedDataMgr::open_chunk_reader(Some(&self.named_mgr_id.as_str()), chunk_id, 0, false)
            .await
            .map_err(|e| {
                warn!("open_chunk_reader_for_restore error:{}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })?;
        
        Ok(Box::pin(reader.0))
    }
}
