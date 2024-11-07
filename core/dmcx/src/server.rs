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
