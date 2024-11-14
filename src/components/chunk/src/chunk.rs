use std::{fmt::Display, str::FromStr};
// 这里将包含chunk模块的实现
use sha2::{Sha256, Digest}; // 确保在 Cargo.toml 中添加 sha2 依赖
use base58::{ToBase58, FromBase58}; // 修改这行，添加 FromBase58
use serde::{Serialize, Deserialize};
use crate::error::*;


pub trait ChunkId: Clone + FromStr + Display + Send + Sync {
    fn length(&self) -> Option<u64> {
        None
    }
}

impl ChunkId for String {}



/// Represents a unique identifier for a chunk of data.
/// It consists of 32 bytes: 8 bytes for length and 24 bytes for a SHA256 hash prefix.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommonChunkId([u8; 32]);

impl ChunkId for CommonChunkId {
    /// Returns the length stored in the ChunkId.
    fn length(&self) -> Option<u64> {
        Some(i64::from_be_bytes(self.0[..8].try_into().unwrap()).abs() as u64)
    }
}

pub struct TempChunkId {

}

impl TempChunkId {
    pub fn with_hasher(length: i64, hasher: Sha256) -> ChunkResult<CommonChunkId> {
        let hash_result = hasher.finalize();
        let hash_array: [u8; 32] = hash_result.into();
        Self::with_hash(length, &hash_array)
    }

    pub fn with_hash(length: i64, sha256: &[u8; 32]) -> ChunkResult<CommonChunkId> {
        let mut id = CommonChunkId::with_hash(length, sha256)?;
        // 将第一位设置为1
        id.as_mut()[0] |= 0x80;
        Ok(id)
    }
}

pub type NormalChunkId = CommonChunkId;

impl NormalChunkId {
    pub fn with_data(data: &[u8]) -> ChunkResult<Self> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        Self::with_hasher(data.len() as i64, hasher)
    }
    /// Creates a new ChunkId with the given length and full SHA256 hash.
    /// Only the first 24 bytes of the hash are used.
    pub fn with_hash(length: i64, sha256: &[u8; 32]) -> ChunkResult<Self> {
        let mut id = [0u8; 32];
        let length = length.abs() as u64;
        id[..8].copy_from_slice(&length.to_be_bytes());
        id[8..].copy_from_slice(&sha256[..24]); // Only use the first 24 bytes of SHA256
        Ok(CommonChunkId(id))
    }

    /// Creates a new ChunkId with the given length and Sha256 hasher.
    pub fn with_hasher(length: i64, hasher: Sha256) -> ChunkResult<Self> {
        let hash_result = hasher.finalize();
        let hash_array: [u8; 32] = hash_result.into();
        Self::with_hash(length, &hash_array)
    }
}

impl CommonChunkId {
    pub fn is_temp(&self) -> bool {
        self.0[0] & 0x80 != 0
    }

    pub fn is_normal(&self) -> bool {
        self.0[0] & 0x80 == 0
    }

    pub fn as_normal(&self) -> Option<&NormalChunkId> {
        if self.is_normal() {
            Some(&self)
        } else {
            None
        }
    }


    /// Returns a new Sha256 hasher.
    pub fn hasher() -> Sha256 {
        Sha256::new()
    }
}

/// Allows ChunkId to be used as a reference to [u8; 32].
impl AsRef<[u8; 32]> for CommonChunkId {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Allows ChunkId to be used as a mutable reference to [u8; 32].
impl AsMut<[u8; 32]> for CommonChunkId {
    fn as_mut(&mut self) -> &mut [u8; 32] {
        &mut self.0
    }
}

/// Implements Display trait for ChunkId, representing it as a Base58 string.
impl Display for CommonChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_base58())
    }
}

/// Implements FromStr trait for ChunkId, allowing creation from a Base58 string.
impl FromStr for CommonChunkId {
    type Err = ChunkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.from_base58().map_err(|_| ChunkError::InvalidId(format!("Base58 string {}", s)))?;
        if bytes.len() != 32 {
            return Err(ChunkError::InvalidId(format!("bytes length: {}", bytes.len())));
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(CommonChunkId(array))
    }
}


