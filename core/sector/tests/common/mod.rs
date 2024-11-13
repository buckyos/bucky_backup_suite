use std::path::PathBuf;
use async_std::{fs, io::Cursor};
use chunk::{ChunkId, LocalStore, ChunkTarget};
use rand::RngCore;
use sha2::{Sha256, Digest};

pub async fn setup_test_env() -> PathBuf {
    use std::io::Write;
    env_logger::builder().filter_level(log::LevelFilter::Info)
    .filter_module("tide", log::LevelFilter::Off)
    .filter_module("sqlx", log::LevelFilter::Off)
    .format(|buf, record| {
        writeln!(
            buf,
            "{}:{} {} - {}",
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.level(),
            record.args()
        )
    }).init();

    let exe_path = std::env::current_exe().unwrap();
    let exe_dir = exe_path.parent().unwrap();   
    let test_dir = exe_dir.join("target/test_tmp");

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
    let chunk_id = ChunkId::with_data(&data[..]).unwrap();
    store.write(&chunk_id, 0, Cursor::new(data), Some(length)).await.unwrap();
    chunk_id
}