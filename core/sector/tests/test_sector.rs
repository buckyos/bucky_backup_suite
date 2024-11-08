mod common;

use sector::{SectorBuilder, SectorMeta};
use chunk::ChunkId;
use std::ops::Range;

#[async_std::test]
async fn test_sector_builder() {
    let test_dir = common::setup_test_env().await;
    
    let mut builder = SectorBuilder::new();
    
    // 添加一些测试数据块
    let chunk1 = ChunkId::with_length(1024).unwrap();
    let chunk2 = ChunkId::with_length(2048).unwrap();
    
    builder.add_chunk(chunk1.clone(), 0..1024);
    builder.add_chunk(chunk2.clone(), 0..2048);
    
    let meta = builder.build();
    
    // 验证构建的扇区元数据
    assert_eq!(meta.sector_length(), 3072);
    assert_eq!(meta.body_length(), 3072);
    assert_eq!(meta.header_length(), 0);
    
    let chunks = meta.chunks();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].0, chunk1);
    assert_eq!(chunks[0].1, 0..1024);
    assert_eq!(chunks[1].0, chunk2); 
    assert_eq!(chunks[1].1, 0..2048);

    common::cleanup_test_env(test_dir).await;
}

#[async_std::test]
async fn test_sector_builder_with_limit() {
    let test_dir = common::setup_test_env().await;
    
    let mut builder = SectorBuilder::new().with_length_limit(2048);
    
    let chunk1 = ChunkId::with_length(1024).unwrap();
    let chunk2 = ChunkId::with_length(2048).unwrap();
    
    // 第一个块应该完全添加
    let added = builder.add_chunk(chunk1.clone(), 0..1024);
    assert_eq!(added, 1024);
    
    // 第二个块应该只添加部分
    let added = builder.add_chunk(chunk2.clone(), 0..2048);
    assert_eq!(added, 1024); // 只添加到达限制的部分
    
    let meta = builder.build();
    assert_eq!(meta.sector_length(), 2048);

    common::cleanup_test_env(test_dir).await;
} 