use log::*;
use std::{
    fmt::{self, Display}, path::Path, sync::Arc 
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
    target::*
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
}

impl Display for LocalStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LocalStore({})", self.base_path())
    }
}

impl ChunkRead for File {}

#[async_trait]
impl ChunkTarget for LocalStore {
    async fn write<T: ChunkId>(&self, chunk_id: &T, offset: u64, reader: impl AsyncRead + Unpin + Send + Sync + 'static, _length: Option<u64>) -> ChunkResult<ChunkStatus<T>> {
        info!("write to local store: {}, chunk_id: {}, offset: {}, length: {}", self, chunk_id, offset, _length.unwrap_or(0));
        let mut reader = reader;
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        let mut file = File::create(path).await
            .map_err(|e| {
                error!("create file error: {}", e);
                ChunkError::Io(e)
            })?;
        file.seek(std::io::SeekFrom::Start(offset)).await
            .map_err(|e| {
                error!("seek file error: {}", e);
                ChunkError::Io(e)
            })?;
        let written = io::copy(&mut reader, &mut file).await
            .map_err(|e| {
                error!("copy file error: {}", e);
                ChunkError::Io(e)
            })?;
        info!("write to local store: {}, chunk_id: {}, offset: {}, copied: {}", self, chunk_id, offset, written);
        let metadata = file.metadata().await
            .map_err(|e| {
                error!("get file metadata error: {}", e);
                ChunkError::Io(e)
            })?;
        let status = ChunkStatus {
            chunk_id: chunk_id.clone(),
            written: metadata.len(),
        };
        info!("write to local store: {}, chunk_id: {}, offset: {}, written: {}", self, chunk_id, offset, metadata.len());
        Ok(status)
    }

    async fn link(&self, chunk_id: &impl ChunkId, target_chunk_id: &impl ChunkId) -> ChunkResult<()> {
        let source_path = Path::new(self.base_path()).join(chunk_id.to_string());
        let target_path = Path::new(self.base_path()).join(target_chunk_id.to_string());
        info!("link chunk {} to {}", chunk_id, target_chunk_id);

        #[cfg(unix)] 
        {
            fs::symlink(target_path, source_path).await
            .map_err(|e| {
                error!("create symlink error: {}", e);
                ChunkError::Io(e)
            })?;
        } 
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target_path, source_path)
                .map_err(|e| {
                    error!("create symlink error: {}", e);
                    ChunkError::Io(e)
                })?;
        }
        Ok(())
    }

    async fn read(&self, chunk_id: &impl ChunkId) -> ChunkResult<Option<Box<dyn ChunkRead>>> {
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        if path.exists() {
            let file = File::open(path).await?;
            Ok(Some(Box::new(file)))
        } else {
            Ok(None)
        }
    }

    async fn get<T: ChunkId>(&self, chunk_id: &T) -> ChunkResult<Option<ChunkStatus<T>>> {
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        if path.exists() {
            let metadata = std::fs::metadata(path)?;
            let status = ChunkStatus {
                chunk_id: chunk_id.clone(),
                written: metadata.len(),
            };
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, chunk_id: &impl ChunkId) -> ChunkResult<()> {
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        fs::remove_file(path).await?;
        Ok(())
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus<String>>> {
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



