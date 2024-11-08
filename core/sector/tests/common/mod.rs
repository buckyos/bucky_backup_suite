use std::path::PathBuf;
use async_std::fs;
use chunk::LocalStore;

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