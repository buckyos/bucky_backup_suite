
use std::{collections::LinkedList, future::Future, io::SeekFrom, ops::Range, pin::Pin, sync::{Mutex}, task::{Context, Poll}};
use aes::Aes256;
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf, AsyncSeekExt, AsyncReadExt};
use cipher::{Block, BlockDecryptMut, BlockSizeUser};
use chunk::*;
use crate::SectorMeta;


struct DecReadProc<T: ChunkTarget> {
    buffer: Vec<u8>, 
    read_offset_in_buffer: Option<usize>, 
    write_offset_in_buffer: Option<usize>,
    decryptor: Option<cbc::Decryptor<Aes256>>, 
    sector_reader: T::ChunkRead,
}

impl<T: ChunkTarget> DecReadProc<T> {
    fn check_read_buffer(&mut self, buf: &mut ReadBuf<'_>) -> usize {
        if let Some(offset_in_buffer) = self.read_offset_in_buffer.take() {
            let remain_len = self.buffer.len() - offset_in_buffer;
            let read = if buf.remaining() < remain_len {
                buf.remaining()
            } else {
                remain_len
            };
            buf.put_slice(&self.buffer[offset_in_buffer..offset_in_buffer + read]);
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
            match Pin::new(&mut self.sector_reader).poll_read(cx, &mut buf) {
                Poll::Ready(Ok(_)) => {
                    let n = buf.filled().len() - before;
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
    decryptor: Option<cbc::Decryptor<Aes256>>,
    buffer: Vec<u8>, 
    sector_reader: T::ChunkRead,
}

impl<T: ChunkTarget> DecSeekProc<T> {
    async fn seek(mut self, block_size: u64) -> std::io::Result<DecSeekProc<T>> {
        let block_offset = self.dest_offset / block_size * block_size;
        self.sector_reader.seek(SeekFrom::Start(block_offset)).await?;
        self.sector_reader.read_exact(&mut self.buffer[..]).await?;
        if let Some(decryptor) = &mut self.decryptor {
            decryptor.decrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut self.buffer[..]));
        }
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
        if self.offset % meta.header().block_size as u64 != 0 {
            return;
        }
        self.read_proc.as_mut().unwrap().decryptor = meta.decryptor_on_offset(self.offset).unwrap();
    }

    fn check_read_buffer(&mut self, buf: &mut ReadBuf<'_>) -> usize {
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
                sector_reader: remote_sectors.read(meta.sector_id()).await?
                    .ok_or(ChunkError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "sector not found")))?, 
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

    fn start_seek(&mut self, offset: u64) -> std::io::Result<()> {
        let mut mut_part = self.mut_part.lock().unwrap();
       
        if let Some((dest_offset, _)) = &mut_part.seek_proc {
            if *dest_offset != offset {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "another seek is in progress"));
            }
        } else {
            let read_proc = mut_part.read_proc.take().unwrap();
            let seek_proc = DecSeekProc {
                dest_offset: offset,
                read_offset_in_buffer: 0,
                buffer: read_proc.buffer,
                decryptor: self.meta.decryptor_on_offset(offset).unwrap(),
                sector_reader: read_proc.sector_reader,
            };
            mut_part.seek_proc = Some((offset, Box::pin(seek_proc.seek(aes::Aes256::block_size() as u64))));
        };
        Ok(())
    }

    fn complete_seek(&mut self, cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        let mut mut_part = self.mut_part.lock().unwrap();
        let (offset, mut future) = mut_part.seek_proc.take().unwrap();
        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(seek_proc)) => {
                mut_part.read_proc = Some(DecReadProc {
                    buffer: seek_proc.buffer,
                    read_offset_in_buffer: Some(seek_proc.read_offset_in_buffer),
                    write_offset_in_buffer: None,
                    decryptor: seek_proc.decryptor,
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

    fn read(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let mut mut_part = self.mut_part.lock().unwrap();
        if mut_part.offset < self.meta.header_length() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek to header")));
        } else if mut_part.read_proc.is_none() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek is in progress")));
        } else if let Some(result) = &mut_part.cached_result {
            match result {
                Ok(_) => {
                    return Poll::Ready(Ok(()));
                },
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        } else {
            let read = mut_part.check_read_buffer(buf);
            if read > 0 {
                mut_part.check_block_offset(&self.meta);
                return Poll::Ready(Ok(()));
            }

            if let Some(offset_in_buffer) = mut_part.read_proc.as_mut().unwrap().write_offset_in_buffer.take() {
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
                match Pin::new(&mut mut_part.read_proc.as_mut().unwrap().sector_reader).poll_read(cx, buf) {
                    Poll::Ready(Ok(())) => {
                        let n = buf.filled().len() - before;
                        for i in 0..n/Aes256::block_size() {
                            if let Some(decryptor) = &mut mut_part.read_proc.as_mut().unwrap().decryptor {
                                decryptor.decrypt_block_mut(Block::<Aes256>::from_mut_slice(&mut buf.filled_mut()[before + i * Aes256::block_size()..before + (i + 1) * Aes256::block_size()]));
                            }
                            mut_part.offset += Aes256::block_size() as u64;
                            mut_part.check_block_offset(&self.meta);
                        }

                        let remain_length = n % Aes256::block_size();
                        if remain_length != 0 {
                            mut_part.read_proc.as_mut().unwrap().buffer.copy_from_slice(&buf.filled()[before + n - remain_length..before + n]);
                            mut_part.read_proc.as_mut().unwrap().write_offset_in_buffer = Some(remain_length);
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

impl<T: 'static + ChunkTarget> AsyncRead for SectorDecryptor<T> {
    fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.read(cx, buf)
    }
}


impl<T: 'static + ChunkTarget> AsyncSeek for SectorDecryptor<T> {
    fn start_seek(self: Pin<&mut Self>, pos: SeekFrom) -> std::io::Result<()> {
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
        this.start_seek(offset)
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        let this = self.get_mut();
        this.complete_seek(cx)
    }
}

struct SectorStub<T: ChunkTarget> {
    offset_in_sector: u64,
    chunk_range: Range<u64>,
    reader: SectorDecryptor<T>,
}

pub struct ChunkDecryptor<T: ChunkTarget> {
    chunk: String, 
    length: u64,
    offset: u64, 
    cached_result: Option<std::io::Result<usize>>,
    cur_stub: Option<SectorStub<T>>, 
    stubs: LinkedList<SectorStub<T>>,
}


impl<T: 'static + ChunkTarget> ChunkDecryptor<T> {
    pub async fn new(chunk: String, length: u64, metas: Vec<SectorMeta>, chunk_target: &T) -> ChunkResult<Self> {
        let mut stubs = LinkedList::new();
        for meta in metas {
            let (offset_in_sector, chunk_range) = meta.offset_of_chunk(&chunk).unwrap();
            let reader = SectorDecryptor::new(meta,chunk_target).await?;
            stubs.push_back(SectorStub {
                offset_in_sector,
                chunk_range,
                reader,
            });
        }
        Ok(Self {
            chunk,
            length, 
            offset: 0,
            cached_result: None,
            cur_stub: None,
            stubs,
        })
    }

    fn seek(&mut self, offset: u64) -> std::io::Result<()> {
        if let Some(stub) = self.cur_stub.take() {
            self.stubs.push_back(stub);
        }
        self.offset = offset;
        Ok(())
    }

    fn read(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        if let Some(result) = &self.cached_result {
            match result {
                Ok(_) => return Poll::Ready(Ok(())),
                Err(e) => return Poll::Ready(Err(std::io::Error::new(e.kind(), e.to_string()))),
            }
        }
        if self.cur_stub.is_none() {
            let mut stub_index = None;
            for (i, stub) in self.stubs.iter().enumerate() {
                if self.offset >= stub.chunk_range.start && self.offset < stub.chunk_range.end {
                    stub_index = Some(i);
                    break;
                }
            }
            if let Some(index) = stub_index {
                let mut split = self.stubs.split_off(index);
                self.cur_stub = split.pop_front();
                self.stubs.append(&mut split);
            } else {
                return Poll::Ready(Ok(()));
            }
        };
        let stub = self.cur_stub.as_mut().unwrap();

        let offset_in_sector = stub.offset_in_sector + self.offset - stub.chunk_range.start;
        if stub.reader.offset() != offset_in_sector {
            stub.reader.start_seek(offset_in_sector).unwrap();
            match Pin::new(&mut stub.reader).complete_seek(cx) {
                Poll::Ready(Err(e)) => {
                    self.cached_result = Some(Err(std::io::Error::new(e.kind(), e.to_string())));
                    return Poll::Ready(Err(e));
                }
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(_)) => {},
            }
        }

        let before = buf.filled().len();
        match Pin::new(&mut stub.reader).poll_read(cx, buf) {
            Poll::Ready(Ok(_)) => {
                let n = buf.filled().len() - before;
                self.offset += n as u64;
                if self.offset >= stub.chunk_range.end {
                    self.stubs.push_back(self.cur_stub.take().unwrap());
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                self.cached_result = Some(Err(std::io::Error::new(e.kind(), e.to_string())));
                Poll::Ready(Err(e))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T: 'static + ChunkTarget> AsyncRead for ChunkDecryptor<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {  
        self.get_mut().read(cx, buf)
    }
}

impl<T: 'static + ChunkTarget> AsyncSeek for ChunkDecryptor<T> {
    fn start_seek(self: Pin<&mut Self>, pos: SeekFrom) -> std::io::Result<()> {
        let this = self.get_mut();
        let offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {this.length - offset as u64},
            SeekFrom::Current(offset) => {
                if offset > 0 {
                    this.offset + offset as u64
                } else {
                    this.offset - offset.abs() as u64
                }
            },
        };
        this.seek(offset)
    }

    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.offset))
    }
}