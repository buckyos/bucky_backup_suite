use std::io::SeekFrom;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{sync::Arc, time::Duration};
use async_std::fs::File;
use async_std::{io::BufReader, task};
use async_std::io::prelude::*;
use chunk::*;
use serde::{Serialize, Deserialize};
use sqlx::types::chrono::{self, DateTime, Utc};
use crate::decrypt::ChunkDecryptor;
use crate::{
    decrypt::SectorDecryptor, 
    encrypt::SeekOnceSectorEncryptor, 
    sector::{SectorMeta, SectorBuilder},
};

struct StoreImpl<T: ChunkTarget> {
    local_store: LocalStore,
    remote_store: T, 
    sql_pool: sqlx::Pool<sqlx::Sqlite>, 
    config: SectorStoreConfig,
    collect_chunks_waker: async_std::channel::Sender<()>,
    collect_chunks_waiter: async_std::channel::Receiver<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorStoreConfig {
    pub base_path: String, 
    pub post_sector_interval: Duration,
    pub collect_sector_interval: Duration,
    pub max_sector_size: u64,
    pub chunk_max_wait_time: Duration,
}

pub struct SectorStore<T: ChunkTarget> {
    inner: Arc<StoreImpl<T>>
}

impl<T: ChunkTarget> Clone for SectorStore<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}

#[derive(sqlx::FromRow)]
struct ChunkRow {
    chunk_id: String, 
    length: i64, 
    created_at: DateTime<Utc>, 
    written_at: Option<DateTime<Utc>>, 
    deleted_at: Option<DateTime<Utc>>, 
    process_id: Option<i64>, 
}

#[derive(sqlx::FromRow)]
struct SectorRow {
    id: String,
    length: i64,
    created_at: DateTime<Utc>,
    written_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    process_id: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct ChunksInSectorRow {
    chunk_id: String,
    sector_id: String,
    offset_in_chunk: i64,
    length: i64,
    offset_in_sector: i64,
}


impl<T: 'static + ChunkTarget + Clone> SectorStore<T> {
    fn sql_create_chunks_table() -> &'static str {
        "CREATE TABLE IF NOT EXISTS chunks (
            chunk_id TEXT PRIMARY KEY, 
            length INTEGER, 
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP, 
            written_at DATETIME, 
            deleted_at DATETIME, 
            process_id INTEGER
        )"
    }

    fn sql_create_sectors_table() -> &'static str {
        "CREATE TABLE IF NOT EXISTS sectors (
            id TEXT PRIMARY KEY, 
            length INTEGER, 
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP, 
            written_at DATETIME, 
            deleted_at DATETIME, 
            process_id INTEGER
        )"
    }

    fn sql_create_sector_chunks_table() -> &'static str {
        "CREATE TABLE IF NOT EXISTS chunks_in_sectors (
            chunk_id TEXT, 
            sector_id TEXT, 
            offset_in_chunk INTEGER, 
            length INTEGER, 
            offset_in_sector INTEGER, 
            PRIMARY KEY (sector_id, offset_in_sector)
        )"
    }

    fn sql_pool(&self) -> &sqlx::Pool<sqlx::Sqlite> {
        &self.inner.sql_pool
    }

    pub fn with_sql_pool(sql_pool: sqlx::Pool<sqlx::Sqlite>, sector_store: T, config: SectorStoreConfig) -> Self {
        let (collect_chunks_waker, collect_chunks_waiter) = async_std::channel::bounded(1);
        Self {
            inner: Arc::new(StoreImpl {
                local_store: LocalStore::new(config.base_path.clone()), 
                sql_pool,  
                remote_store: sector_store,
                config,
                collect_chunks_waker,
                collect_chunks_waiter,
            })
        }
    }

    pub async fn init(&self) -> ChunkResult<()> {
        self.inner.local_store.init().await?;
        sqlx::query(Self::sql_create_chunks_table()).execute(self.sql_pool()).await?;
        sqlx::query(Self::sql_create_sectors_table()).execute(self.sql_pool()).await?;
        sqlx::query(Self::sql_create_sector_chunks_table()).execute(self.sql_pool()).await?;    
        Ok(())
    }

    fn config(&self) -> &SectorStoreConfig {
        &self.inner.config
    }

    fn local_store(&self) -> &LocalStore {
        &self.inner.local_store
    }

    fn remote_store(&self) -> &T {
        &self.inner.remote_store
    }

    pub async fn start(&self) -> ChunkResult<()> {
        {
            let store = self.clone();
            task::spawn(async move {
                let _ = store.post_sector_loop().await;
            });
        }
        self.collect_sector_loop().await
    }

    async fn query_sector_meta(&self, sector: &SectorRow) -> ChunkResult<SectorMeta> {
        let chunks = sqlx::query_as::<_, ChunksInSectorRow>(
            "SELECT * FROM chunks_in_sectors WHERE sector_id = ? ORDER BY offset_in_sector ASC"
        )
        .bind(&sector.id)
        .fetch_all(self.sql_pool())
        .await?;

        // 构建扇区数据
        let mut sector_builder = SectorBuilder::new();
        for chunk in chunks {
            sector_builder.add_chunk(
                chunk.chunk_id.parse()?,
                chunk.offset_in_chunk as u64..(chunk.offset_in_chunk + chunk.length) as u64,
            );
        }

        Ok(sector_builder.build())
    }

    async fn post_sector_inner(&self, sector: SectorRow) -> ChunkResult<()> {
        // 调用底层存储写入扇区数据
        // 从数据库中获取该扇区包含的所有数据块信息
        let sector_meta = self.query_sector_meta(&sector).await?;

        let sector_encryptor = SeekOnceSectorEncryptor::new(
            sector_meta, 
            self.local_store().clone()
        );
        
        let status = self.inner.remote_store.write(
            &sector.id.parse()?,  // 将sector id字符串解析为ChunkId
            0,  // 从头开始写入
            BufReader::new(sector_encryptor),
            Some(sector.length as u64)
        ).await?;

        if status.written == sector.length as u64 {
            // 更新sectors表中的写入时间
            sqlx::query(
                "UPDATE sectors SET written_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(sector.id)
            .execute(self.sql_pool())
            .await?;
        }

        Ok(())
    }

    async fn sectors_of_chunk(&self, chunk_id: &ChunkId) -> ChunkResult<Vec<SectorRow>> {
        let sectors = sqlx::query_as::<_, SectorRow>(
            "SELECT * FROM sectors WHERE chunk_id = ? ORDER BY offset_in_chunk ASC"
        )
        .bind(chunk_id.to_string())
        .fetch_all(self.sql_pool())
        .await?;

        Ok(sectors)
    }

    async fn post_sector_loop(&self) -> ChunkResult<()> {
        // 查询最早创建但未写入的扇区
        loop {
            let row = sqlx::query_as::<_, SectorRow>(
                "SELECT * FROM sectors WHERE written_at IS NULL ORDER BY created_at ASC LIMIT 1"
            )
            .fetch_optional(self.sql_pool())
            .await?;
    
            if let Some(sector) = row {
                return self.post_sector_inner(sector).await;
            } else {
                task::sleep(self.config().post_sector_interval).await;
            }
        }
       
    }

    async fn collect_sector_inner(&self) -> ChunkResult<bool> {
        // 查询超过等待时间且未完全分配到扇区的chunks
        let chunks = sqlx::query_as::<_, ChunkRow>(
            "SELECT * FROM chunks WHERE written_at IS NOT NULL 
             AND id IN (
                SELECT chunk_id FROM sector_chunks 
                GROUP BY chunk_id 
                HAVING SUM(length) < (SELECT length FROM chunks WHERE id = chunk_id)
             )
             ORDER BY written_at ASC"
        )
        .fetch_all(self.sql_pool())
        .await?;

        if !chunks.is_empty() {
            return Ok(false);
        }

        let sector_builder = {
            let mut sector_builder = SectorBuilder::new().with_length_limit(self.config().max_sector_size);
            let first_chunk = &chunks[0];
            let overtime = first_chunk.written_at.unwrap() + self.config().chunk_max_wait_time < chrono::Utc::now();
            for chunk in chunks {
                let sectors = self.sectors_of_chunk(&chunk.chunk_id.parse()?).await?;
                // 计算已分配到扇区的总长度
                let allocated_length: i64 = sectors.iter()
                    .map(|s| s.length)
                    .sum();

                let remain_length = chunk.length - allocated_length;
                let added = sector_builder.add_chunk(chunk.chunk_id.parse()?, allocated_length as u64..(allocated_length + remain_length) as u64);
                if added < remain_length as u64 {
                    break;
                }
            }

            if overtime {
                Some(sector_builder)
            } else if sector_builder.length() == self.config().max_sector_size {
                Some(sector_builder)
            } else {
                None
            }
        };
        
        if sector_builder.is_none() {
            return Ok(false);
        }

        let sector_meta = sector_builder.unwrap().build();
        let mut transaction = self.sql_pool().begin().await?;
        
        let mut offset_in_sector = 0;
        for (chunk_id, range_in_chunk) in sector_meta.header().chunks.iter() {
            let length = range_in_chunk.end - range_in_chunk.start;
            sqlx::query(
                "INSERT INTO chunks_in_sectors (chunk_id, sector_id, offset_in_chunk, length, offset_in_sector) 
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(chunk_id.to_string())
            .bind(sector_meta.sector_id().to_string())
            .bind(range_in_chunk.start as i64)
            .bind(length as i64)
            .bind(offset_in_sector as i64)
            .execute(&mut transaction)
            .await?;

            offset_in_sector += length;
        }

        transaction.commit().await?;

        Ok(true)
    }

    async fn collect_sector_loop(&self) -> ChunkResult<()> {
        loop {
            match self.collect_sector_inner().await {
                Err(_) | Ok(false) => {
                    let _ = async_std::future::timeout(
                        self.config().collect_sector_interval, 
                        self.inner.collect_chunks_waiter.recv()
                    ).await;
                    task::sleep(self.config().collect_sector_interval).await;
                },
                _ => {}
            }
            
        }
    }
}

pub enum SectorStoreRead<T: ChunkTarget> {
    Remote(ChunkDecryptor<T>),
    Local(File),
}

impl<T: 'static + ChunkTarget> Read for SectorStoreRead<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        let mut_self = self.get_mut();
        match mut_self {
            Self::Remote(sector_decryptor) => Pin::new(sector_decryptor).poll_read(cx, buf),
            Self::Local(file) => Pin::new(file).poll_read(cx, buf),
        }
    }
}

impl<T: 'static + ChunkTarget> Seek for SectorStoreRead<T> {
    fn poll_seek(self: Pin<&mut Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<std::io::Result<u64>> {
        let mut_self = self.get_mut();
        match mut_self {
            Self::Remote(sector_decryptor) => Pin::new(sector_decryptor).poll_seek(cx, pos),
            Self::Local(file) => Pin::new(file).poll_seek(cx, pos),
        }
    }
}

#[async_trait::async_trait]
impl<T: 'static + ChunkTarget + Clone> ChunkTarget for SectorStore<T> {
    type Read = SectorStoreRead<T>;

    async fn read(&self, chunk_id: &ChunkId) -> ChunkResult<Self::Read> {
        match self.local_store().read(chunk_id).await {
            Ok(local) => {
                return Ok(SectorStoreRead::Local(local));
            }
            Err(_) => {},
        };

        let sectors = self.sectors_of_chunk(chunk_id).await?;
        let mut metas = vec![];
        for sector in sectors {
            metas.push(self.query_sector_meta(&sector).await?);
        }

        Ok(SectorStoreRead::Remote(ChunkDecryptor::new(chunk_id.clone(), metas, self.remote_store()).await?))
    }

    async fn write(&self, chunk_id: &ChunkId, offset: u64, reader: impl async_std::io::BufRead + Unpin + Send + Sync + 'static, length: Option<u64>) -> ChunkResult<ChunkStatus> {
        // 首先检查 chunks 表中的状态
        let row = sqlx::query_as::<_, ChunkRow>("SELECT * FROM sectors WHERE id = ?")
            .bind(chunk_id.to_string()).fetch_optional(self.sql_pool()).await?;

        match row {
            // 如果记录不存在,或者已被删除,需要写入 chunk store
            None => {
                let _ = sqlx::query("INSERT OR REPLACE INTO chunks (id, length) VALUES (?, ?)")
                    .bind(chunk_id.to_string()).bind(chunk_id.length() as i64)
                    .execute(self.sql_pool()).await?;
                let _ = self.inner.collect_chunks_waker.try_send(());
            }, 
            Some(row) if row.written_at.is_none() => {
               
            },
            // 如果已经写入,直接返回状态
            _ => {
                return Ok(ChunkStatus {
                    chunk_id: chunk_id.clone(),
                    written: chunk_id.length(),
                });
            }
        };

        // 写入成功后更新数据库
        let chunk_status = self.inner.local_store.write(chunk_id, offset, reader, length).await?;
                
        if chunk_status.written == chunk_id.length() {
            // 更新写入时间为当前时间
            let _ = sqlx::query("UPDATE chunks SET written_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(chunk_id.to_string())
                .execute(self.sql_pool())
                .await?;
        }

        Ok(chunk_status)
    }

    async fn get(&self, chunk_id: &ChunkId) -> ChunkResult<Option<ChunkStatus>> {
        let row = sqlx::query_as::<_, ChunkRow>("SELECT * FROM chunks WHERE id = ?")
            .bind(chunk_id.to_string()).fetch_optional(self.sql_pool()).await?;
        todo!()
    }

    async fn delete(&mut self, chunk_id: &ChunkId) -> ChunkResult<()> {
        Ok(())
    }

    async fn list(&self) -> ChunkResult<Vec<ChunkStatus>> {
        Ok(vec![])
    }
}
