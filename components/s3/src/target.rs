use std::sync::Arc;
use async_std::io::prelude::*;
use async_trait::async_trait;
use chunk::*;

pub struct S3TargetConfig {
    bucket: String,
    region: String,
}

struct TargetImpl {
    config: S3TargetConfig,
}

#[derive(Clone)]
pub struct S3Target(Arc<TargetImpl>);

#[async_trait]
impl ChunkTarget for S3Target {
    async fn write(&self, chunk_id: &CommonChunkId, offset: u64, reader: impl Read + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus> {
        todo!()
    }


    type Read = async_std::io::BufReader<async_std::io::Cursor<Vec<u8>>>;
    async fn read(&self, chunk_id: &CommonChunkId) -> ChunkResult<Option<Self::Read>> {
        todo!()
    }

    
    async fn link(&self, chunk_id: &CommonChunkId, target_chunk_id: &NormalChunkId) -> ChunkResult<()> {
        todo!()
    }

    async fn get(&self, chunk_id: &CommonChunkId) -> ChunkResult<Option<ChunkStatus>> {
        todo!()
    }

    async fn delete(&self, chunk_id: &CommonChunkId) -> ChunkResult<()> {
        todo!()
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>> {
        todo!()
    }
}



