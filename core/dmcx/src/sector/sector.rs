use std::cell::{OnceCell, RefCell};
use std::future::Future;
use std::sync::Arc;
use std::{io::SeekFrom, ops::Range};
use std::pin::Pin;
use async_std::io::prelude::*;
use async_std::{future, task};
use generic_array::typenum::{U16, U32};
use generic_array::GenericArray;
use std::task::{Context, Poll, Waker};
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


