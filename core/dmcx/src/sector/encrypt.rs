use std::cell::{OnceCell, RefCell};
use std::future::Future;
use std::io::SeekFrom;
use std::pin::Pin;
use async_std::io::prelude::*;
use std::task::{Context, Poll};
use aes::Aes256;
use cipher::{Block, BlockEncryptMut, BlockSizeUser};
use chunk::*;
use super::sector::SectorMeta;

struct EncMutPart {
    offset: u64, 
    cached_result: OnceCell<std::io::Result<usize>>, 
    buffer: Vec<u8>, 
    read_offset_in_buffer: Option<usize>, 
    write_offset_in_buffer: Option<usize>,
    encryptor: Option<cbc::Encryptor<Aes256>>, 
    chunk_reader: Box<dyn Read + Unpin>,
}

impl EncMutPart {
    fn check_block_offset(&mut self, meta: &SectorMeta) {
        if self.offset < meta.header_length() {
            return;
        }
        if self.offset % meta.block_size() as u64 != 0 {
            return;
        }
        self.encryptor = meta.encryptor_on_offset(self.offset).unwrap();
    }

    fn check_read_buffer(&mut self, buf: &mut [u8]) -> usize {
        if let Some(offset_in_buffer) = self.read_offset_in_buffer.take() {
            let remain_len = self.buffer.len() - offset_in_buffer;
            let read = if buf.len() < remain_len {
                buf.len()
            } else {
                remain_len
            };
            buf[..read].copy_from_slice(&self.buffer[offset_in_buffer..offset_in_buffer + read]);
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
            match Pin::new(&mut self.chunk_reader).poll_read(cx, &mut self.buffer[offset_in_buffer..]) {
                Poll::Ready(Ok(n)) => {
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
                    let _ = self.cached_result.set(Err(std::io::Error::new(e.kind(), e.to_string())));
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
    header_part: Vec<u8>,
    mut_part: RefCell<EncMutPart>,
}


pub struct SeekOnceSectorEncryptor<T: 'static + Unpin + ChunkTarget> {
    offset: OnceCell<u64>,
    reader_params: OnceCell<(SectorMeta, T)>, 
    cached_result: OnceCell<std::io::Result<usize>>,
    create_future: RefCell<Option<Pin<Box<dyn Future<Output = ChunkResult<SectorEncryptor>>>>>>,
    reader: OnceCell<SectorEncryptor>,
}

impl<T: 'static + Unpin + ChunkTarget> Read for SeekOnceSectorEncryptor<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let mut_self = self.get_mut();
        if let Some(result) = mut_self.cached_result.get() {
            match result {
                Ok(n) => return Poll::Ready(Ok(*n)),
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        }
        if let Some(reader) = mut_self.reader.get_mut() {
            match Pin::new(reader).poll_read(cx, buf) {
                Poll::Ready(Ok(n)) => {
                    *mut_self.offset.get_mut().unwrap() += n as u64;  
                    return Poll::Ready(Ok(n));
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
        let mut future = if let Some(future) = mut_self.create_future.get_mut().take() {
            future
        } else {
            Box::pin(SectorEncryptor::new(meta, chunk_target, offset))
        };
        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(reader)) => {
                let _ = mut_self.reader.set(reader);
                match Pin::new(mut_self.reader.get_mut().unwrap()).poll_read(cx, buf) {
                    Poll::Ready(Ok(n)) => {
                        *mut_self.offset.get_mut().unwrap() += n as u64;  
                        return Poll::Ready(Ok(n));
                    }
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(e));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
            Poll::Pending => {
                *mut_self.create_future.borrow_mut() = Some(future);
                return Poll::Pending;
            }
            Poll::Ready(Err(e)) => {
                let _ = mut_self.cached_result.set(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }
       
    }
}

impl<T: 'static + Unpin + ChunkTarget> Seek for SeekOnceSectorEncryptor<T> {
    fn poll_seek(self: Pin<&mut Self>, _: &mut Context<'_>, pos: SeekFrom) -> Poll<std::io::Result<u64>> {
        let new_offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(_) => {
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Seeking from end is not supported")));
            }
            SeekFrom::Current(offset) => {
                let pre = self.offset.get().map_or(0, |v| *v);
                if offset > 0 {
                    pre + offset as u64
                } else {
                    pre.saturating_sub(offset.unsigned_abs())
                }
            }
        };
        if let Some(offset) = self.offset.get() {
            if *offset == new_offset {
                return Poll::Ready(Ok(*offset));
            } else {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Seeking more than once is not supported"
                )));
            }
        }
        self.offset.set(new_offset).unwrap();
        return Poll::Ready(Ok(new_offset));
    }
}


impl SectorEncryptor {
    pub async fn new<T: ChunkTarget>(meta: SectorMeta, chunk_target: T, offset: u64) -> ChunkResult<Self> {
        let chunk_offset = std::cmp::max(offset, meta.header_length());
        Ok(Self {
            header_part: meta.encrypt_to_vec(),
            mut_part: RefCell::new(EncMutPart {
                offset,
                cached_result: OnceCell::new(),
                buffer: vec![0u8; Aes256::block_size()],
                read_offset_in_buffer: None,
                write_offset_in_buffer: None,
                encryptor: meta.encryptor_on_offset(chunk_offset)?,
                chunk_reader: Self::reader_of_chunks(&meta, &chunk_target, chunk_offset).await?,
            }),
            meta,
        })
    }

    pub fn seek_once<T: 'static + Unpin + ChunkTarget>(meta: SectorMeta, chunk_target: T) -> SeekOnceSectorEncryptor<T> {
        SeekOnceSectorEncryptor {
            offset: OnceCell::new(),
            reader_params: OnceCell::from((meta, chunk_target)),
            cached_result: OnceCell::new(),
            create_future: RefCell::new(None),
            reader: OnceCell::new(),
        }
    }

    async fn reader_of_chunks<T: ChunkTarget>(meta: &SectorMeta, chunk_target: &T, offset: u64) -> ChunkResult<Box<dyn Read + Unpin>> {
        struct ChunksReader {
            offset: u64,
            pedding_length: u64, 
            source_length: u64,
            chunk_index: usize,
            chunks: Vec<(Box<dyn Read + Unpin>, u64)>,
        }

        impl Read for ChunksReader {
            fn poll_read(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut [u8],
            ) -> Poll<std::io::Result<usize>> {
                let reader = self.get_mut();
                if reader.offset >= reader.pedding_length {
                    return Poll::Ready(Ok(0));
                }

                if reader.offset >= reader.source_length {
                    let read = u64::min(reader.pedding_length - reader.offset, buf.len() as u64) as usize;
                    buf[0..read].fill(0u8);
                    reader.offset += read as u64;
                    return Poll::Ready(Ok(read));
                }

                let (chunk_reader, chunk_upper_offset) = &mut reader.chunks[reader.chunk_index];

                match Pin::new(chunk_reader.as_mut()).poll_read(cx, &mut buf[..]) {
                    Poll::Ready(Ok(n)) => {
                        reader.offset += n as u64;
                        if reader.offset >= *chunk_upper_offset {
                            reader.chunk_index += 1;
                        }
                        return Poll::Ready(Ok(n));
                    }
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                }
             
            }
        }

        let (chunk_index, _, offset_range) = meta
            .chunk_on_offset(offset).ok_or(ChunkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset out of range")))?;
        let mut chunk_upper_offset = offset_range.start;
        let mut chunks = vec![];
        for (chunk_id, range) in meta.chunks()[chunk_index..].iter() {
            let mut chunk_reader = chunk_target.read(chunk_id).await?;
            if offset > chunk_upper_offset {
                chunk_reader.seek(SeekFrom::Start(offset - chunk_upper_offset)).await?;
            }
            chunk_upper_offset += range.end - range.start;
            chunks.push((Box::new(chunk_reader) as Box<dyn Read + Unpin>, chunk_upper_offset));
        }
        Ok(Box::new(ChunksReader {
            offset,
            pedding_length: meta.sector_length() - offset,
            source_length: meta.body_length(),
            chunk_index,
            chunks,
        }))
    }

    fn read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let mut mut_part = self.mut_part.borrow_mut();
        if let Some(result) = mut_part.cached_result.get() {
            match result {
                Ok(n) => {
                    return Poll::Ready(Ok(*n));
                },
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        } else if mut_part.offset < self.meta.header_length() {
            let read = if buf.len() < self.header_part.len() {
                buf.len()
            } else {
                self.header_part.len()
            };
            let offset = mut_part.offset as usize + read;
            buf[..read].copy_from_slice(&self.header_part[mut_part.offset as usize..offset]);
            mut_part.offset = offset as u64;
            return Poll::Ready(Ok(read));
        } else {
            let read = mut_part.check_read_buffer(buf);
            if read > 0 {
                mut_part.check_block_offset(&self.meta);
                return Poll::Ready(Ok(read));
            }
            if let Some(offset_in_buffer) = mut_part.write_offset_in_buffer.take() {
                if mut_part.fill_buffer(cx, offset_in_buffer).is_ready() {
                    let read = mut_part.check_read_buffer(buf);
                    mut_part.check_block_offset(&self.meta);
                    return Poll::Ready(Ok(read));
                } else {
                    return Poll::Pending;
                }
            }
            
            let read = if buf.len() % Aes256::block_size() == 0 {
                buf.len()
            } else {
                buf.len() / Aes256::block_size() * Aes256::block_size()
            };
            if read < Aes256::block_size() {
                if mut_part.fill_buffer(cx, 0).is_ready() {
                    let read = mut_part.check_read_buffer(buf);
                    mut_part.check_block_offset(&self.meta);
                    return Poll::Ready(Ok(read));
                } else {
                    return Poll::Pending;
                }      
            } else {
                match Pin::new(&mut mut_part.chunk_reader).poll_read(cx, &mut buf[..read]) {
                    Poll::Ready(Ok(n)) => {
                        for i in 0..n/Aes256::block_size() {
                            if let Some(encryptor) = &mut mut_part.encryptor {
                                encryptor.encrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut buf[i * Aes256::block_size()..(i + 1) * Aes256::block_size()]));
                            }
                            mut_part.offset += Aes256::block_size() as u64;
                            mut_part.check_block_offset(&self.meta);
                        }

                        let read = n / Aes256::block_size() * Aes256::block_size();
                        let remain_length = n % Aes256::block_size();
                        if remain_length != 0 {
                            mut_part.buffer.copy_from_slice(&buf[n - remain_length..n]);
                            mut_part.write_offset_in_buffer = Some(remain_length);
                        } 
                        return Poll::Ready(Ok(read));
                    }
                    Poll::Ready(Err(e)) => {
                        let _ = mut_part.cached_result.set(Err(std::io::Error::new(e.kind(), e.to_string())));
                        return Poll::Ready(Err(e));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
    }   
}

impl Read for SectorEncryptor {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.read(cx, buf)
    }
}