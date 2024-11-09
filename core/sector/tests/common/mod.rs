use std::path::PathBuf;
use async_std::{fs, io::Cursor};
use chunk::{ChunkId, LocalStore, ChunkTarget};
use rand::RngCore;
use sha2::{Sha256, Digest};

pub async fn setup_test_env() -> PathBuf {
    let test_dir = PathBuf::from("target/test_tmp");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir_all(&test_dir).await.unwrap();
    test_dir
}

pub async fn cleanup_test_env(test_dir: PathBuf) {
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
}

pub fn create_test_local_store(test_dir: &PathBuf) -> LocalStore {
    LocalStore::new(test_dir.to_str().unwrap().to_string())
} 

pub fn create_test_remote_store(test_dir: &PathBuf) -> LocalStore {
    LocalStore::new(test_dir.to_str().unwrap().to_string())
}

pub async fn create_random_chunk(store: &LocalStore, length: u64) -> ChunkId {
    let mut rng = rand::thread_rng();
    let mut data = vec![0u8; length as usize];
    rng.fill_bytes(&mut data);
    let mut hasher = ChunkId::hasher();
    hasher.update(&data);
    let chunk_id = ChunkId::with_hasher(length, hasher).unwrap();
    store.write(&chunk_id, 0, Cursor::new(data), Some(length)).await.unwrap();
    chunk_id
}
