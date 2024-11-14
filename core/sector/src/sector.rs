use std::ops::Range;
use generic_array::typenum::{U16, U32};
use generic_array::GenericArray;
use aes::Aes256;
use sha2::{Sha256, Digest}; 
use cipher::{BlockSizeUser, Iv, KeyIvInit};
use chunk::*;

const SECTOR_MAGIC: u64 = 0x444d4358;
const SECTOR_VERSION_0: u32 = 0;
const SECTOR_FLAGS_DEFAULT: u32 = 0;

const SECTOR_KEY_SIZE: usize = 32;
pub type SectorKey = GenericArray<u8, U32>;

#[derive(Clone)]
pub struct SectorHeader {
    pub version: u32,
    pub flags: u32, 
    pub block_size: u16,
    pub key: Option<SectorKey>,
    pub chunks: Vec<(ChunkId, Range<u64>)>,
    pub reserved: [u8; 12],
}

impl Default for SectorHeader {
    fn default() -> Self {
        Self {
            version: SECTOR_VERSION_0,
            flags: SECTOR_FLAGS_DEFAULT,
            block_size: 16 * 1024,
            key: None,
            chunks: Vec::new(),
            reserved: [0u8; 12],
        }
    }
}

impl SectorHeader {
    fn calc_length(&self) -> usize {
        let length = size_of::<u64>()    // SECTOR_MAGIC
        + size_of::<u32>()  // version
        + size_of::<u32>()  // flags
        + size_of::<u16>() // block_size
        + if self.key.is_some() {
            SECTOR_KEY_SIZE // key length
        } else {
            0
        }
        + size_of::<u16>() // length of chunks
        + self.chunks.iter().map(|(..)| {
            size_of::<ChunkId>() + size_of::<u64>() + size_of::<u64>()
        }).sum::<usize>()
        + self.reserved.len(); // reserved
        assert_eq!(length % Aes256::block_size() as usize, 0);
        length as usize
    }

    pub fn encrypt_to_vec(&self) -> Vec<u8> {
        let mut result = Vec::new();
        
        // 写入魔数
        result.extend_from_slice(&SECTOR_MAGIC.to_be_bytes());
        
        result.extend_from_slice(&self.version.to_be_bytes());

        result.extend_from_slice(&self.flags.to_be_bytes());

        result.extend_from_slice(&self.block_size.to_be_bytes());

        if let Some(key) = &self.key {
            result.extend_from_slice(key);
        }
    
        result.extend_from_slice(&(self.chunks.len() as u16).to_be_bytes());
        
        // 写入每个块的信息
        for (chunk_id, range) in &self.chunks {
            // 写入块ID
            result.extend_from_slice(chunk_id.as_ref());
            // 写入范围
            result.extend_from_slice(&range.start.to_be_bytes());
            result.extend_from_slice(&range.end.to_be_bytes());
        }

        result.extend_from_slice(&self.reserved);
        
        result
    }
}

pub struct ChunkOnOffset {
    pub chunk_index: usize,
    pub range_in_sector: Range<u64>,
    pub range_in_chunk: Range<u64>,
}

#[derive(Clone)]
pub struct SectorMeta {
    header: SectorHeader,
    id: ChunkId,
    header_length: u64,
    body_length: u64,
    sector_length: u64,
}

impl SectorMeta {
    pub fn new(header: SectorHeader) -> Self {
        let header_length = header.calc_length() as u64;
        let body_length = header.chunks.iter().map(|(_, range)| range.end - range.start).sum();
        let sector_length = header_length + body_length;
        let sector_length = if sector_length % Aes256::block_size() as u64 != 0 {
            sector_length / Aes256::block_size() as u64 * Aes256::block_size() as u64 + Aes256::block_size() as u64
        } else {
            sector_length
        };
        let mut hasher = Sha256::new();
        
        if let Some(key) = &header.key {
            hasher.update(key);
        }
        
        // 添加所有chunk的信息到哈希计算中
        for (chunk_id, range) in &header.chunks {
            hasher.update(chunk_id.as_ref());
            hasher.update(&range.start.to_be_bytes());
            hasher.update(&range.end.to_be_bytes());
        }
        let id = ChunkId::with_hasher(sector_length as i64, hasher).unwrap();

        Self {
            header,
            header_length,
            body_length,
            sector_length,
            id,
        }
    }

    pub fn header(&self) -> &SectorHeader {
        &self.header
    }

    pub fn sector_id(&self) -> &ChunkId {
        &self.id
    }

    pub fn encryptor_on_offset(&self, offset: u64) -> ChunkResult<Option<cbc::Encryptor<Aes256>>> {
        if let Some(iv) = self.iv_on_offset(offset)? {
            Ok(Some(cbc::Encryptor::<Aes256>::new(self.header.key.as_ref().unwrap(), &iv)))
        } else {
            Ok(None)
        }
    }

    pub fn decryptor_on_offset(&self, offset: u64) -> ChunkResult<Option<cbc::Decryptor<Aes256>>> {
        if let Some(iv) = self.iv_on_offset(offset)? {
            Ok(Some(cbc::Decryptor::<Aes256>::new(self.header.key.as_ref().unwrap(), &iv)))
        } else {
            Ok(None)
        }
    }

    fn iv_on_offset(&self, _: u64) -> ChunkResult<Option<Iv<cbc::Encryptor<Aes256>>>> {
        if let Some(_) = &self.header.key {
            Ok(Some(GenericArray::<u8, U16>::from_slice(&[0u8; 16]).clone()))
        } else {
            Ok(None)
        }
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

    pub fn chunk_on_offset(&self, offset: u64) -> Option<ChunkOnOffset> {
        let offset_in_chunks = offset - self.header_length;
        let mut start_offset_in_chunks = 0;
        for (i, (_, range)) in self.header.chunks.iter().enumerate() {
            if offset_in_chunks >= start_offset_in_chunks && offset < start_offset_in_chunks + range.end - range.start  {
                if i == self.header.chunks.len() - 1 {
                    return Some(ChunkOnOffset {
                        chunk_index: i,
                        range_in_sector: self.header_length + start_offset_in_chunks..self.sector_length,
                        range_in_chunk: range.clone(),
                    });
                } else {
                    return Some(ChunkOnOffset {
                        chunk_index: i,
                        range_in_sector: self.header_length + start_offset_in_chunks..self.header_length + start_offset_in_chunks + range.end - range.start,
                        range_in_chunk: range.clone(),
                    });
                }
            }
            start_offset_in_chunks += range.end - range.start;
        }
        None
    }

    pub fn offset_of_chunk(&self, chunk_id: &ChunkId) -> Option<(u64, Range<u64>)> {
        if let Some((index, _)) = self.header.chunks.iter().enumerate().find(|(_, (id, _))| id == chunk_id) {
            let offset: u64 = self.header.chunks[..index].iter().map(|(_, range)| range.end - range.start).sum();
            Some((self.header_length + offset, self.header.chunks[index].1.clone()))
        } else {
            None
        }
    }
   
}



pub struct SectorBuilder {
    length_limit: u64,
    length: u64, 
    header: SectorHeader,
}

impl SectorBuilder {
    pub fn new() -> Self {
        Self {
            length_limit: u64::MAX,
            length: 0,
            header: SectorHeader::default(),
        }
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn length_limit(&self) -> u64 {
        self.length_limit
    }

    pub fn with_key(mut self, key: Vec<u8>) -> Self {
        self.header.key = Some(SectorKey::clone_from_slice(&key[0..SECTOR_KEY_SIZE]));
        self
    }

    pub fn with_length_limit(mut self, length_limit: u64) -> Self {
        if self.header.chunks.len() > 0 {
            assert!(false, "length_limit must be greater than the largest chunk");
            return self;
        }
        self.length_limit = length_limit;
        self
    }

    pub fn with_block_size(mut self, block_size: u16) -> Self {
        self.header.block_size = block_size;
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
        self.header.chunks.push((chunk_id, range.start..(range.start + length)));
        length
    }

    pub fn build(self) -> SectorMeta {
        SectorMeta::new(self.header)
    }
}


