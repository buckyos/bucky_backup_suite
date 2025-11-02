
use tokio::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use crossbeam::queue::SegQueue;

use anyhow::Result;
use std::sync::Arc;
use buckyos_backup_lib::*;
use log::*;

const MAX_CACHE_SIZE:u64 = 1024*1024*512;

pub struct ChunkCacheNode {
    pub start_offset: u64,
    pub end_offset: u64,
    pub cache_pieces: SegQueue<(u64,Vec<u8>)>,
}

impl ChunkCacheNode {

    pub fn add_piece(&mut self,piece:Vec<u8>) {
        let piece_len = piece.len() as u64;
        let piece_start_offset = self.end_offset;
        debug!("add piece [{} - {}] (size: {}) to cache",piece_start_offset, piece_start_offset + piece_len, piece_len);
        self.cache_pieces.push((piece_start_offset,piece));
        self.end_offset += piece_len;

    }    

    pub fn free_piece_before_offset(&mut self, _:u64) -> u64 {
        //TODO: implement
        return 0;
    }
    
}

pub struct ChunkTaskCacheMgr {
    pub total_size : Arc<AtomicU64>,
    pub max_size : u64,
    chunk_cache: HashMap<String, Arc<Mutex<ChunkCacheNode>>>,
}



impl ChunkTaskCacheMgr {

    pub fn new() -> Self {
        Self {
            chunk_cache: HashMap::new(),
            total_size: Arc::new(AtomicU64::new(1)),
            max_size: MAX_CACHE_SIZE,
        }
    }

    pub async fn create_chunk_cache(&mut self,chunk_id:&str,start_offset:u64) -> Result<()> {
        if self.chunk_cache.contains_key(chunk_id) {
            return Err(anyhow::anyhow!("Chunk cache already exists"));
        }

        let cache_pieces = SegQueue::new();
        let chunk_cache_node = ChunkCacheNode {
            start_offset,
            end_offset: start_offset,
            cache_pieces,
        };
        self.chunk_cache.insert(chunk_id.to_string(), Arc::new(Mutex::new(chunk_cache_node)));
        Ok(())
    }

    //then can access the cache piece
    pub fn get_chunk_cache_node(&self,chunk_id:&str) -> Option<Arc<Mutex<ChunkCacheNode>>> {
        let result_node = self.chunk_cache.get(chunk_id)?;
        Some(result_node.clone())
    }

    pub async fn free_chunk_cache(&mut self,chunk_id:&str) -> Result<()> {
        if let Some(chunk_cache_node) = self.chunk_cache.remove(chunk_id) {
            let chunk_cache_node = chunk_cache_node.lock().await;
            let mut free_size = 0;
            while let Some((_piece_start_offset,piece)) = chunk_cache_node.cache_pieces.pop() {
                free_size += piece.len() as u64;
            }
            self.total_size.fetch_sub(free_size, Ordering::Relaxed);
            debug!("free {} chunk cache, size: {} MB", chunk_id, free_size / 1024 / 1024);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Chunk cache not found"))
        }
    }

}

lazy_static::lazy_static!{
    pub static ref CHUNK_TASK_CACHE_MGR: Arc<Mutex<ChunkTaskCacheMgr>> = Arc::new(Mutex::new(ChunkTaskCacheMgr::new()));
}

// pub struct CachedReader<R> {
//     chunk_id: String,
//     must_use_cache: bool,
//     inner: R,
//     pos: u64,
//     init_pos: u64,
//     total_len: u64,
//     cache: Arc<Mutex<Vec<u8>>>,
//     // 存储在 poll_read 中正在进行的异步读取 Future
//     read_future: Option<Pin<Box<dyn Future<Output = std::io::Result<usize>> + Send>>>,
//     // 简易 EOF 标记，避免反复创建 Future
//     eof_reached: bool,
// }

// impl<R> CachedReader<R> {
//     pub fn new(chunk_id: &str, inner: R, must_use_cache: bool, pos: u64, total_len: u64) -> Self {
//         Self {
//             chunk_id: chunk_id.to_string(),
//             inner,
//             must_use_cache,
//             pos,
//             init_pos: pos,
//             total_len,
//             cache: Arc::new(Mutex::new(vec![])),
//             read_future: None,
//             eof_reached: false,
//         }
//     }
// }

// impl<R: AsyncRead + AsyncSeek + Unpin> CachedReader<R> {
//     /// 真正执行异步读取的逻辑 (async 函数)
//     async fn read_inner(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
//         debug!("-------------------CachedReader::read_inner, buf len = {}", buf.len());

//         // 先尝试从缓存中获取
//         let cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
//         let chunk_cache_res = cache_mgr.get_chunk_cache(&self.chunk_id, self.pos, buf).await;
//         let max_size = cache_mgr.max_size;
//         let total_size = cache_mgr.total_size.clone();
//         drop(cache_mgr); // 立即释放锁

//         // 如果 cache 命中，返回实际拷贝的长度
//         if let Ok(copied) = chunk_cache_res {
//             self.pos += copied as u64;
//             return Ok(copied as usize);
//         }

//         // 如果当前 pos > init_pos，说明不是第一次读，需要先把底层流移动到指定位置
//         if self.pos > self.init_pos {
//             self.inner.seek(SeekFrom::Start(self.pos)).await?;
//         }

//         // 如果必须要把数据写到缓存里
//         if self.must_use_cache {
//             loop {
//                 // 如果缓存占用超了，就先等待一毫秒，看是否有机会释放
//                 if total_size.load(Ordering::Relaxed) > max_size {
//                     tokio::time::sleep(Duration::from_millis(1)).await;
//                     continue;
//                 }

//                 // 从底层读取
//                 let read_len = self.inner.read(buf).await?;
//                 if read_len == 0 {
//                     // 说明到底层 EOF 了
//                     return Ok(0);
//                 }

//                 // 把读取到的新数据写入缓存
//                 let new_piece = buf[..read_len].to_vec();
//                 let mut cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
//                 cache_mgr.add_chunk_piece(&self.chunk_id, new_piece).await;
//                 drop(cache_mgr);

//                 self.pos += read_len as u64;
//                 return Ok(read_len);
//             }
//         } else {
//             // 不强制写入缓存就直接读
//             let read_len = self.inner.read(buf).await?;
//             self.pos += read_len as u64;
//             Ok(read_len)
//         }
//     }
// }

// /// 为 CachedReader<R> 实现 AsyncRead
// /// --------------------------------
// impl<R: AsyncRead + AsyncSeek + Unpin + Send + 'static> AsyncRead for CachedReader<R> {
//     fn poll_read(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut ReadBuf<'_>,
//     ) -> Poll<std::io::Result<()>> {
//         debug!("CachedReader::poll_read, want to read {} bytes", buf.remaining());

//         let this = self.as_mut().get_mut();

//         // 如果已经读到 EOF 了，直接返回 Ok(())，表示本次没再读到数据
//         if this.eof_reached {
//             debug!("CachedReader::poll_read: already EOF");
//             return Poll::Ready(Ok(()));
//         }

//         // 如果之前没有 Future，那么创建一个新的
//         if this.read_future.is_none() {
//             let unfilled_buf = buf.initialize_unfilled();
//             let fut = Box::pin(this.read_inner(unfilled_buf));
//             this.read_future = Some(fut);
//         }

//         // 拿到当前的 future 并进行轮询
//         let fut = this.read_future.as_mut().unwrap();
//         match fut.as_mut().poll(cx) {
//             Poll::Ready(Ok(n)) => {
//                 debug!("CachedReader::poll_read: read {} bytes", n);
//                 // 读取到多少就推进 buffer 的指针
//                 buf.advance(n);
                
//                 // 用完一次 Future 就清空
//                 this.read_future = None;

//                 // 如果 n == 0，说明到底层 EOF 了
//                 if n == 0 {
//                     this.eof_reached = true;
//                 }

//                 // 返回本次 poll 的结果
//                 Poll::Ready(Ok(()))
//             }
//             Poll::Ready(Err(e)) => {
//                 // 如果读取报错，也要把 Future 清空
//                 this.read_future = None;
//                 debug!("CachedReader::poll_read error: {}", e);
//                 Poll::Ready(Err(e))
//             }
//             Poll::Pending => {
//                 debug!("CachedReader::poll_read pending");
//                 Poll::Pending
//             }
//         }
//     }
// }
// //由于 CachedReader 没有使用 Pin 字段，可以安全地实现 Unpin
// impl<R: AsyncRead + Unpin> Unpin for CachedReader<R> {}

pub struct BackupTaskSession {
    pub task_id: String,
    pub logs:Vec<String>,
}

impl BackupTaskSession {
    pub fn new(task_id:String) -> Self {
        Self {
            task_id,
            logs:Vec::new(),
        }
    }
}