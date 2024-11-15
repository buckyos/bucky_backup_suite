use log::*;
use std::{
    fmt::{self, Display}, path::Path, sync::Arc, time::{SystemTime, UNIX_EPOCH} 
};
use async_trait::async_trait;
use tokio::{
    fs::{self, File}, 
    io::{self, AsyncRead, AsyncSeek, AsyncSeekExt}, 
};
use futures::Stream;
use crate::{
    error::*, 
    chunk::*,
    target::*,
    source::*,
};

struct StoreImpl {
    base_path: String,
}

#[derive(Clone)]
pub struct LocalStore(Arc<StoreImpl>);

impl LocalStore {
    pub fn new(base_path: String) -> Self {
        Self(Arc::new(StoreImpl { base_path }))
    }

    pub async fn init(&self) -> ChunkResult<()> {
        let path = Path::new(self.base_path());
        if !path.exists() {
            fs::create_dir_all(path).await?;
        }
        Ok(())
    }

    fn base_path(&self) -> &str {
        &self.0.base_path
    }

    async fn link(&self, quick_id: &str, full_id: &str) -> ChunkResult<()> {
        let source_path = Path::new(self.base_path()).join(quick_id);
        let target_path = Path::new(self.base_path()).join(full_id);
        info!("link chunk {} to {}", quick_id, full_id);

        #[cfg(unix)] 
        {
            fs::symlink(full_id, quick_id).await
            .map_err(|e| {
                error!("create symlink error: {}", e);
                ChunkError::Io(e)
            })?;
        } 
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(quick_id, full_id)
                .map_err(|e| {
                    error!("create symlink error: {}", e);
                    ChunkError::Io(e)
                })?;
        }
        Ok(())
    }
}

impl Display for LocalStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LocalStore({})", self.base_path())
    }
}


#[async_trait]
impl ChunkTarget for LocalStore {
    async fn write<T: AsyncRead + Unpin + Send + Sync + 'static>(&self, param: ChunkWrite<T>) -> ChunkResult<ChunkStatus> {
        let mut param = param;
        info!("write to local store: {}, param: {}", self, param);
        let path = Path::new(self.base_path()).join(&param.chunk_id);
        let mut file = File::create(path).await
            .map_err(|e| {
                error!("create file error: {}", e);
                ChunkError::Io(e)
            })?;
        file.seek(std::io::SeekFrom::Start(param.offset)).await
            .map_err(|e| {
                error!("seek file error: {}", e);
                ChunkError::Io(e)
            })?;
        let written = io::copy(&mut param.reader, &mut file).await
            .map_err(|e| {
                error!("copy file error: {}", e);
                ChunkError::Io(e)
            })?;
        info!("write to local store: {}, param: {}, copied: {}", self, param, written);
        let metadata = file.metadata().await
            .map_err(|e| {
                error!("get file metadata error: {}", e);
                ChunkError::Io(e)
            })?;
        info!("write to local store: {}, param: {}, written: {}", self, param, metadata.len());
        if let Some(full_id) = param.full_id {
            self.link(&param.chunk_id, &full_id).await?;
        }
        let status = ChunkStatus {
            chunk_id: param.chunk_id,
            written: metadata.len(),
        };


       
        Ok(status)
    }

   

    type ChunkRead = File;
    async fn read(&self, chunk_id: &str) -> ChunkResult<Option<File>> {
        let path = Path::new(self.base_path()).join(chunk_id);
        if path.exists() {
            let file = File::open(path).await?;
            Ok(Some(file))
        } else {
            Ok(None)
        }
    }

    async fn get(&self, chunk_id: &str) -> ChunkResult<Option<ChunkStatus>> {
        let path = Path::new(self.base_path()).join(chunk_id);
        if path.exists() {
            let metadata = std::fs::metadata(path)?;
            let status = ChunkStatus {
                chunk_id: chunk_id.to_owned(),
                written: metadata.len(),
            };
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, chunk_id: &str) -> ChunkResult<()> {
        let path = Path::new(self.base_path()).join(chunk_id);
        fs::remove_file(path).await?;
        Ok(())
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>> {
        let mut chunks = Vec::new();
        let mut entries = fs::read_dir(self.base_path()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let chunk_id = file_name.to_str().unwrap().to_owned();
                let metadata = entry.metadata().await?;
                let status = ChunkStatus {
                    chunk_id,
                    written: metadata.len(),
                };
                chunks.push(status);
            }
        }
        Ok(chunks)
    }
}


#[async_trait]
impl ChunkSource for LocalStore {
    type Read = File;
    async fn open_item(&self, item_id: &str)->ChunkResult<Self::Read> {
        let path = Path::new(self.base_path()).join(item_id);
        let file = File::open(path).await?;
        Ok(file)
    }

    async fn prepare_items(&self)->ChunkResult<Vec<ChunkItem>> {
        let mut items = Vec::new();
        let mut entries = fs::read_dir(self.base_path()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if let Some(file_name_str) = file_name.to_str() {
                    let metadata = entry.metadata().await?;
                    let item = ChunkItem {
                        item_id: file_name_str.to_owned(),
                        chunk_id: None, 
                        length: metadata.len(),
                        last_modify_time: metadata.modified()?.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                        create_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                    };
                    items.push(item);
                }
            }
        }
        Ok(items)
    }
}

