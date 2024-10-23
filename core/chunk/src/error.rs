use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChunkError {
    #[error("invalid chunk id format: {0}")]
    InvalidId(String), 
    #[error("I/O error occurred: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP Error: {0}")]
    Http(String),
    #[error("unknown chunk error")]
    Unknown,
}



/// 定义一个Result类型别名，用于简化错误处理
pub type ChunkResult<T> = std::result::Result<T, ChunkError>;
