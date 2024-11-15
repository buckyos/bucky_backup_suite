use async_trait::async_trait;
use async_std::io::prelude::*;
use serde::{Deserialize, Serialize};
use crate::{
    error::*, 
    chunk::*
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkStatus {
    pub chunk_id: ChunkId,
    pub written: u64,
}

#[async_trait]
pub trait ChunkTarget: Send + Sync {
    /// 将多个数据写入目标存储
    async fn write_vectored(&self, readers: Vec<(ChunkId, impl Read + Unpin + Send + Sync + 'static)>) -> ChunkResult<Vec<ChunkStatus>> {
        let mut statuses = Vec::new();
        for (chunk_id, reader) in readers {
            statuses.push(self.write(&chunk_id, 0, reader, None).await?);
        }
        Ok(statuses)
    }

    /// 将数据写入目标存储
    async fn write(&self, chunk_id: &ChunkId, offset: u64, reader: impl Read + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus>;
   
    /// 将一个chunk链接到另一个chunk
    async fn link(&self, chunk_id: &ChunkId, target_chunk_id: &NormalChunkId) -> ChunkResult<()>;

    type Read: 'static + Read + Seek + Unpin + Send + Sync;
    /// 从目标存储读取数据
    async fn read(&self, chunk_id: &ChunkId) -> ChunkResult<Option<Self::Read>>;

    /// 获取指定chunk的状态
    async fn get(&self, chunk_id: &ChunkId) -> ChunkResult<Option<ChunkStatus>>;

    /// 获取多个chunk的状态
    async fn get_vectored(&self, chunk_ids: &[ChunkId]) -> ChunkResult<Vec<Option<ChunkStatus>>> {
        let mut statuses = Vec::new();
        for chunk_id in chunk_ids {
            statuses.push(self.get(chunk_id).await?);
        }
        Ok(statuses)
    }

    /// 从目标存储中删除指定的chunk
    async fn delete(&mut self, chunk_id: &ChunkId) -> ChunkResult<()>;

    /// 列出目标存储中的所有chunk
    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>>;
}


