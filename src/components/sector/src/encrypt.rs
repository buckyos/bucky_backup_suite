use std::future::Future;
use std::io::SeekFrom;
use std::pin::Pin;
use std::cell::OnceCell;
use std::sync::{Mutex};
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf, AsyncSeekExt};
use std::task::{Context, Poll};
use aes::Aes256;
use cipher::{Block, BlockEncryptMut, BlockSizeUser};
use chunk::*;
use super::sector::SectorMeta;

struct EncMutPart {
    offset: u64, 
    cached_result: Option<std::io::Result<usize>>, 
    buffer: Vec<u8>, 
    read_offset_in_buffer: Option<usize>, 
    write_offset_in_buffer: Option<usize>,
    encryptor: Option<cbc::Encryptor<Aes256>>, 
    chunk_reader: Box<dyn AsyncRead + Send + Unpin>,
}

impl EncMutPart {
    fn check_block_offset(&mut self, meta: &SectorMeta) {
        if self.offset < meta.header_length() {
            return;
        }
        if self.offset % meta.header().block_size as u64 != 0 {
            return;
        }
        self.encryptor = meta.encryptor_on_offset(self.offset).unwrap();
    }

    fn check_read_buffer(&mut self, buf: &mut ReadBuf<'_>) -> usize {
        if let Some(offset_in_buffer) = self.read_offset_in_buffer.take() {
            let remain_len = self.buffer.len() - offset_in_buffer;
            let read = if buf.remaining() < remain_len {
                buf.remaining()
            } else {
                remain_len
            };
            buf.put_slice(&self.buffer[offset_in_buffer..offset_in_buffer + read]);
            self.offset += read as u64;
            if read < remain_len {
                self.read_offset_in_buffer = Some(offset_in_buffer + read);
            }
            read
        } else {
            0
        }
    }

    fn fill_buffer(&mut self, cx: &mut Context<'_>, mut offset_in_buffer: usize) -> Poll<std::io::Result<()>> {
        loop {
            let mut buf = ReadBuf::new(&mut self.buffer[offset_in_buffer..]);
            let before = buf.filled().len();
            match Pin::new(&mut self.chunk_reader).poll_read(cx, &mut buf) {
                Poll::Ready(Ok(_)) => {
                    let n = buf.filled().len() - before;
                    if offset_in_buffer + n == Aes256::block_size() {
                        self.write_offset_in_buffer = None;
                        if let Some(encryptor) = &mut self.encryptor {
                            encryptor.encrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut self.buffer[..]));
                        }
                        self.read_offset_in_buffer = Some(0);
                        return Poll::Ready(Ok(()));    
                    } else {
                        offset_in_buffer += n;
                    }
                }, 
                Poll::Ready(Err(e)) => {
                    let _ = self.cached_result = Some(Err(std::io::Error::new(e.kind(), e.to_string())));
                    return Poll::Ready(Err(e));
                }
                Poll::Pending => {
                    self.write_offset_in_buffer = Some(offset_in_buffer);
                    return Poll::Pending;
                }
            }
        }
    }
}

pub struct SectorEncryptor {
    meta: SectorMeta,
    mut_part: Mutex<EncMutPart>,
}




impl SectorEncryptor {
    pub async fn new<T: ChunkTarget>(meta: SectorMeta, chunk_target: T, offset: u64) -> ChunkResult<Self> {
        if offset > meta.header_length() && offset % meta.header().block_size as u64 != 0 {
            return Err(ChunkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset must be a multiple of block size")));
        }

        
        let mut_part = EncMutPart {
            offset,
            cached_result: None,
            buffer: vec![0u8; Aes256::block_size()],
            read_offset_in_buffer: None,
            write_offset_in_buffer: None,
            encryptor: meta.encryptor_on_offset(std::cmp::max(offset, meta.header_length())).unwrap(),
            chunk_reader: Self::reader_of_chunks(&meta, &chunk_target, std::cmp::max(offset, meta.header_length())).await?,
        };

        Ok(Self {
            mut_part: Mutex::new(mut_part),
            meta,
        })
    }

    async fn reader_of_chunks<T: ChunkTarget>(meta: &SectorMeta, chunk_target: &T, offset: u64) -> ChunkResult<Box<dyn AsyncRead + Unpin + Send>> {
        struct ChunkStub {
            end_offset_in_sector: u64,
            chunk_reader: Box<dyn AsyncRead + Unpin + Send>
        }
        
        struct ChunksReader {
            offset: u64,
            pedding_length: u64, 
            source_length: u64,
            chunk_stub_index: usize,
            chunk_stubs: Vec<ChunkStub>,
        }

        impl AsyncRead for ChunksReader {
            fn poll_read(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut ReadBuf<'_>,
            ) -> Poll<std::io::Result<()>> {
                let reader = self.get_mut();
                if reader.offset >= reader.pedding_length {
                    return Poll::Ready(Ok(()));
                }

                if reader.offset >= reader.source_length {
                    let read = u64::min(reader.pedding_length - reader.offset, buf.remaining() as u64) as usize;
                    let zero = vec![0u8; read];
                    buf.put_slice(&zero);
                    reader.offset += read as u64;
                    return Poll::Ready(Ok(()));
                }

                let chunk_stub = &mut reader.chunk_stubs[reader.chunk_stub_index];

                let before = buf.filled().len() ;
                match Pin::new(chunk_stub.chunk_reader.as_mut()).poll_read(cx, buf) {
                    Poll::Ready(Ok(_)) => {
                        let n = buf.filled().len() - before;
                        reader.offset += n as u64;
                        if reader.offset >= chunk_stub.end_offset_in_sector {
                            reader.chunk_stub_index += 1;
                        }
                        return Poll::Ready(Ok(()));
                    }
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                }
             
            }
        }

        let chunk_on_offset = meta
            .chunk_on_offset(offset).ok_or(ChunkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset out of range")))?;
        let mut chunk_stubs = vec![];
        {
            let mut chunk_reader = chunk_target.read(&meta.header().chunks[chunk_on_offset.chunk_index].0).await?
                .ok_or(ChunkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "chunk not found")))?;
            if offset > chunk_on_offset.range_in_sector.start {
                chunk_reader.seek(SeekFrom::Start(offset - chunk_on_offset.range_in_sector.start + chunk_on_offset.range_in_chunk.start)).await?;
            }
            chunk_stubs.push(ChunkStub {
                end_offset_in_sector: chunk_on_offset.range_in_sector.end,
                chunk_reader: Box::new(chunk_reader) as Box<dyn AsyncRead + Unpin + Send>,
            });
        }
        
        let mut end_offset_in_sector = chunk_on_offset.range_in_sector.end;
        if chunk_on_offset.chunk_index < meta.header().chunks.len() - 1 {
            for (chunk_id, range_in_chunk) in meta.header().chunks[chunk_on_offset.chunk_index + 1..].iter() {
                let mut chunk_reader = chunk_target.read(&chunk_id).await?
                    .ok_or(ChunkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "chunk not found")))?;
                if range_in_chunk.start > 0 {
                    chunk_reader.seek(SeekFrom::Start(range_in_chunk.start)).await?;
                }
                end_offset_in_sector += range_in_chunk.end - range_in_chunk.start;
                chunk_stubs.push(ChunkStub {
                    end_offset_in_sector,
                    chunk_reader: Box::new(chunk_reader) as Box<dyn AsyncRead + Unpin + Send>,
                });
            }
        }
        Ok(Box::new(ChunksReader {
            offset,
            pedding_length: meta.sector_length() - offset,
            source_length: meta.body_length(),
            chunk_stub_index: 0,
            chunk_stubs,
        }))
    }

    fn read(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let mut mut_part = self.mut_part.lock().unwrap();
        if let Some(result) = &mut_part.cached_result {
            match result {
                Ok(_) => {
                    return Poll::Ready(Ok(()));
                },
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        } else if mut_part.offset < self.meta.header_length() {
            let read = if buf.remaining() < self.meta.header_length() as usize {
                buf.remaining()
            } else {
                self.meta.header_length() as usize
            };
            let offset = mut_part.offset as usize + read;
            buf.put_slice(&self.meta.header_bytes()[mut_part.offset as usize..offset]);
            mut_part.offset = offset as u64;
            return Poll::Ready(Ok(()));
        } else {
            let read = mut_part.check_read_buffer(buf);
            if read > 0 {
                mut_part.check_block_offset(&self.meta);
                return Poll::Ready(Ok(()));
            }
            if let Some(offset_in_buffer) = mut_part.write_offset_in_buffer.take() {
                if mut_part.fill_buffer(cx, offset_in_buffer).is_ready() {
                    mut_part.check_read_buffer(buf);
                    mut_part.check_block_offset(&self.meta);
                    return Poll::Ready(Ok(()));
                } else {
                    return Poll::Pending;
                }
            }
            
            let read = if buf.remaining() % Aes256::block_size() == 0 {
                buf.remaining()
            } else {
                buf.remaining() / Aes256::block_size() * Aes256::block_size()
            };
            if read < Aes256::block_size() {
                if mut_part.fill_buffer(cx, 0).is_ready() {
                    mut_part.check_read_buffer(buf);
                    mut_part.check_block_offset(&self.meta);
                    return Poll::Ready(Ok(()));
                } else {
                    return Poll::Pending;
                }      
            } else {
                let before = buf.filled().len();
                match Pin::new(&mut mut_part.chunk_reader).poll_read(cx, buf) {
                    Poll::Ready(Ok(_)) => {
                        let n = buf.filled().len() - before;
                        for i in 0..n/Aes256::block_size() {
                            if let Some(encryptor) = &mut mut_part.encryptor {
                                encryptor.encrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut buf.filled_mut()[before + i * Aes256::block_size()..before + (i + 1) * Aes256::block_size()]));
                            }
                            mut_part.offset += Aes256::block_size() as u64;
                            mut_part.check_block_offset(&self.meta);
                        }

                        let remain_length = n % Aes256::block_size();
                        if remain_length != 0 {
                            mut_part.buffer.copy_from_slice(&buf.filled()[before + n - remain_length..before + n]);
                            mut_part.write_offset_in_buffer = Some(remain_length);
                        } 
                        return Poll::Ready(Ok(()));
                    }
                    Poll::Ready(Err(e)) => {
                        mut_part.cached_result = Some(Err(std::io::Error::new(e.kind(), e.to_string())));
                        return Poll::Ready(Err(e));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
    }   
}

impl AsyncRead for SectorEncryptor {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.read(cx, buf)
    }
}


struct SeekEncMutPart<T: 'static + Unpin + ChunkTarget> {
    offset: OnceCell<u64>,
    reader_params: OnceCell<(SectorMeta, T)>, 
    cached_result: OnceCell<std::io::Result<usize>>,
    create_future: Option<Pin<Box<dyn Future<Output = ChunkResult<SectorEncryptor>> + Send>>>,
    reader: OnceCell<SectorEncryptor>,
}

pub struct SeekOnceSectorEncryptor<T: 'static + Unpin + ChunkTarget> {
    mut_part: Mutex<SeekEncMutPart<T>>,
}

impl<T: 'static + Unpin + ChunkTarget> SeekOnceSectorEncryptor<T> {
    pub fn new(meta: SectorMeta, chunk_target: T) -> Self {
        Self {
            mut_part: Mutex::new(SeekEncMutPart {
                offset: OnceCell::new(),
                reader_params: OnceCell::from((meta, chunk_target)),
                cached_result: OnceCell::new(),
                create_future: None,
                reader: OnceCell::new(),
            }),
        }
    }
}

impl<T: 'static + Unpin + ChunkTarget> AsyncRead for SeekOnceSectorEncryptor<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let mut mut_self = self.mut_part.lock().unwrap();
        if let Some(result) = mut_self.cached_result.get() {
            match result {
                Ok(_) => return Poll::Ready(Ok(())),
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        }
        if let Some(reader) = mut_self.reader.get_mut() {
            let before = buf.filled().len();
            match Pin::new(reader).poll_read(cx, buf) {
                Poll::Ready(Ok(_)) => {
                    let n = buf.filled().len() - before;
                    *mut_self.offset.get_mut().unwrap() += n as u64;  
                    return Poll::Ready(Ok(()));
                }
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(e));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
        let offset = if let Some(offset) = mut_self.offset.get() {
            *offset
        } else {
            mut_self.offset.set(0).unwrap();
            0
        };
        
        let (meta, chunk_target) = mut_self.reader_params.take().unwrap();
        let mut future = if let Some(future) = mut_self.create_future.take() {
            future
        } else {
            Box::pin(SectorEncryptor::new(meta, chunk_target, offset))
        };
        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(reader)) => {
                let _ = mut_self.reader.set(reader);
                let before = buf.filled().len();
                match Pin::new(mut_self.reader.get_mut().unwrap()).poll_read(cx, buf) {
                    Poll::Ready(Ok(_)) => {
                        let n = buf.filled().len() - before;
                        *mut_self.offset.get_mut().unwrap() += n as u64;  
                        return Poll::Ready(Ok(()));
                    }
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(e));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
            Poll::Pending => {
                mut_self.create_future = Some(future);
                return Poll::Pending;
            }
            Poll::Ready(Err(e)) => {
                let _ = mut_self.cached_result.set(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }
       
    }
}

impl<T: 'static + Unpin + ChunkTarget> AsyncSeek for SeekOnceSectorEncryptor<T> {
    fn start_seek(self: Pin<&mut Self>, pos: SeekFrom) -> std::io::Result<()> {
        let mut_part = self.mut_part.lock().unwrap();
        let new_offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(_) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Seeking from end is not supported"));
            }
            SeekFrom::Current(offset) => {
                let pre = mut_part.offset.get().map_or(0, |v| *v);
                if offset > 0 {
                    pre + offset as u64
                } else {
                    pre.saturating_sub(offset.unsigned_abs())
                }
            }
        };
        if let Some(offset) = mut_part.offset.get() {
            if *offset == new_offset {
                return Ok(());
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Seeking more than once is not supported"
                ));
            }
        }
        mut_part.offset.set(new_offset);
        return Ok(());
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        let mut_part = self.mut_part.lock().unwrap();
        if let Some(offset) = mut_part.offset.get() {
            Poll::Ready(Ok(*offset))
        } else {
            Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, "offset not set")))
        }
        
    }
}