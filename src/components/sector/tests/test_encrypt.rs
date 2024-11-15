mod common;
use std::io::SeekFrom;
use rand::RngCore;
use sector::*;
use chunk::*;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[tokio::test]
async fn test_sector_encrypt_and_decrypt() {
    let test_dir = common::setup_test_env().await;
    let chunk_store = common::create_test_local_store(&test_dir);
    let sector_store = common::create_test_remote_store(&test_dir);

    async fn one_chunk_without_key(chunk_store: &LocalStore, sector_store: &LocalStore) {
        // 创建测试数据
        let chunk_length = 1024;
        let chunk_id = common::create_random_chunk(&chunk_store, chunk_length).await;
        
        // 构建扇区
        let mut builder = SectorBuilder::new();
       
        builder.add_chunk(chunk_id.clone(), 0..chunk_length);
        let meta = builder.build();
        
        // 创建加密器
        let encryptor = SectorEncryptor::new(meta.clone(), chunk_store.clone(), 0).await.unwrap();
        sector_store.write(ChunkWrite {
            chunk_id: meta.sector_id().to_owned(), 
            offset: 0, 
            reader: encryptor, 
            tail: Some(meta.sector_length()), 
            length: Some(meta.sector_length()), 
            full_id: None
        }).await.unwrap();

        let mut direct_read = sector_store.read(&meta.sector_id()).await.unwrap().unwrap();
        let mut buffer = vec![];
        direct_read.seek(SeekFrom::Start(meta.header_length())).await.unwrap();
        let direct_read_len = direct_read.read_to_end(&mut buffer).await.unwrap();
        assert_eq!(direct_read_len, 1024);
        let direct_read_chunk_id = FullHasher::calc_from_bytes(&buffer);
        // assert_eq!(chunk_id.length(), buffer.len());
        assert_eq!(chunk_id, direct_read_chunk_id);
        
        let mut decryptor = ChunkDecryptor::new(chunk_id.clone(), chunk_length, vec![meta], sector_store).await.unwrap();
        let mut buffer = vec![];
        decryptor.read_to_end(&mut buffer).await.unwrap();
        let decrypted_chunk_id = FullHasher::calc_from_bytes(&buffer);
        assert_eq!(chunk_id, decrypted_chunk_id);
    }


    async fn one_chunk_with_key(chunk_store: &LocalStore, sector_store: &LocalStore) {
        // 创建测试数据
        let chunk_length = 1024;
        let chunk_id = common::create_random_chunk(&chunk_store, chunk_length).await;
        
        // 构建扇区
        let mut rng = rand::thread_rng();
        let mut key = vec![0u8; 32];
        rng.fill_bytes(&mut key);
        let mut builder = SectorBuilder::new()
            .with_key(key);
        builder.add_chunk(chunk_id.clone(), 0..chunk_length);
        let meta = builder.build();
        
        // 创建加密器
        let encryptor = SectorEncryptor::new(meta.clone(), chunk_store.clone(), 0).await.unwrap();
        sector_store.write(ChunkWrite {
            chunk_id: meta.sector_id().to_owned(), 
            offset: 0, 
            reader: encryptor, 
            tail: Some(meta.sector_length()), 
            length: Some(meta.sector_length()), 
            full_id: None
        }).await.unwrap();
        
        let mut decryptor = ChunkDecryptor::new(chunk_id.clone(), chunk_length, vec![meta], sector_store).await.unwrap();
        let mut buffer = vec![];
        decryptor.read_to_end(&mut buffer).await.unwrap();
        let decrypted_chunk_id = FullHasher::calc_from_bytes(&buffer);
        assert_eq!(chunk_id, decrypted_chunk_id);
    }
    
    one_chunk_without_key(&chunk_store, &sector_store).await;
    one_chunk_with_key(&chunk_store, &sector_store).await;
    common::cleanup_test_env(test_dir).await;
} 