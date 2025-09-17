#![allow(unused)]
use std::error::Error;
use std::path::{Path, PathBuf};
use tokio::fs;
use rusqlite::{Connection, Result as SqliteResult, OptionalExtension};
use sha2::{Sha256, Digest};
use async_channel::{bounded, Sender, Receiver};
use ignore::WalkBuilder;
use tokio::io::AsyncReadExt;
use serde::{Serialize, Deserialize};

// 文件扫描状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum ScanStatus {
    Pending,           // 等待扫描
    SmallFileScanned, // 小文件已完成扫描
    LargeFilePending, // 大文件等待处理
    DiffPending,      // 需要进行diff比较
    Completed,        // 扫描完成
    Failed,           // 扫描失败
    FirstPassCompleted, // 第一遍扫描完成
}

// 添加新的元信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaInfo {
    mode: u32,      // 文件权限模式 (Linux permissions)
    metadata: serde_json::Value, // 额外的metadata以JSON格式存储
}

impl Default for MetaInfo {
    fn default() -> Self {
        Self {
            mode: 0,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

// 文件信息结构
#[derive(Debug)]
struct FileInfo {
    path: PathBuf,
    size: u64,
    hash: Option<String>,
    status: ScanStatus,
    last_hash: Option<String>, // 上次扫描的hash值
    meta_info: MetaInfo,  // 添加元信息字段
}

#[derive(Debug)]
struct DirInfo {
    path: PathBuf,
    status: ScanStatus,
    total_size: u64,
    files_count: u32,
    subdirs_count: u32,
}

#[derive(Debug)]
pub struct ScanConfig {
    ignore_patterns: Vec<String>,
    custom_ignore_file: Option<PathBuf>,
    respect_gitignore: bool,
    min_file_size: u64,    // 小于此大小的文件直接忽略
    max_file_size: u64,    // 大于此大小的文件进入大文件队列
    root_dir: Option<PathBuf>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            ignore_patterns: vec![
                String::from("node_modules"),
                String::from("target"),
                String::from(".git"),
                String::from("*.tmp"),
                String::from("*.log"),
            ],
            custom_ignore_file: None,
            respect_gitignore: true,
            min_file_size: 1024,        // 1KB
            max_file_size: 100 * 1024 * 1024, // 100MB
            root_dir: None,
        }
    }
}

pub struct DirSource {
    pub path: String,
    db_conn: Option<Connection>,
    config: ScanConfig,
    large_file_sender: Option<Sender<FileInfo>>,
    diff_file_sender: Option<Sender<FileInfo>>,
}

impl DirSource {
    pub fn new(path: String, config: Option<ScanConfig>) -> Self {
        Self {
            path,
            db_conn: None,
            config: config.unwrap_or_default(),
            large_file_sender: None,
            diff_file_sender: None,
        }
    }

    // 初始化数据库和通道
    async fn init(&mut self) -> Result<(), Box<dyn Error>> {
        // 初始化数据库
        let conn = Connection::open_in_memory()?;
        // 文件表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                size INTEGER NOT NULL,
                hash TEXT,
                status TEXT NOT NULL,
                last_hash TEXT,
                mode INTEGER NOT NULL,
                metadata TEXT NOT NULL 
            )",
            [],
        )?;

        // 目录表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS directories (
                path TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                files_count INTEGER NOT NULL,
                subdirs_count INTEGER NOT NULL
            )",
            [],
        )?;
        self.db_conn = Some(conn);

        // 初始化通道
        let (large_tx, large_rx) = bounded(100);
        let (diff_tx, diff_rx) = bounded(100);
        self.large_file_sender = Some(large_tx);
        self.diff_file_sender = Some(diff_tx);

        // 启动大文件处理器
        self.start_large_file_processor(large_rx);
        // 启动diff处理器
        self.start_diff_processor(diff_rx);

        Ok(())
    }

    // 第一遍扫描:快速扫描所有文件和目录
    async fn first_pass_scan(&self, dir: &Path) -> Result<DirInfo, Box<dyn Error>> {
        let mut walker = WalkBuilder::new(dir);
        
        // 配置忽略规则
        if self.config.respect_gitignore {
            walker.git_ignore(true);
            walker.git_global(true);
        }
        
        // 添加自定义忽略规则
        for pattern in &self.config.ignore_patterns {
            walker.add_ignore(pattern);
        }
        
        // 如果有自定义忽略文件
        if let Some(ignore_file) = &self.config.custom_ignore_file {
            walker.add_custom_ignore_filename(ignore_file);
        }

        let mut dir_info = DirInfo {
            path: dir.to_path_buf(),
            status: ScanStatus::Pending,
            total_size: 0,
            files_count: 0,
            subdirs_count: 0,
        };

        // 遍历文件系统
        for entry in walker.build() {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                dir_info.subdirs_count += 1;
                // 创建目录信息
                let subdir_info = DirInfo {
                    path: path.to_path_buf(),
                    status: ScanStatus::FirstPassCompleted,
                    total_size: 0,
                    files_count: 0,
                    subdirs_count: 0,
                };
                self.save_dir_info(&subdir_info)?;
            } else {
                let metadata = fs::metadata(&path).await?;
                let size = metadata.len();
                
                // 检查文件大小是否在处理范围内
                if size < self.config.min_file_size {
                    continue;
                }

                dir_info.total_size += size;
                dir_info.files_count += 1;

                let mut status = if size > self.config.max_file_size {
                    ScanStatus::LargeFilePending
                } else {
                    ScanStatus::Pending
                };

                // 检查是否存在上次的hash
                let last_hash = self.get_last_hash(&path)?;
                
                let file_info = FileInfo {
                    path: path.to_path_buf(),
                    size,
                    hash: None,
                    status,
                    last_hash,
                    meta_info: MetaInfo {
                        mode: 0,
                        metadata: serde_json::Value::Object(serde_json::Map::new()),
                    },
                };

                // 保存文件信息到数据库
                self.save_file_info(&file_info)?;

                // 如果是小文件,直接处理
                if size <= self.config.max_file_size {
                    self.process_small_file(&file_info).await?;
                }
            }
        }

        // 更新目录状态为已完成第一遍扫描
        dir_info.status = ScanStatus::FirstPassCompleted;
        self.save_dir_info(&dir_info)?;
        
        Ok(dir_info)
    }

    // 第二遍扫描:处理大文件
    async fn second_pass_scan(&self) -> Result<(), Box<dyn Error>> {
        if let Some(sender) = &self.large_file_sender {
            let large_files = self.get_files_by_status(ScanStatus::LargeFilePending)?;
            for file in large_files {
                sender.send(file).await?;
            }
        }
        Ok(())
    }

    // 第三遍扫描:处理需要diff的文件
    async fn third_pass_scan(&self) -> Result<(), Box<dyn Error>> {
        if let Some(sender) = &self.diff_file_sender {
            let diff_files = self.get_files_by_status(ScanStatus::DiffPending)?;
            for file in diff_files {
                sender.send(file).await?;
            }
        }
        Ok(())
    }

    // 启动扫描
    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        // 初始化
        self.init().await?;
        
        // 第一遍扫描
        self.first_pass_scan(Path::new(&self.path)).await?;
        
        // 第二遍扫描
        self.second_pass_scan().await?;
        
        // 第三遍扫描
        self.third_pass_scan().await?;
        
        Ok(())
    }

    // 处理小文件
    async fn process_small_file(&self, file_info: &FileInfo) -> Result<(), Box<dyn Error>> {
        let mut hasher = Sha256::new();
        
        // 只将 mode 添加到哈希计算中
        hasher.update(&file_info.meta_info.mode.to_le_bytes());
        
        // 计算文件内容的哈希
        let content_hash = self.calculate_file_hash(&file_info.path).await?;
        hasher.update(content_hash.as_bytes());
        
        let final_hash = format!("{:x}", hasher.finalize());
        
        // 检查是否需要diff
        if let Some(last_hash) = &file_info.last_hash {
            if last_hash != &final_hash {
                self.update_file_status(&file_info.path, ScanStatus::DiffPending, Some(final_hash))?;
                return Ok(());
            }
        }

        self.update_file_status(&file_info.path, ScanStatus::Completed, Some(final_hash))?;
        Ok(())
    }

    // 启动大文件处理器
    fn start_large_file_processor(&self, rx: Receiver<FileInfo>) {
        tokio::spawn(async move {
            while let Ok(_file) = rx.recv().await {
                // TODO: 实现大文件的分块处理
            }
        });
    }

    // 启动diff处理器
    fn start_diff_processor(&self, rx: Receiver<FileInfo>) {
        tokio::spawn(async move {
            while let Ok(_file) = rx.recv().await {
                // TODO: 实现文件diff的计算
            }
        });
    }

    // 数据库相关辅助方法...

    // 保存目录信息到数据库
    fn save_dir_info(&self, dir_info: &DirInfo) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = &self.db_conn {
            conn.execute(
                "INSERT OR REPLACE INTO directories 
                (path, status, total_size, files_count, subdirs_count) 
                VALUES (?1, ?2, ?3, ?4, ?5)",
                [
                    dir_info.path.to_string_lossy().to_string(),
                    format!("{:?}", dir_info.status),
                    dir_info.total_size.to_string(),
                    dir_info.files_count.to_string(),
                    dir_info.subdirs_count.to_string(),
                ],
            )?;
        }
        Ok(())
    }

    // 获取目录信息
    fn get_dir_info(&self, path: &Path) -> Result<Option<DirInfo>, Box<dyn Error>> {
        if let Some(conn) = &self.db_conn {
            let mut stmt = conn.prepare(
                "SELECT status, total_size, files_count, subdirs_count 
                FROM directories WHERE path = ?1"
            )?;
            
            let dir_info = stmt.query_row(
                [path.to_string_lossy().to_string()],
                |row| {
                    Ok(DirInfo {
                        path: path.to_path_buf(),
                        status: serde_json::from_str(&row.get::<_, String>(0)?).unwrap(),
                        total_size: row.get(1)?,
                        files_count: row.get(2)?,
                        subdirs_count: row.get(3)?,
                    })
                },
            ).optional()?;

            Ok(dir_info)
        } else {
            Ok(None)
        }
    }

    // 计算文件哈希值
    async fn calculate_file_hash(&self, path: &Path) -> Result<String, Box<dyn Error>> {
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192]; // 8KB buffer
        
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    // 保存文件信息到数据库
    fn save_file_info(&self, file_info: &FileInfo) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = &self.db_conn {
            conn.execute(
                "INSERT OR REPLACE INTO files 
                (path, size, hash, status, last_hash, mode, metadata) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                [
                    file_info.path.to_string_lossy().to_string(),
                    file_info.size.to_string(),
                    file_info.hash.clone().unwrap_or_default(),
                    format!("{:?}", file_info.status),
                    file_info.last_hash.clone().unwrap_or_default(),
                    file_info.meta_info.mode.to_string(),
                    file_info.meta_info.metadata.to_string(), // 序列化metadata为JSON字符串
                ],
            )?;
        }
        Ok(())
    }

    // 根据状态获取文件列表
    fn get_files_by_status(&self, status: ScanStatus) -> Result<Vec<FileInfo>, Box<dyn Error>> {
        let mut files = Vec::new();
        if let Some(conn) = &self.db_conn {
            let mut stmt = conn.prepare(
                "SELECT path, size, hash, last_hash FROM files WHERE status = ?1"
            )?;
            
            let rows = stmt.query_map([format!("{:?}", status)], |row| {
                Ok(FileInfo {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    size: row.get(1)?,
                    hash: row.get(2)?,
                    status: status.clone(),
                    last_hash: row.get(3)?,
                    meta_info: MetaInfo {
                        mode: 0,
                        metadata: serde_json::Value::Object(serde_json::Map::new()),
                    },
                })
            })?;

            for file in rows {
                files.push(file?);
            }
        }
        Ok(files)
    }

    // 更新文件状态
    fn update_file_status(
        &self, 
        path: &Path, 
        status: ScanStatus,
        hash: Option<String>
    ) -> Result<(), Box<dyn Error>> {
        if let Some(conn) = &self.db_conn {
            conn.execute(
                "UPDATE files SET status = ?1, hash = ?2 WHERE path = ?3",
                [
                    format!("{:?}", status),
                    hash.unwrap_or_default(),
                    path.to_string_lossy().to_string(),
                ],
            )?;
        }
        Ok(())
    }

    // 获取文件的上次哈希值
    fn get_last_hash(&self, path: &Path) -> Result<Option<String>, Box<dyn Error>> {
        if let Some(conn) = &self.db_conn {
            let hash = conn.query_row(
                "SELECT hash FROM files WHERE path = ?1",
                [path.to_string_lossy().to_string()],
                |row| row.get(0)
            ).optional()?;
            Ok(hash)
        } else {
            Ok(None)
        }
    }

    // fn process_symlink(&self, link_path: &Path) -> Result<(), Error> {
    //     let target = fs::read_link(link_path)?;
        
    //     // 如果目标路径在根目录下
    //     if self.is_under_root_dir(&target) {
    //         // 存储相对路径,这会影响目录的hash值
    //         let relative_target = self.make_relative_to_root(&target);
    //         self.store_symlink_info(link_path, relative_target)?;
    //     } else {
    //         // 目标在根目录外,存储绝对路径
    //         self.store_symlink_info(link_path, target)?;
    //     }
    //     Ok(())
    // }

    fn is_under_root_dir(&self, path: &Path) -> bool {
        if let Some(root) = &self.config.root_dir {
            path.starts_with(root)
        } else {
            false
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    // 创建测试目录结构的辅助函数
    fn create_test_directory() -> Result<TempDir, Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        
        // 创建一些测试文件
        let small_file_path = temp_dir.path().join("small.txt");
        let mut small_file = File::create(small_file_path)?;
        small_file.write_all(b"Hello, World!")?;

        // 创建一个大文件
        let large_file_path = temp_dir.path().join("large.bin");
        let mut large_file = File::create(large_file_path)?;
        let large_data = vec![0u8; 1024 * 1024]; // 1MB
        large_file.write_all(&large_data)?;

        // 创建一个子目录和文件
        std::fs::create_dir(temp_dir.path().join("subdir"))?;
        let subdir_file_path = temp_dir.path().join("subdir").join("file.txt");
        let mut subdir_file = File::create(subdir_file_path)?;
        subdir_file.write_all(b"Subdir file content")?;

        Ok(temp_dir)
    }

    #[tokio::test]
    async fn test_basic_initialization() {
        let dir_source = DirSource::new(String::from("/tmp"), None);
        assert_eq!(dir_source.path, "/tmp");
        assert!(dir_source.db_conn.is_none());
        assert!(dir_source.large_file_sender.is_none());
        assert!(dir_source.diff_file_sender.is_none());
    }

    #[tokio::test]
    async fn test_first_pass_scan() -> Result<(), Box<dyn Error>> {
        let temp_dir = create_test_directory()?;
        let mut dir_source = DirSource::new(
            temp_dir.path().to_string_lossy().to_string(),
            None
        );
        
        // 初始化
        dir_source.init().await?;
        
        // 执行第一遍扫描
        let dir_info = dir_source.first_pass_scan(temp_dir.path()).await?;
        
        // 验证扫描结果
        assert_eq!(dir_info.status, ScanStatus::FirstPassCompleted);
        assert_eq!(dir_info.subdirs_count, 1); // 一个子目录
        assert_eq!(dir_info.files_count, 3);   // 三个文件
        assert!(dir_info.total_size > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_hash_calculation() -> Result<(), Box<dyn Error>> {
        let temp_dir = create_test_directory()?;
        let dir_source = DirSource::new(
            temp_dir.path().to_string_lossy().to_string(),
            None
        );

        // 计算小文件的哈希值
        let small_file_path = temp_dir.path().join("small.txt");
        let hash = dir_source.calculate_file_hash(&small_file_path).await?;
        
        // 验证哈希值不为空且长度正确 (SHA-256 produces 64 character hex string)
        assert_eq!(hash.len(), 64);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_database_operations() -> Result<(), Box<dyn Error>> {
        let temp_dir = create_test_directory()?;
        let mut dir_source = DirSource::new(
            temp_dir.path().to_string_lossy().to_string(),
            None
        );
        
        // 初始化数据库
        dir_source.init().await?;

        // 测试文件信息保存和检索
        let test_file_path = temp_dir.path().join("small.txt");
        let file_info = FileInfo {
            path: test_file_path.clone(),
            size: 13,
            hash: Some(String::from("test_hash")),
            status: ScanStatus::Pending,
            last_hash: None,
            meta_info: MetaInfo {
                mode: 0,
                metadata: serde_json::Value::Object(serde_json::Map::new()),
            },
        };

        // 保存文件信息
        dir_source.save_file_info(&file_info)?;

        // 获取并验证状态
        let files = dir_source.get_files_by_status(ScanStatus::Pending)?;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, test_file_path);
        assert_eq!(files[0].size, 13);

        // 测试状态更新
        dir_source.update_file_status(
            &test_file_path,
            ScanStatus::Completed,
            Some(String::from("new_hash"))
        )?;

        // 验证更新后的状态
        let completed_files = dir_source.get_files_by_status(ScanStatus::Completed)?;
        assert_eq!(completed_files.len(), 1);
        assert_eq!(completed_files[0].path, test_file_path);

        Ok(())
    }
}
