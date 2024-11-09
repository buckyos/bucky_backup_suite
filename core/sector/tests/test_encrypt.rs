mod common;
use sector::*;
use chunk::*;
use async_std::io::{prelude::*, BufReader};

#[async_std::test]
async fn test_sector_encryption() {
    let test_dir = common::setup_test_env().await;
    let chunk_store = common::create_test_local_store(&test_dir);
    let sector_store = common::create_test_remote_store(&test_dir);
    
    // 创建测试数据
    let chunk_id = common::create_random_chunk(&chunk_store, 1024).await;
    
    // 构建扇区
    let mut builder = SectorBuilder::new();
    builder.add_chunk(chunk_id.clone(), 0..1024);
    let meta = builder.build();
    
    // 创建加密器
    let encryptor = SectorEncryptor::new(meta.clone(), chunk_store, 0).await.unwrap();
    
    sector_store.write(&meta.sector_id(), 0, BufReader::new(encryptor), Some(meta.sector_length())).await.unwrap();
    
    let mut decryptor = ChunkDecryptor::new(chunk_id.clone(), vec![meta], &sector_store).await.unwrap();

    let mut buffer = vec![0u8; 1024];
    decryptor.read_to_end(&mut buffer).await.unwrap();

    let decrypted_chunk_id = ChunkId::with_data(&buffer).unwrap();
    assert_eq!(chunk_id, decrypted_chunk_id);

    common::cleanup_test_env(test_dir).await;
} 