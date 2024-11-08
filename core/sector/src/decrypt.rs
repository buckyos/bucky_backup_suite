use std::{collections::LinkedList, future::Future, io::SeekFrom, ops::Range, pin::Pin, sync::{Arc, Mutex}, task::{Context, Poll}};
use aes::Aes256;
use async_std::io::prelude::*;
use cipher::{Block, BlockDecryptMut, BlockSizeUser};
use chunk::{ChunkId, ChunkResult, ChunkTarget};
use crate::SectorMeta;


struct DecReadProc<T: ChunkTarget> {
    buffer: Vec<u8>, 
    read_offset_in_buffer: Option<usize>, 
    write_offset_in_buffer: Option<usize>,
    decryptor: Option<cbc::Decryptor<Aes256>>, 
    sector_reader: T::Read,
}

impl<T: ChunkTarget> DecReadProc<T> {
    fn check_read_buffer(&mut self, buf: &mut [u8]) -> usize {
        if let Some(offset_in_buffer) = self.read_offset_in_buffer.take() {
            let remain_len = self.buffer.len() - offset_in_buffer;
            let read = if buf.len() < remain_len {
                buf.len()
            } else {
                remain_len
            };
            buf[..read].copy_from_slice(&self.buffer[offset_in_buffer..offset_in_buffer + read]);
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
            match Pin::new(&mut self.sector_reader).poll_read(cx, &mut self.buffer[offset_in_buffer..]) {
                Poll::Ready(Ok(n)) => {
                    if offset_in_buffer + n == Aes256::block_size() {
                        self.write_offset_in_buffer = None;
                        if let Some(decryptor) = &mut self.decryptor {
                            decryptor.decrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut self.buffer[..]));
                        }
                        self.read_offset_in_buffer = Some(0);
                        return Poll::Ready(Ok(()));    
                    } else {
                        offset_in_buffer += n;
                    }
                }, 
                Poll::Ready(Err(e)) => {
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

struct DecSeekProc<T: ChunkTarget> {
    dest_offset: u64, 
    read_offset_in_buffer: usize, 
    decryptor: cbc::Decryptor<Aes256>,
    buffer: Vec<u8>, 
    sector_reader: T::Read,
}

impl<T: ChunkTarget> DecSeekProc<T> {
    async fn seek(mut self, block_size: u64) -> std::io::Result<DecSeekProc<T>> {
        let block_offset = self.dest_offset / block_size * block_size;
        self.sector_reader.seek(SeekFrom::Start(block_offset)).await?;
        self.sector_reader.read_exact(&mut self.buffer[..]).await?;
        self.decryptor.decrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut self.buffer[..]));
        self.read_offset_in_buffer = (self.dest_offset % block_size) as usize;
        Ok(self)
    }
}

struct DecMutPart<T: ChunkTarget> {
    offset: u64, 
    cached_result: Option<std::io::Result<usize>>, 
    read_proc: Option<DecReadProc<T>>,
    seek_proc: Option<(u64, Pin<Box<dyn Send + Future<Output = std::io::Result<DecSeekProc<T>>>>>)>,
}



impl<T: ChunkTarget> DecMutPart<T> {
    fn check_block_offset(&mut self, meta: &SectorMeta) {
        if self.offset < meta.header_length() {
            return;
        }
        if self.offset % meta.block_size() as u64 != 0 {
            return;
        }
        self.read_proc.as_mut().unwrap().decryptor = meta.decryptor_on_offset(self.offset).unwrap();
    }

    fn check_read_buffer(&mut self, buf: &mut [u8]) -> usize {
        let read = self.read_proc.as_mut().unwrap().check_read_buffer(buf);
        self.offset += read as u64;
        read
    }

    fn fill_buffer(&mut self, cx: &mut Context<'_>, offset_in_buffer: usize) -> Poll<std::io::Result<()>> {
        let result = self.read_proc.as_mut().unwrap().fill_buffer(cx, offset_in_buffer);
        if let Poll::Ready(Err(e)) = result {
            self.cached_result = Some(Err(std::io::Error::new(e.kind(), e.to_string())));
            Poll::Ready(Err(e))
        } else {
            result
        }
    }
}


pub struct SectorDecryptor<T: ChunkTarget> {
    meta: SectorMeta,
    mut_part: Mutex<DecMutPart<T>>,
}


impl<T: 'static + ChunkTarget> SectorDecryptor<T> {
    pub async fn new(meta: SectorMeta, remote_sectors: &T) -> ChunkResult<Self> {
        let mut_part = DecMutPart {
            offset: 0,
            cached_result: None, 
            read_proc: Some(DecReadProc {
                buffer: vec![0u8; Aes256::block_size()],
                read_offset_in_buffer: None,
                write_offset_in_buffer: None,
                decryptor: None,
                sector_reader: remote_sectors.read(meta.sector_id()).await?, 
            }),
            seek_proc: None,
        };

        Ok(Self {
            mut_part: Mutex::new(mut_part),
            meta,
        })
    }

    pub fn offset(&self) -> u64 {
        self.mut_part.lock().unwrap().offset
    }

    fn seek(&mut self, cx: &mut Context<'_>, offset: u64) -> Poll<std::io::Result<u64>> {
        let mut mut_part = self.mut_part.lock().unwrap();
        let mut future = if let Some((dest_offset, _)) = &mut_part.seek_proc {
            if *dest_offset != offset {
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek offset is not continuous")));
            } else {
                mut_part.seek_proc.take().unwrap().1
            }
        } else {
            let read_proc = mut_part.read_proc.take().unwrap();
            let seek_proc = DecSeekProc {
                dest_offset: offset,
                read_offset_in_buffer: 0,
                buffer: read_proc.buffer,
                decryptor: self.meta.decryptor_on_offset(offset).unwrap().unwrap(),
                sector_reader: read_proc.sector_reader,
            };
            Box::pin(seek_proc.seek(self.meta.block_size() as u64))
        };
        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(seek_proc)) => {
                mut_part.read_proc = Some(DecReadProc {
                    buffer: seek_proc.buffer,
                    read_offset_in_buffer: Some(seek_proc.read_offset_in_buffer),
                    write_offset_in_buffer: None,
                    decryptor: Some(seek_proc.decryptor),
                    sector_reader: seek_proc.sector_reader,
                });
                mut_part.offset = offset;
                Poll::Ready(Ok(offset))
            }
            Poll::Ready(Err(e)) => {
                mut_part.cached_result = Some(Err(std::io::Error::new(e.kind(), e.to_string())));
                Poll::Ready(Err(e))
            }, 
            Poll::Pending => {
                mut_part.seek_proc = Some((offset, future));
                Poll::Pending
            }
        }
    }

    fn read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let mut mut_part = self.mut_part.lock().unwrap();
        if mut_part.offset < self.meta.header_length() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek to header")));
        } else if mut_part.read_proc.is_none() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek is in progress")));
        } else if let Some(result) = &mut_part.cached_result {
            match result {
                Ok(n) => {
                    return Poll::Ready(Ok(*n));
                },
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        } else {
            let read = mut_part.check_read_buffer(buf);
            if read > 0 {
                mut_part.check_block_offset(&self.meta);
                return Poll::Ready(Ok(read));
            }

            if let Some(offset_in_buffer) = mut_part.read_proc.as_mut().unwrap().write_offset_in_buffer.take() {
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
                match Pin::new(&mut mut_part.read_proc.as_mut().unwrap().sector_reader).poll_read(cx, &mut buf[..read]) {
                    Poll::Ready(Ok(n)) => {
                        for i in 0..n/Aes256::block_size() {
                            if let Some(decryptor) = &mut mut_part.read_proc.as_mut().unwrap().decryptor {
                                decryptor.decrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut buf[i * Aes256::block_size()..(i + 1) * Aes256::block_size()]));
                            }
                            mut_part.offset += Aes256::block_size() as u64;
                            mut_part.check_block_offset(&self.meta);
                        }

                        let read = n / Aes256::block_size() * Aes256::block_size();
                        let remain_length = n % Aes256::block_size();
                        if remain_length != 0 {
                            mut_part.read_proc.as_mut().unwrap().buffer.copy_from_slice(&buf[n - remain_length..n]);
                            mut_part.read_proc.as_mut().unwrap().write_offset_in_buffer = Some(remain_length);
                        } 
                        return Poll::Ready(Ok(read));
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

impl<T: 'static + ChunkTarget> Read for SectorDecryptor<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.read(cx, buf)
    }
}


impl<T: 'static + ChunkTarget> Seek for SectorDecryptor<T> {
    fn poll_seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<std::io::Result<u64>> {
        let this = self.get_mut();
        let offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {this.meta.sector_length() - offset as u64},
            SeekFrom::Current(offset) => {
                if offset > 0 {
                    this.mut_part.lock().unwrap().offset + offset as u64
                } else {
                    this.mut_part.lock().unwrap().offset - offset.abs() as u64
                }
            },
        };
        this.seek(cx, offset)
    }
}

struct SectorStub<T: ChunkTarget> {
    range: Range<u64>,
    reader: SectorDecryptor<T>,
}

pub struct ChunkDecryptor<T: ChunkTarget> {
    offset: u64,
    cur_stub: Option<SectorStub<T>>, 
    stubs: LinkedList<SectorStub<T>>,
}


impl<T: 'static + ChunkTarget> ChunkDecryptor<T> {
    pub async fn new(chunk: ChunkId, metas: Vec<SectorMeta>, chunk_target: &T) -> ChunkResult<Self> {
        let mut stubs = vec![];
        for meta in metas {
            let range = meta.chunks().iter().find(|(id, _)| *id == chunk).map(|(_, range)| range.clone()).unwrap();
            let reader = SectorDecryptor::new(meta,chunk_target).await?;
            stubs.push(SectorStub {
                range,
                reader,
            });
        }
        Ok(Self {
            offset: 0,
            stubs,
        })
    }
}

impl<T: 'static + ChunkTarget> Read for ChunkDecryptor<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {  
        let this = self.get_mut();
        let mut stub_index = None;
        for (i, stub) in this.stubs.iter().enumerate() {
            if this.offset >= stub.range.start && this.offset < stub.range.end {
                stub_index = Some(i);
                break;
            }
        }

        let stub_index = if let Some(index) = stub_index {
            index
        } else {
            return Poll::Ready(Ok(0));
        };

        let stub = &mut this.stubs[stub_index];
        if stub.reader.offset() != this.offset {
            match Pin::new(&mut stub.reader).poll_seek(cx, SeekFrom::Start(this.offset)) {
                Poll::Ready(Ok(_)) => {},
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        let remain_len = stub.range.end - this.offset;
        let read_len = if buf.len() as u64 > remain_len {
            remain_len as usize
        } else {
            buf.len()
        };

        match Pin::new(&mut stub.reader).poll_read(cx, &mut buf[..read_len]) {
            Poll::Ready(Ok(n)) => {
                this.offset += n as u64;
                Poll::Ready(Ok(n))
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// impl Seek for SectorDecryptor {
//     fn poll_seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<std::io::Result<u64>> {
//         todo!()
//     }
// }

