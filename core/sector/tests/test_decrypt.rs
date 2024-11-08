mod common;

use sector::{SectorBuilder, SectorEncryptor, SectorDecryptor};
use chunk::{ChunkId, ChunkTarget};
use async_std::io::ReadExt;

#[async_std::test]
async fn test_sector_encryption_decryption() {
    let test_dir = common::setup_test_env().await;
    let store = common::create_test_local_store(&test_dir);
    
    // 创建原始测试数据
    let chunk_id = ChunkId::with_length(1024).unwrap();
    let original_data = vec![1u8; 1024];
    
    // 写入测试数据
    store.write(&chunk_id, 0, &original_data[..], Some(1024 as u64)).await.unwrap();
    
    // 构建扇区并加密
    let mut builder = SectorBuilder::new();
    builder.add_chunk(chunk_id.clone(), 0..1024);
    let meta = builder.build();
    
    let mut encryptor = SectorEncryptor::new(meta.clone(), store.clone(), 0).await.unwrap();
    
    // 读取加密数据
    let mut encrypted = Vec::new();
    encryptor.read_to_end(&mut encrypted).await.unwrap();
    
    // 写入加密数据
    let encrypted_chunk_id = ChunkId::with_length(encrypted.len() as u64).unwrap();
    store.write(&encrypted_chunk_id, 0, &encrypted[..], Some(encrypted.len() as u64)).await.unwrap();
    
    // 解密数据
    let mut decryptor = SectorDecryptor::new(vec![meta], store).await.unwrap();
    let mut decrypted = Vec::new();
    decryptor.read_to_end(&mut decrypted).await.unwrap();
    
    // 验证解密后的数据与原始数据相同
    assert_eq!(decrypted, original_data);

    common::cleanup_test_env(test_dir).await;
} 