use log::*;
use std::{
    io::SeekFrom, str::FromStr, sync::Arc, time::Duration
};
use async_std::{fs, io::prelude::*, stream::StreamExt, task};
use serde::{Serialize, Deserialize};
use tide::{Request, Response, Body, http::Url};

use crate::{
    error::*, 
    chunk::*,
    target::*, 
    local_store::LocalStore
};

#[derive(Serialize, Deserialize, Clone)]
pub struct HttpServerConfig {
    pub host: String, 
    pub root: String,
}

struct ServerImpl {
    store: LocalStore,
    config: HttpServerConfig
}


#[derive(Clone)]
pub struct HttpServer(Arc<ServerImpl>);

impl HttpServer {
    fn config(&self) -> &HttpServerConfig {
        &self.0.config
    }

    fn store(&self) -> &LocalStore {
        &self.0.store
    }

    pub async fn listen(self) -> ChunkResult<()> {
        let host = self.config().host.clone();
        let mut http_server = tide::with_state(self);
       

        http_server.at("/chunk/:chunk_id").post(|mut req: Request<Self>| async move {
            let server = req.state().clone();
            let chunk_id = ChunkId::from_str(req.param("chunk_id")?)?;
            let offset: u64 = req.header("offset")
                .and_then(|h| h.as_str().parse().ok())
                .unwrap_or(0);
            let length: Option<u64> = req.header("length")
                .and_then(|h| h.as_str().parse().ok());
            let body = req.take_body().into_reader();
            server.store().write(&chunk_id, offset, body, length).await?;
            
            Ok(Response::new(200))
        });

        http_server.at("/chunk/:chunk_id").get(|req: Request<Self>| async move {
            let server = req.state().clone();
            let chunk_id = ChunkId::from_str(req.param("chunk_id")?)?;
            let offset: u64 = req.header("offset")
                .and_then(|h| h.as_str().parse().ok())
                .unwrap_or(0);
            let length: Option<usize> = req.header("length")
                .and_then(|h| h.as_str().parse().ok());
            
            let mut file = server.store().read(&chunk_id).await?;
            file.seek(SeekFrom::Start(offset)).await?;
            let reader = async_std::io::BufReader::new(file);
            let mut res = Response::new(200);
            res.set_body(Body::from_reader(reader, length));
            Ok(res)
        });

        http_server.at("/chunk/:chunk_id").head(|req: Request<Self>| async move {
            let server = req.state().clone();
            let chunk_id = req.param("chunk_id").unwrap();
            let chunk_id = ChunkId::from_str(chunk_id).unwrap();
            let status = server.store().get(&chunk_id).await?;
            let mut res = Response::new(200);
            res.set_body(serde_json::to_string(&status)?);
            Ok(res)
        });

        let _ = http_server.listen(host.as_str()).await?;
        Ok(())
    }

    pub fn new(config: HttpServerConfig) -> Self {
        Self(Arc::new(ServerImpl {
            store: LocalStore::new(config.root.clone()), 
            config, 
        }))
    }

    pub async fn init(&self) -> ChunkResult<()> {
        self.store().init().await?;
        Ok(())
    }

}