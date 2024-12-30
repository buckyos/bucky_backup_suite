use std::io::Cursor;
use ndn_lib::{calc_quick_hash, ChunkHasher, ChunkId};
use rand::RngCore;
use buckyos_backup_lib::IBackupChunkTargetProvider;
use s3_chunk_target::*;
use tokio::io::AsyncReadExt;
use url::Url;
use buckyos_kit::*;

async fn create_random_chunk(length: u64) -> (ChunkId, Vec<u8>) {
    let mut rng = rand::thread_rng();
    let mut data = vec![0u8; length as usize];
    rng.fill_bytes(&mut data);
    let mut hasher: ChunkHasher = ChunkHasher::new(None).unwrap();
    hasher.update_from_bytes(&data);
    let chunk_id = hasher.finalize_chunk_id();
    (chunk_id, data)
}

async fn create_test_s3_target() -> S3ChunkTarget {
    S3ChunkTarget::with_url(Url::parse("s3://buckyos-test-chunks?region=us-east-1").unwrap()).await.unwrap()
}

#[tokio::test]
async fn test_s3_chunk_write_read() {
    init_logging("s3_chunk_target");
    let target = create_test_s3_target().await;
    
    // 测试不同大小的 chunks
    let chunk_sizes = vec![1024, 10 * 1024 * 1024, 5 * 1024 * 1024 + 1024];
    
    for size in chunk_sizes {
        // 创建随机数据和计算 chunk id
        let (chunk_id, data) = create_random_chunk(size).await;
        
        let (mut writer, _) = target.open_chunk_writer(&chunk_id, 0, data.len() as u64).await.unwrap();
        tokio::io::copy(&mut Cursor::new(data), writer.as_mut().get_mut()).await.unwrap();
        target.complete_chunk_writer(&chunk_id).await.unwrap();
        
        let mut hasher = ChunkHasher::new(None).unwrap();
        // 读取 chunk
        let mut reader = target.open_chunk_reader_for_restore(&chunk_id, 0).await.unwrap();
        
        let mut read_buf = vec![0u8; size as usize];
        reader.read_exact(&mut read_buf).await.unwrap();
        hasher.update_from_bytes(&read_buf);
        let read_chunk_id = hasher.finalize_chunk_id();
 
        // 验证读取的数据
        assert_eq!(chunk_id, read_chunk_id, "Chunk ID mismatch for size {}", size);
    }
}



#[tokio::test]
async fn test_s3_chunk_resume_upload() {
    init_logging("s3_chunk_target");
   
    let size = 10 * 1024 * 1024;
    let (chunk_id, data) = create_random_chunk(size).await;


    let target = create_test_s3_target().await;
    let (mut writer, _) = target.open_chunk_writer(&chunk_id, 0, size).await.unwrap();
    tokio::io::copy(&mut Cursor::new(&data[..S3ChunkTarget::part_size()]), writer.as_mut().get_mut()).await.unwrap();
    

    let target = create_test_s3_target().await;
    let (mut writer, written) = target.open_chunk_writer(&chunk_id, 0, size).await.unwrap();
    assert_eq!(written as usize, S3ChunkTarget::part_size());
    tokio::io::copy(&mut Cursor::new(&data[S3ChunkTarget::part_size()..]), writer.as_mut().get_mut()).await.unwrap();
    target.complete_chunk_writer(&chunk_id).await.unwrap();
        
    let mut hasher = ChunkHasher::new(None).unwrap();
    // 读取 chunk
    let mut reader = target.open_chunk_reader_for_restore(&chunk_id, 0).await.unwrap();
    
    let mut read_buf = vec![0u8; size as usize];
    reader.read_exact(&mut read_buf).await.unwrap();
    hasher.update_from_bytes(&read_buf);
    let read_chunk_id = hasher.finalize_chunk_id();
 
    // 验证读取的数据
    assert_eq!(chunk_id, read_chunk_id, "Chunk ID mismatch for size {}", size);
}

// test link chunk
#[tokio::test]
async fn test_s3_chunk_link() {
    init_logging("s3_chunk_target");
    let target = create_test_s3_target().await;

    let size = 1024 * 1024;
    let (chunk_id, data) = create_random_chunk(size).await;

    let quick_hash = calc_quick_hash(&mut Cursor::new(&data), Some(size)).await.unwrap();
    assert_ne!(quick_hash, chunk_id);

    let (mut writer, _) = target.open_chunk_writer(&quick_hash, 0, size).await.unwrap();
    tokio::io::copy(&mut Cursor::new(&data), writer.as_mut().get_mut()).await.unwrap();
    target.complete_chunk_writer(&quick_hash).await.unwrap();

    target.link_chunkid(&quick_hash, &chunk_id).await.unwrap();

    let link_chunk_id = target.query_link_target(&quick_hash).await.unwrap().unwrap();
    assert_eq!(link_chunk_id, chunk_id);

    let mut hasher = ChunkHasher::new(None).unwrap();
    // 读取 chunk
    let mut reader = target.open_chunk_reader_for_restore(&chunk_id, 0).await.unwrap();
    
    let mut read_buf = vec![0u8; size as usize];
    reader.read_exact(&mut read_buf).await.unwrap();
    hasher.update_from_bytes(&read_buf);
    let read_chunk_id = hasher.finalize_chunk_id();
 
    // 验证读取的数据
    assert_eq!(chunk_id, read_chunk_id, "Chunk ID mismatch for size {}", size);
}

