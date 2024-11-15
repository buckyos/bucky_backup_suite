use std::str::FromStr;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncSeek};
use serde::{Deserialize, Serialize};
use crate::{
    error::*, 
    chunk::*
};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkStatus<T: ChunkId> {
    pub chunk_id: T,
    pub written: u64,
}

pub trait ChunkRead: AsyncRead + AsyncSeek + Unpin + Send + Sync {

}

#[async_trait]
pub trait ChunkTarget: Send + Sync {
    /// 将多个数据写入目标存储
    async fn write_vectored<T: ChunkId + Send + Sync + 'static>(&self, readers: Vec<(T, impl AsyncRead + Unpin + Send + Sync + 'static)>) -> ChunkResult<Vec<ChunkStatus<T>>> {
        let reader_futures = readers.into_iter().map(|(chunk_id, reader)| {
            async move {
                self.write(&chunk_id, 0, reader, None).await
            }
        });
        futures::future::try_join_all(reader_futures).await
    }

    /// 将数据写入目标存储
    async fn write<T: ChunkId>(&self, chunk_id: &T, offset: u64, reader: impl AsyncRead + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus<T>>;
   
    /// 将一个chunk链接到另一个chunk
    async fn link(&self, chunk_id: &impl ChunkId, target_chunk_id: &impl ChunkId) -> ChunkResult<()>;
    /// 从目标存储读取数据
    async fn read(&self, chunk_id: &impl ChunkId) -> ChunkResult<Option<Box<dyn ChunkRead>>>;

    /// 获取指定chunk的状态
    async fn get<T: ChunkId>(&self, chunk_id: &T) -> ChunkResult<Option<ChunkStatus<T>>>;

    /// 获取多个chunk的状态
    async fn get_vectored<T: ChunkId + Send + Sync + 'static>(&self, chunk_ids: Vec<&T>) -> ChunkResult<Vec<Option<ChunkStatus<T>>>> {
        let get_futures = chunk_ids.into_iter().map(|chunk_id| {
            async move {
                self.get(chunk_id).await
            }
        });
        futures::future::try_join_all(get_futures).await
    }

    /// 从目标存储中删除指定的chunk
    async fn delete(&self, chunk_id: &impl ChunkId) -> ChunkResult<()>;

    /// 列出目标存储中的所有chunk
    async fn list(&self) -> ChunkResult<Vec<ChunkStatus<String>>>;
}


