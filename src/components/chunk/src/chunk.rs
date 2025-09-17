use base58::{ToBase58, FromBase58};
use sha2::{Sha256, Digest};
use tokio::io::{self, AsyncRead, AsyncSeek, AsyncSeekExt, AsyncReadExt};
use std::io::SeekFrom;

pub struct FullHasher {
    hasher: Sha256,
}

impl FullHasher {
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }

    pub async fn update_from_reader<T: AsyncRead + Unpin>(&mut self, reader: &mut T) -> io::Result<()> {
        let mut buffer = vec![0u8; 4096];
        loop {
            let n = reader.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            self.hasher.update(&buffer[..n]);
        }
        Ok(())
    }

    pub async fn calc_from_reader<T: AsyncRead + Unpin>(reader: &mut T) -> io::Result<String> {
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 4096];
        loop {
            let n = reader.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(hasher.finalize().to_vec().to_base58())
    }

    pub fn update_from_bytes(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    pub fn calc_from_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize().to_vec().to_base58()
    }

    pub fn finalize(self) -> String {
        self.hasher.finalize().to_vec().to_base58()
    }
}

pub struct QuickHasher {
    // 采样块大小
    block_size: usize,
    // 最大采样数
    max_samples: usize,
}

impl Default for QuickHasher {
    fn default() -> Self {
        Self {
            block_size: 4096,        // 4KB 块
            max_samples: 100,         // 最多采样 100 个点
        }
    }
}

impl QuickHasher {
    pub async fn calc<T: AsyncRead + AsyncSeek + Unpin>(&self, reader: &mut T, length: Option<u64>) -> io::Result<String> {
        let length = if let Some(length) = length {
            length
        } else {
            let length = reader.seek(SeekFrom::End(0)).await?;
            reader.seek(SeekFrom::Start(0)).await?;
            length
        };

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; length as usize];
        if length < self.block_size as u64 {
            reader.read_exact(&mut buffer[..length as usize]).await?;
            hasher.update(&buffer[..length as usize]);
        } else {
            let mut offset = 0;
            let interval = (length / self.max_samples as u64)
                .max(self.block_size as u64);
            let last_offset = length - offset;
            loop {
                reader.read_exact(&mut buffer).await?;
                hasher.update(&buffer);
                offset += interval;
                if offset >= last_offset {
                    break;
                }
                reader.seek(SeekFrom::Start(offset)).await?;
            }
            
            reader.seek(SeekFrom::Start(last_offset)).await?;
            reader.read_exact(&mut buffer).await?;
            hasher.update(&buffer);
        }

        Ok(hasher.finalize().to_vec().to_base58())
    }
}