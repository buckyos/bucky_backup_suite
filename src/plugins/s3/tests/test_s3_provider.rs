use backup_lib::{IBackupChunkTargetProvider, ChunkTarget};
use backup_s3::S3ChunkProvider;
use std::env;

#[tokio::test]
async fn test_s3_operations() {
    let bucket = env::var("TEST_S3_BUCKET").expect("TEST_S3_BUCKET must be set");
    let prefix = env::var("TEST_S3_PREFIX").unwrap_or_default();
    let region = env::var("AWS_REGION").ok();

    let provider = S3ChunkProvider::new(bucket, prefix, region)
        .await
        .expect("Failed to create S3 provider");

    // 创建测试目标
    let target = ChunkTarget::new("test-chunk-1".to_string(), 0, 1024);
    
    // 测试写入
    let test_data = b"Hello, S3!".to_vec();
    provider.write(&target, &test_data)
        .await
        .expect("Failed to write chunk");

    // 测试读取
    let mut read_buf = vec![0u8; test_data.len()];
    let read_len = provider.read(&target, &mut read_buf)
        .await
        .expect("Failed to read chunk");
    
    assert_eq!(read_len, test_data.len());
    assert_eq!(&read_buf[..read_len], &test_data[..]);

    // 测试追加写入
    let append_target = ChunkTarget::new("test-chunk-1".to_string(), test_data.len() as u64, 1024);
    let append_data = b" More data!".to_vec();
    provider.write(&append_target, &append_data)
        .await
        .expect("Failed to append to chunk");

    // 验证完整数据
    let mut full_buf = vec![0u8; test_data.len() + append_data.len()];
    let full_read_len = provider.read(&target, &mut full_buf)
        .await
        .expect("Failed to read full chunk");
    
    assert_eq!(full_read_len, test_data.len() + append_data.len());
    assert_eq!(&full_buf[..test_data.len()], &test_data[..]);
    assert_eq!(&full_buf[test_data.len()..], &append_data[..]);

    // 测试删除
    provider.remove(&target)
        .await
        .expect("Failed to remove chunk");
} 