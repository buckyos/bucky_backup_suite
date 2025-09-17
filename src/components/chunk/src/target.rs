use std::{fmt, str::FromStr};
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncSeek};
use serde::{Deserialize, Serialize};
use crate::{
    error::*, 
    chunk::*
};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkStatus {
    pub chunk_id: String,
    pub written: u64,
}

pub struct ChunkWrite<T: AsyncRead + Unpin + Send + Sync + 'static> {
    pub chunk_id: String,
    pub offset: u64,
    pub reader: T,
    pub length: Option<u64>, /* length of the reader*/ 
    pub tail: Option<u64>, /* length of the chunk */
    pub full_id: Option<String> /* full id of the chunk */
}

impl<T: AsyncRead + Unpin + Send + Sync + 'static> fmt::Display for ChunkWrite<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChunkWrite(chunk_id: {}, offset: {}, length: {:?}, tail: {:?})", self.chunk_id, self.offset, self.length, self.tail)
    }
}

#[async_trait]
pub trait ChunkTarget: Clone + Send + Sync {
    /// 将多个数据写入目标存储
    async fn write_vectored<T: AsyncRead + Unpin + Send + Sync + 'static>(&self, params: Vec<ChunkWrite<T>>) -> ChunkResult<Vec<ChunkStatus>> {
        let reader_futures = params.into_iter().map(|param| {
            async move {
                self.write(param).await
            }
        });
        futures::future::try_join_all(reader_futures).await
    }

    /// 将数据写入目标存储
    async fn write<T: AsyncRead + Unpin + Send + Sync + 'static>(&self, param: ChunkWrite<T>) -> ChunkResult<ChunkStatus>;

    type ChunkRead: AsyncRead + AsyncSeek + Unpin + Send + Sync + 'static;
    /// 从目标存储读取数据
    async fn read(&self, chunk_id: &str) -> ChunkResult<Option<Self::ChunkRead>>;

    /// 获取指定chunk的状态
    async fn get(&self, chunk_id: &str) -> ChunkResult<Option<ChunkStatus>>;

    /// 获取多个chunk的状态
    async fn get_vectored(&self, chunk_ids: Vec<String>) -> ChunkResult<Vec<Option<ChunkStatus>>> {
        let get_futures = chunk_ids.into_iter().map(|chunk_id| {
            async move {
                self.get(&chunk_id).await
            }
        });
        futures::future::try_join_all(get_futures).await
    }

    /// 从目标存储中删除指定的chunk
    async fn delete(&self, chunk_id: &str) -> ChunkResult<()>;

    /// 列出目标存储中的所有chunk
    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>>;
}


