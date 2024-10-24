use std::cell::{OnceCell, RefCell};
use std::{io::SeekFrom, ops::Range};
use std::pin::Pin;
use async_std::io::prelude::*;
use generic_array::typenum::{U16, U32};
use generic_array::GenericArray;
use std::task::{Context, Poll};
use aes::Aes256;
use sha2::{Sha256, Digest}; 
use cipher::{Block, BlockEncryptMut, BlockSizeUser, Iv, KeyIvInit};
use chunk::*;

const SECTOR_MAGIC: u64 = 0x444d4358;
const SECTOR_VERSION_0: u16 = 0;
const SECTOR_FLAGS_DEFAULT: u32 = 0;

const SECTOR_KEY_SIZE: usize = 32;
pub type SectorKey = GenericArray<u8, U32>;

#[derive(Clone, Copy, Debug, Default)]
pub struct SectorHeader {
    pub version: u32,
    pub flags: u32,
    pub key: Option<SectorKey>,
}

pub struct SectorMeta {
    key: Option<SectorKey>,
    chunks: Vec<(ChunkId, Range<u64>)>,

    id: ChunkId,
    header_length: u64,
    body_length: u64,
    sector_length: u64,
}

impl SectorMeta {
    pub fn new(key: Option<SectorKey>, chunks: Vec<(ChunkId, Range<u64>)>) -> Self {
        let header_length = 0;
        let body_length = chunks.iter().map(|(_, range)| range.end - range.start).sum();
        let sector_length = header_length + body_length;
        let sector_length = if sector_length % Aes256::block_size() as u64 != 0 {
            sector_length / Aes256::block_size() as u64 * Aes256::block_size() as u64 + Aes256::block_size() as u64
        } else {
            sector_length
        };
        let mut hasher = Sha256::new();
        
        if let Some(key) = &key {
            hasher.update(key);
        }
        
        // 添加所有chunk的信息到哈希计算中
        for (chunk_id, range) in &chunks {
            hasher.update(chunk_id.as_ref());
            hasher.update(&range.start.to_be_bytes());
            hasher.update(&range.end.to_be_bytes());
        }
        let id = ChunkId::with_hasher(sector_length, hasher).unwrap();


        Self {
            key,
            chunks,

            header_length,
            body_length,
            sector_length,
            id,
        }
    }

    pub fn sector_id(&self) -> &ChunkId {
        &self.id
    }

    pub fn block_size(&self) -> usize {
        16 * 1024 
    }

    pub fn encryptor_on_offset(&self, offset: u64) -> ChunkResult<Option<cbc::Encryptor<Aes256>>> {
        if let Some(iv) = self.iv_on_offset(offset)? {
            Ok(Some(cbc::Encryptor::<Aes256>::new(self.key.as_ref().unwrap(), &iv)))
        } else {
            Ok(None)
        }
    }

    fn iv_on_offset(&self, offset: u64) -> ChunkResult<Option<Iv<cbc::Encryptor<Aes256>>>> {
        if let Some(_) = &self.key {
            if offset < self.header_length {
                Ok(Some(GenericArray::<u8, U16>::from_slice(&[0u8; 16]).clone()))
            } else {
                Ok(Some(GenericArray::<u8, U16>::from_slice(&[0u8; 16]).clone()))
            }
        } else {
            Ok(None)
        }
    }

    pub fn chunks(&self) -> &Vec<(ChunkId, Range<u64>)> {
        &self.chunks
    }

    pub fn sector_length(&self) -> u64 {
        self.sector_length
    }

    pub fn header_length(&self) -> u64 {
        self.header_length
    }

    pub fn body_length(&self) -> u64 {
        self.body_length
    }

    pub fn chunk_on_offset(&self, offset: u64) -> Option<(usize, u64, Range<u64>)> {
        let offset = offset - self.header_length;
        let mut in_offset = 0;
        for (i, (_, range)) in self.chunks.iter().enumerate() {
            if offset >= in_offset && offset < in_offset + range.end - range.start  {
                if i == self.chunks.len() - 1 {
                    return Some((i, offset - in_offset, range.start + in_offset..self.sector_length));
                } else {
                    return Some((i, offset - in_offset, range.start + in_offset..range.end));
                }
            }
            in_offset += range.end - range.start;
        }
        None
    }

    pub fn encrypt_to_vec(&self) -> Vec<u8> {
       todo!()
    }
}



pub struct SectorBuilder {
    length_limit: u64,
    length: u64, 
    chunks: Vec<(ChunkId, Range<u64>)>,
}

impl SectorBuilder {
    pub fn new() -> Self {
        Self {
            length_limit: u64::MAX,
            length: 0,
            chunks: Vec::new(),
        }
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn length_limit(&self) -> u64 {
        self.length_limit
    }

    pub fn chunks(&self) -> &Vec<(ChunkId, Range<u64>)> {
        &self.chunks
    }

    pub fn set_length_limit(&mut self, length_limit: u64) -> &mut Self {
        if self.chunks.len() > 0 {
            assert!(false, "length_limit must be greater than the largest chunk");
            return self;
        }
        self.length_limit = length_limit;
        self
    }

    pub fn add_chunk(&mut self, chunk_id: ChunkId, range: Range<u64>) -> u64 {
        if self.length >= self.length_limit {
            return 0;
        }
        let length = range.end - range.start;
        let length = if self.length + length > self.length_limit {
            self.length_limit - self.length
        } else {
            length
        };
        self.length += length;
        self.chunks.push((chunk_id, range.start..(range.start + length)));
        length
    }
}



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


impl SectorEncryptor {
    pub async fn new<T: ChunkTarget>(meta: SectorMeta, chunk_target: T, offset: u64) -> ChunkResult<Self> {
        let chunk_offset = if offset < meta.header_length() {
            meta.header_length()
        } else {
            offset
        };
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
        for (chunk_id, range) in meta.chunks[chunk_index..].iter() {
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

