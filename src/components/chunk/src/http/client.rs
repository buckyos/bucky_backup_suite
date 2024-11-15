use std::{io::SeekFrom, pin::Pin, sync::{Arc, Mutex}, task::{Context, Poll, Waker}};
use async_std::{io::prelude::*};
use async_trait::async_trait;
use surf::{self, StatusCode, Body};
use crate::{
    error::*, 
    chunk::*, 
    target::*
};


pub struct HttpClient(Arc<ClientImpl>);

pub struct ClientImpl {
    base_url: String,
}

impl HttpClient {
    pub fn new(base_url: String) -> Self {
        Self(Arc::new(ClientImpl{
            base_url,
        }))
    }
}

#[async_trait]
impl ChunkTarget for HttpClient {
    async fn link(&self, chunk_id: &ChunkId, target_chunk_id: &ChunkId) -> ChunkResult<()> {
        let url = format!("{}/chunk/{}", self.0.base_url, chunk_id);
        let client = surf::Client::new();
        let mut req = client.post(&url);
        req = req.header("link", target_chunk_id.to_string());
        let res = req.send().await
            .map_err(|e| ChunkError::Http(format!("{}", e)))?;
        Ok(())
    }

    async fn write(&self, chunk_id: &ChunkId, offset: u64, reader: impl Read + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus> {
        let url = format!("{}/chunk/{}", self.0.base_url, chunk_id);
        let client = surf::Client::new();
        let mut req = client.post(&url);
        
        if let Some(len) = length {
            req = req.header("Content-Length", len.to_string());
        }
        
        if offset > 0 {
            req = req.header("Range", format!("bytes={}-", offset));
        }
        
        let res = req.body(Body::from_reader(async_std::io::BufReader::new(reader), length.map(|l| l as usize))).send().await
            .map_err(|e| ChunkError::Http(format!("{}", e)))?;
        
        if res.status().is_success() {
            let content_length = res.header("Content-Length")
                .and_then(|v| v.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            
            Ok(crate::ChunkStatus {
                chunk_id: chunk_id.clone(),
                written: content_length,
            })
        } else {
            Err(ChunkError::Http(format!("{}", res.status())))
        }
    }

    type Read = HttpRead;
    async fn read(&self, chunk_id: &ChunkId) -> ChunkResult<Option<Self::Read>> {
        let url = format!("{}/chunk/{}", self.0.base_url, chunk_id);
        Ok(Some(HttpRead::new(url)))
    }

    async fn get(&self, chunk_id: &ChunkId) -> ChunkResult<Option<ChunkStatus>> {
        let url = format!("{}/chunk/{}", self.0.base_url, chunk_id);
        let client = surf::Client::new();
        let mut res = client.head(&url).send().await
        .map_err(|e| ChunkError::Http(format!("{}", e)))?;
        
        if res.status().is_success() {
            let status = res.body_json().await
                .map_err(|e| ChunkError::Http(format!("resp body: {}", e)))?;
            Ok(status)
        } else {
            Err(ChunkError::Http(format!("{}", res.status())))
        }
    }

    async fn delete(&mut self, chunk_id: &ChunkId) -> ChunkResult<()> {
        let url = format!("{}/chunk/{}", self.0.base_url, chunk_id);
        let client = surf::Client::new();
        let res = client.delete(&url).send().await
            .map_err(|e| ChunkError::Http(format!("{}", e)))?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(ChunkError::Http(format!("{}", res.status())))
        }
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>> {
        let url = format!("{}/chunks", self.0.base_url);
        let client = surf::Client::new();
        let mut res = client.get(&url).send().await
            .map_err(|e| ChunkError::Http(format!("{}", e)))?;
        if res.status().is_success() {
            let chunks: Vec<ChunkStatus> = res.body_json().await
                .map_err(|e| ChunkError::Http(format!("resp body: {}", e)))?;
            Ok(chunks)
        } else {
            Err(ChunkError::Http(format!("{}", res.status())))
        }
    }
}

struct ReqStub {
    waker: Option<Waker>, 
    offset: u64, 
    resp: Option<Result<Box<dyn BufRead + Unpin + Send + Sync + 'static>, String>>,
}

struct ReadImpl {
    url: String, 
    stub: Mutex<ReqStub>,
}

#[derive(Clone)]
pub struct HttpRead(Arc<ReadImpl>);

impl HttpRead {
    fn new(url: String) -> Self {
        Self(Arc::new(ReadImpl { 
            url, 
            stub: Mutex::new(ReqStub {
                waker: None,
                offset: 0,
                resp: None,
            }) 
        }))
    }

    async fn req(self: &Self) {
        let client = surf::Client::new();
        let req = client.get(&self.0.url);
        let resp = req.send().await
            .map_err(|e| e.to_string())
            .map(|mut r| r.take_body().into_reader());
        let mut stub = self.0.stub.lock().unwrap();
        stub.resp = Some(resp);
        if let Some(waker) = stub.waker.take() {
            waker.wake();
        }
    }
}

impl Read for HttpRead {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let mut stub = self.0.stub.lock().unwrap();
        if let Some(resp) = stub.resp.as_mut() {
            match resp.as_mut() {
                Ok(resp) => {
                    Pin::new(resp.as_mut()).poll_read(cx, buf)
                }
                Err(e) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.as_str()))),
            }
        } else {
            let old_waker = stub.waker.take();
            stub.waker = Some(cx.waker().clone());
            if old_waker.is_none() {
                let read = self.clone();
                async_std::task::spawn(async move {
                    let _ = read.req().await;
                });
            }
            Poll::Pending
        }
    }
}

impl Seek for HttpRead {
    fn poll_seek(self: Pin<&mut Self>, _cx: &mut Context<'_>, pos: SeekFrom) -> Poll<std::io::Result<u64>> {
        let mut stub = self.0.stub.lock().unwrap();
        Poll::Ready(match pos {
            SeekFrom::Start(offset) => {
                stub.offset = offset;
                Ok(stub.offset)
            }, 
            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    stub.offset = stub.offset.saturating_add(offset as u64);
                } else {
                    stub.offset = stub.offset.saturating_sub(offset.unsigned_abs());
                }
                Ok(stub.offset)
            },
            SeekFrom::End(_) => Err(std::io::Error::new(std::io::ErrorKind::Other, "Seek from end is not supported")),
        })
    }
}


