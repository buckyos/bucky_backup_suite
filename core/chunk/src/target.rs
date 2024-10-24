use async_trait::async_trait;
use async_std::io::prelude::*;
use serde::{Deserialize, Serialize};
use crate::{
    error::*, 
    chunk::ChunkId
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkStatus {
    pub chunk_id: ChunkId,
    pub written: u64,
}

#[async_trait]
pub trait ChunkTarget {
    /// 将数据写入目标存储
    async fn write(&self, chunk_id: &ChunkId, offset: u64, reader: impl BufRead + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus>;

    type Read: 'static + Read + Seek + Unpin;
    /// 从目标存储读取数据
    async fn read(&self, chunk_id: &ChunkId) -> ChunkResult<Self::Read>;

    /// 获取指定chunk的状态
    async fn get(&self, chunk_id: &ChunkId) -> ChunkResult<Option<ChunkStatus> >;

    /// 从目标存储中删除指定的chunk
    async fn delete(&mut self, chunk_id: &ChunkId) -> ChunkResult<()>;

    /// 列出目标存储中的所有chunk
    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>>;
}
