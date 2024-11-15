use log::*;
use std::{
    io::SeekFrom, str::FromStr
};
use async_std::{io::prelude::*};
use tide::{Request, Response, Body, Server};

use crate::{
    error::*, 
    chunk::*,
    target::*, 
};


pub struct HttpChunkServer;

impl HttpChunkServer {
    pub async fn listen<T: 'static + ChunkTarget + Clone + Send + Sync>(http_server: &mut Server<T>) -> ChunkResult<()> {
        http_server.at("/chunk/:chunk_id").post(|mut req: Request<T>| async move {
            let store = req.state().clone();
            let chunk_id = ChunkId::from_str(req.param("chunk_id")?)?;
            if let Some(link_chunk_id) = req.header("link")
                .and_then(|h| h.as_str().parse().ok()) {
                store.link(&chunk_id, &link_chunk_id).await?;

            } else {
                let offset: u64 = req.header("offset")
                    .and_then(|h| h.as_str().parse().ok())
                    .unwrap_or(0);
                let length: Option<u64> = req.header("length")
                    .and_then(|h| h.as_str().parse().ok());
                let body = req.take_body().into_reader();
                store.write(&chunk_id, offset, body, length).await?;
            }
            
            Ok(Response::new(200))
        });

        http_server.at("/chunk/:chunk_id").get(|req: Request<T>| async move {
            let store = req.state().clone();
            let chunk_id = ChunkId::from_str(req.param("chunk_id")?)?;
            let offset: u64 = req.header("offset")
                .and_then(|h| h.as_str().parse().ok())
                .unwrap_or(0);
            let length: Option<usize> = req.header("length")
                .and_then(|h| h.as_str().parse().ok());
            
            let mut chunk = store.read(&chunk_id).await?;
            if let Some(mut chunk) = chunk {
                chunk.seek(SeekFrom::Start(offset)).await?;
                let reader = async_std::io::BufReader::new(chunk);
                let mut res = Response::new(200);
                res.set_body(Body::from_reader(reader, length));
                Ok(res)
            } else {
                Ok(Response::new(404))
            }
        });

        http_server.at("/chunk/:chunk_id").head(|req: Request<T>| async move {
            let store = req.state().clone();
            let chunk_id = req.param("chunk_id").unwrap();
            let chunk_id = ChunkId::from_str(chunk_id).unwrap();
            let status = store.get(&chunk_id).await?;
            let mut res = Response::new(200);
            res.set_body(serde_json::to_string(&status)?);
            Ok(res)
        });

        Ok(())
    }

}