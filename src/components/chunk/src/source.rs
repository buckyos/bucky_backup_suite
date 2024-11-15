use tokio::io::{AsyncRead, AsyncSeek};
use async_trait::async_trait;
use crate::{
    error::*, 
    chunk::*,
};

#[derive(Debug,Clone)]
pub struct ChunkItem {
    pub item_id: String,
    pub chunk_id: Option<String>,
    pub length: u64,
    pub last_modify_time: u64,
    pub create_time: u64, 
}


#[async_trait]
pub trait ChunkSource: Clone + Send + Sync {
    type Read: AsyncRead + AsyncSeek + Unpin + Send + Sync;
    async fn open_item(&self, item_id: &str)->ChunkResult<Self::Read>;
    async fn prepare_items(&self)->ChunkResult<Vec<ChunkItem>>;
}