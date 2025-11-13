use ndn_lib::ChunkId;
use rusqlite::types::{FromSql, ToSql, ValueRef};
use std::ops::{Deref, DerefMut};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BuckyBackupError {
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("AlreadyDone: {0}")]
    AlreadyDone(String),
    #[error("TryLater: {0}")]
    TryLater(String),
    #[error("NeedProcess: {0}")]
    NeedProcess(String),
    #[error("Failed: {0}")]
    Failed(String),
    #[error("NotFound: {0}")]
    NotFound(String),
}

pub type BackupResult<T> = std::result::Result<T, BuckyBackupError>;

//use tokio::fs::AsyncReadExt;
#[derive(Debug, Clone, PartialEq)]
pub enum BackupItemState {
    New,
    Done,
    Failed(String),
}

impl ToSql for BackupItemState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            BackupItemState::New => "NEW".to_string(),
            BackupItemState::Done => "DONE".to_string(),
            BackupItemState::Failed(msg) => format!("FAILED:{}", msg),
        };

        Ok(s.into())
    }
}

impl FromSql for BackupItemState {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "NEW" => BackupItemState::New,
            "DONE" => BackupItemState::Done,
            _ => {
                if s.starts_with("FAILED:") {
                    BackupItemState::Failed(s.to_string())
                } else {
                    BackupItemState::New
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct BackupChunkItem {
    pub item_id: String, //对source来说，可以用item_id来唯一的标示一个待备份的item,一般是文件的相对路径
    pub chunk_id: ChunkId,
    pub local_chunk_id: Option<ChunkId>, //original chunk id before crypto
    pub state: BackupItemState,
    pub size: u64,
    pub last_update_time: u64,
    pub offset: u64
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckPointState {
    New,
    Prepared, //local ok
    WaitTrans,
    Working,
    //Evaluated,//所有的backup item都计算了hash和diff(如有需要)
    Done,
    Failed(String),
}

impl CheckPointState {
    pub fn need_working(&self) -> bool {
        match self {
            CheckPointState::Done => false,
            CheckPointState::Failed(_) => false,
            _ => true,
        }
    }
}

impl ToSql for CheckPointState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            CheckPointState::New => "NEW".to_string(),
            CheckPointState::Prepared => "PREPARED".to_string(),
            CheckPointState::WaitTrans => "WAIT_TRANS".to_string(),
            CheckPointState::Working => "WORKING".to_string(),
            CheckPointState::Done => "DONE".to_string(),
            CheckPointState::Failed(msg) => format!("FAILED:{}", msg.as_str()),
        };
        Ok(s.into())
    }
}

impl FromSql for CheckPointState {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "NEW" => CheckPointState::New,
            "PREPARED" => CheckPointState::Prepared,
            "WAIT_TRANS" => CheckPointState::WaitTrans,
            "WORKING" => CheckPointState::Working,
            "DONE" => CheckPointState::Done,
            _ => CheckPointState::Failed(s.to_string()), // 默认失败状态
        })
    }
}

pub const CHECKPOINT_TYPE_CHUNK: &str = "c2c";

//remote checkpoint,which will be sent to backup service and stored in backup service
pub struct BackupCheckpoint {
    pub checkpoint_type: String,
    pub checkpoint_name: String,
    pub prev_checkpoint_id: Option<String>,
    pub state: CheckPointState,
    pub extra_info: String,
    pub create_time: u64,
    pub last_update_time: u64,

    pub item_list_id: String,
    pub item_count: u64,
    pub total_size: u64,
}

impl BackupCheckpoint {
    pub fn new(
        checkpoint_type: String,
        checkpoint_name: String,
        prev_checkpoint_id: Option<String>,
        items: Option<&Vec<BackupChunkItem>>,
    ) -> Self {
        let now = buckyos_kit::buckyos_get_unix_timestamp();
        if items.is_some() {
            let items = items.unwrap();
            let item_count = items.len() as u64;
            let total_size = items.iter().map(|item| item.size).sum();
            return Self {
                checkpoint_type: checkpoint_type,
                checkpoint_name: checkpoint_name,
                prev_checkpoint_id: prev_checkpoint_id,
                state: CheckPointState::New,
                extra_info: String::new(),
                create_time: now,
                last_update_time: now,
                item_list_id: String::new(),
                item_count: item_count,
                total_size: total_size,
            };
        } else {
            return Self {
                checkpoint_type: checkpoint_type,
                checkpoint_name: checkpoint_name,
                prev_checkpoint_id: prev_checkpoint_id,
                state: CheckPointState::New,
                extra_info: String::new(),
                create_time: now,
                last_update_time: now,
                item_list_id: String::new(),
                item_count: 0,
                total_size: 0,
            };
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteBackupCheckPointItemStatus {
    //query state item by item
    NotSupport,
    WaitItemList,
    //not complete item ids
    NotComplete(Vec<String>),
    //complete item ids
    Complete(Vec<String>),
}

pub struct LocalBackupCheckpoint {
    pub checkpoint: BackupCheckpoint,
    pub checkpoint_id: String,
    pub owner_plan_id: String,
    pub crpyto_config: String,
    pub crypto_key: String,
    pub org_item_list_id: String,
}

impl Deref for LocalBackupCheckpoint {
    type Target = BackupCheckpoint;

    fn deref(&self) -> &Self::Target {
        &self.checkpoint
    }
}

impl DerefMut for LocalBackupCheckpoint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.checkpoint
    }
}

impl LocalBackupCheckpoint {
    pub fn new(checkpoint: BackupCheckpoint, checkpoint_id: String, owner_plan_id: String) -> Self {
        Self {
            checkpoint: checkpoint,
            checkpoint_id: checkpoint_id,
            owner_plan_id: owner_plan_id,
            crpyto_config: String::new(),
            crypto_key: String::new(),
            org_item_list_id: String::new(),
        }
    }
}
