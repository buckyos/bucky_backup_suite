use std::{
    path::{Path},
    sync::Arc, 
};
use async_trait::async_trait;
use async_std::{
    fs::File, 
    io::prelude::*, 
    stream::StreamExt
};
use crate::{
    error::*, 
    chunk::*,
    target::*
};

struct StoreImpl {
    base_path: String,
}


pub struct LocalStore(Arc<StoreImpl>);

impl LocalStore {
    pub fn new(base_path: String) -> Self {
        Self(Arc::new(StoreImpl { base_path }))
    }

    pub async fn init(&self) -> ChunkResult<()> {
        let path = Path::new(self.base_path());
        if !path.exists() {
            async_std::fs::create_dir_all(path).await?;
        }
        Ok(())
    }

    fn base_path(&self) -> &str {
        &self.0.base_path
    }
}

#[async_trait]
impl ChunkTarget for LocalStore {
    type Read = File;

    async fn write(&self, chunk_id: &ChunkId, offset: u64, reader: impl BufRead + Unpin + Send + Sync + 'static, _length: Option<u64>) -> ChunkResult<ChunkStatus> {
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        let mut file = File::create(path).await?;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        async_std::io::copy(reader, &mut file).await?;
        let metadata = file.metadata().await?;
        let status = ChunkStatus {
            chunk_id: chunk_id.clone(),
            written: metadata.len(),
        };
        Ok(status)
    }

    async fn read(&self, chunk_id: &ChunkId) -> ChunkResult<File> {
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        let file = File::open(path).await?;
        Ok(file)
    }

    async fn get(&self, chunk_id: &ChunkId) -> ChunkResult<Option<ChunkStatus>> {
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

    async fn delete(&mut self, chunk_id: &ChunkId) -> ChunkResult<()> {
        let path = Path::new(self.base_path()).join(chunk_id.to_string());
        async_std::fs::remove_file(path).await?;
        Ok(())
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>> {
        let mut chunks = Vec::new();
        let mut entries = async_std::fs::read_dir(self.base_path()).await?;
        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if let Ok(chunk_id) = file_name.to_str().unwrap().parse::<ChunkId>() {
                    let metadata = entry.metadata().await?;
                    let status = ChunkStatus {
                        chunk_id,
                        written: metadata.len(),
                    };
                    chunks.push(status);
                }
            }
        }
        Ok(chunks)
    }
}



