mod common;

use sector::{SectorBuilder, SectorEncryptor};
use chunk::{ChunkId, ChunkTarget};
use async_std::io::ReadExt;

#[async_std::test]
async fn test_sector_encryption() {
    let test_dir = common::setup_test_env().await;
    let store = common::create_test_local_store(&test_dir);
    
    // 创建测试数据
    let chunk_id = ChunkId::with_length(1024).unwrap();
    let test_data = vec![1u8; 1024];
    
    // 写入测试数据到本地存储
    store.write(&chunk_id, 0, &test_data[..], Some(1024 as u64)).await.unwrap();
    
    // 构建扇区
    let mut builder = SectorBuilder::new();
    builder.add_chunk(chunk_id.clone(), 0..1024);
    let meta = builder.build();
    
    // 创建加密器
    let mut encryptor = SectorEncryptor::new(meta, store, 0).await.unwrap();
    
    // 读取加密后的数据
    let mut encrypted = Vec::new();
    encryptor.read_to_end(&mut encrypted).await.unwrap();
    
    // 验证加密后的数据长度
    assert_eq!(encrypted.len(), 1024);
    // 验证加密后的数据与原始数据不同
    assert_ne!(encrypted, test_data);

    common::cleanup_test_env(test_dir).await;
} 