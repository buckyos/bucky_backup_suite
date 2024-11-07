use std::sync::Arc;
use async_std::fs::File;
use chunk::*;
use dmc_tools_user::*;

struct ServerImpl {
    source: SourceServer,
    journal: JournalServer,
    local_store: LocalStore,
}

pub struct DmcxChunkServer(Arc<ServerImpl>);


impl DmcxChunkServer {

}


impl DmcxChunkServer {
    pub fn new() -> Self {
       todo!()
    }
}


#[async_trait::async_trait]
impl ChunkTarget for DmcxChunkServer {
    type Read = File;

    async fn read(&self, chunk_id: &ChunkId) -> ChunkResult<Self::Read> {
        Ok(File::open(chunk_id.to_string()).await?)
    }

    async fn write(&self, chunk_id: &ChunkId, offset: u64, reader: impl async_std::io::BufRead + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus> {
        todo!()
    }

    async fn get(&self, chunk_id: &ChunkId) -> ChunkResult<Option<ChunkStatus>> {
        todo!()
    }

    async fn delete(&mut self, chunk_id: &ChunkId) -> ChunkResult<()> {
        Ok(())
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>> {
        Ok(vec![])
    }
}
