#![allow(dead_code)]
#![allow(unused)]
use buckyos_backup_lib::RestoreConfig;
use buckyos_backup_lib::*;
use log::*;
use ndn_lib::ChunkId;
use rusqlite::types::{FromSql, ToSql, ValueRef};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::TryFrom;
use thiserror::Error;
use uuid::Uuid;

// impl From<ChunkItem> for BackupItem {
//     fn from(item: ChunkItem) -> Self {
//         Self {
//             item_id: item.item_id,
//             item_type: BackupItemType::Chunk,
//             chunk_id: item.chunk_id.map(|id| id.to_string()),
//             quick_hash: None,
//             state: BackupItemState::New,
//             size: item.length,
//             last_modify_time: item.last_modify_time,
//             create_time: item.create_time,
//         }
//     }
// }

#[derive(Error, Debug)]
pub enum BackupDbError {
    #[error("NotFound: {0}")]
    NotFound(String),
    #[error("InvalidCheckpointId: {0}")]
    InvalidCheckpointId(String),
    #[error("database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("Data format error: {0}")]
    DataFormatError(String),
}

pub type Result<T> = std::result::Result<T, BackupDbError>;

impl From<BackupDbError> for BuckyBackupError {
    fn from(err: BackupDbError) -> Self {
        match err {
            BackupDbError::NotFound(msg) => BuckyBackupError::NotFound(msg),
            BackupDbError::InvalidCheckpointId(msg) => {
                BuckyBackupError::Failed(format!("invalid checkpoint id: {}", msg))
            }
            BackupDbError::DatabaseError(e) => {
                BuckyBackupError::Failed(format!("database error: {}", e.to_string()))
            }
            BackupDbError::DataFormatError(msg) => {
                BuckyBackupError::Failed(format!("data format error: {}", msg))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum BackupSource {
    Directory(String),
    ChunkList(String),
}

impl BackupSource {
    pub fn get_source_url(&self) -> &str {
        match self {
            BackupSource::Directory(url) => url.as_str(),
            BackupSource::ChunkList(url) => url.as_str(),
        }
    }
}

fn default_target_state() -> String {
    "UNKNOWN".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTargetRecord {
    pub target_id: String,
    pub target_type: String,
    pub url: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_target_state")]
    pub state: String,
    #[serde(default)]
    pub used: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub config: Option<Value>,
}

impl BackupTargetRecord {
    pub fn new(
        target_id: String,
        target_type: &str,
        url: &str,
        name: &str,
        description: Option<&str>,
        config: Option<Value>,
    ) -> Self {
        Self {
            target_id,
            target_type: target_type.to_string(),
            url: url.to_string(),
            name: name.to_string(),
            description: description.unwrap_or_default().to_string(),
            state: default_target_state(),
            used: 0,
            total: 0,
            last_error: String::new(),
            config,
        }
    }

    pub fn to_json_value(&self) -> Value {
        let mut value = json!({
            "target_id": self.target_id,
            "target_type": self.target_type,
            "url": self.url,
            "name": self.name,
            "description": self.description,
            "state": self.state,
            "used": self.used,
            "total": self.total,
            "last_error": self.last_error,
        });
        if let Some(cfg) = &self.config {
            value["config"] = cfg.clone();
        }
        value
    }
}

#[derive(Debug, Clone)]
pub struct BackupPlanConfig {
    pub source: BackupSource,
    pub target: String,
    pub title: String,
    pub description: String,
    pub type_str: String,
    pub last_checkpoint_index: u64,
    pub policy: Value,
    pub priority: i64,
}

impl BackupPlanConfig {
    pub fn get_checkpiont_type(&self) -> String {
        return self.type_str.clone();
    }

    pub fn to_json_value(&self) -> Value {
        let result = json!({
            "source": self.source.get_source_url(),
            "target": self.target,
            "title": self.title,
            "description": self.description,
            "type_str": self.type_str,
            "last_checkpoint_index": self.last_checkpoint_index,
            "policy": self.policy.clone(),
            "priority": self.priority,
        });
        result
    }

    pub fn chunk2chunk(source: &str, target_id: &str, title: &str, description: &str) -> Self {
        let source = BackupSource::ChunkList(source.to_string());
        Self {
            source,
            target: target_id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            type_str: "c2c".to_string(),
            last_checkpoint_index: 1024,
            policy: Value::Array(vec![]),
            priority: 0,
        }
    }

    pub fn dir2chunk(source: &str, target_id: &str, title: &str, description: &str) -> Self {
        unimplemented!()
    }

    pub fn dir2dir(source: &str, target_id: &str, title: &str, description: &str) -> Self {
        unimplemented!()
    }

    pub fn get_plan_key(&self) -> String {
        let key = format!(
            "{}-{}-{}",
            self.type_str,
            self.source.get_source_url(),
            self.target
        );
        return key;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Running,
    Pending,
    Paused,
    Done,
    Failed(String),
}

impl TaskState {
    pub fn to_string(&self) -> String {
        match self {
            TaskState::Running => "RUNNING".to_string(),
            TaskState::Pending => "PENDING".to_string(),
            TaskState::Paused => "PAUSED".to_string(),
            TaskState::Done => "DONE".to_string(),
            TaskState::Failed(msg) => format!("FAILED:{}", msg.as_str()),
        }
    }
}

impl ToSql for TaskState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            TaskState::Running => "RUNNING".to_string(),
            TaskState::Pending => "PENDING".to_string(),
            TaskState::Paused => "PAUSED".to_string(),
            TaskState::Done => "DONE".to_string(),
            TaskState::Failed(msg) => format!("FAILED:{}", msg.as_str()),
        };
        Ok(s.into())
    }
}

impl FromSql for TaskState {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "RUNNING" => TaskState::Running,
            "PENDING" => TaskState::Pending,
            "PAUSED" => TaskState::Paused,
            "DONE" => TaskState::Done,
            _ => {
                let state = s.split_once(|c| c == ':');
                if let Some((state, msg)) = state {
                    if state == "FAILED" {
                        return TaskState::Failed(msg.to_string());
                    }
                }
                TaskState::Failed("UNKNOWN".to_string()) // 默认失败状态
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskType {
    Backup,
    Restore,
}

impl TaskType {
    pub fn to_string(&self) -> &str {
        match self {
            TaskType::Backup => "BACKUP",
            TaskType::Restore => "RESTORE",
        }
    }
}

impl ToSql for TaskType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            TaskType::Backup => "BACKUP",
            TaskType::Restore => "RESTORE",
        };
        Ok(s.into())
    }
}

impl FromSql for TaskType {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "BACKUP" => TaskType::Backup,
            "RESTORE" => TaskType::Restore,
            _ => TaskType::Backup, // 默认备份类型
        })
    }
}

#[derive(Debug, Clone)]
pub struct WorkTask {
    pub taskid: String,
    pub task_type: TaskType,
    pub owner_plan_id: String,
    pub checkpoint_id: String,
    pub total_size: u64,
    pub completed_size: u64,
    pub state: TaskState,
    pub create_time: u64,
    pub update_time: u64,
    pub item_count: u64,
    pub completed_item_count: u64,
    pub wait_transfer_item_count: u64,
    pub restore_config: Option<RestoreConfig>,
}

impl WorkTask {
    pub fn new(owner_plan_id: &str, checkpoint_id: &str, task_type: TaskType) -> Self {
        let new_id = format!("task_{}", Uuid::new_v4());
        Self {
            taskid: new_id.to_string(),
            task_type,
            owner_plan_id: owner_plan_id.to_string(),
            checkpoint_id: checkpoint_id.to_string(),
            total_size: 0,
            completed_size: 0,
            state: TaskState::Paused,
            create_time: chrono::Utc::now().timestamp_millis() as u64,
            update_time: chrono::Utc::now().timestamp_millis() as u64,
            item_count: 0,
            completed_item_count: 0,
            wait_transfer_item_count: 0,
            restore_config: None,
        }
    }

    pub fn set_restore_config(&mut self, restore_config: RestoreConfig) {
        self.restore_config = Some(restore_config);
    }

    pub fn to_json_value(&self) -> Value {
        if self.restore_config.is_some() {
            let restore_config = self.restore_config.as_ref().unwrap();
            let restore_config_json = json!({
                "restore_location_url": restore_config.restore_location_url,
                "is_clean_restore": restore_config.is_clean_restore,
            });
            let result = json!({
                "taskid": self.taskid,
                "task_type": self.task_type.to_string(),
                "owner_plan_id": self.owner_plan_id,
                "checkpoint_id": self.checkpoint_id,
                "total_size": self.total_size,
                "completed_size": self.completed_size,
                "state": self.state.to_string(),
                "create_time": self.create_time,
                "update_time": self.update_time,
                "item_count": self.item_count,
                "completed_item_count": self.completed_item_count,
                "wait_transfer_item_count": self.wait_transfer_item_count,
                "restore_config": restore_config_json,
            });
            return result;
        } else {
            let result = json!({
                "taskid": self.taskid,
                "task_type": self.task_type.to_string(),
                "owner_plan_id": self.owner_plan_id,
                "checkpoint_id": self.checkpoint_id,
                "total_size": self.total_size,
                "completed_size": self.completed_size,
                "state": self.state.to_string(),
                "create_time": self.create_time,
                "update_time": self.update_time,
                "item_count": self.item_count,
                "completed_item_count": self.completed_item_count,
                "wait_transfer_item_count": self.wait_transfer_item_count,
            });
            return result;
        }
    }
}

#[derive(Clone)]
pub struct BackupTaskDb {
    db_path: String,
}

impl BackupTaskDb {
    pub fn new(db_path: &str) -> Self {
        let db = Self {
            db_path: db_path.to_string(),
        };
        db.init_database().expect("Failed to initialize database");
        db
    }

    fn init_database(&self) -> Result<()> {
        let dir =
            std::path::Path::new(&self.db_path)
                .parent()
                .ok_or(BackupDbError::DatabaseError(rusqlite::Error::InvalidPath(
                    std::path::PathBuf::from(self.db_path.clone()),
                )))?;
        std::fs::create_dir_all(dir).map_err(|_| {
            BackupDbError::DatabaseError(rusqlite::Error::InvalidPath(std::path::PathBuf::from(
                self.db_path.clone(),
            )))
        })?;

        let conn = Connection::open(&self.db_path).map_err(BackupDbError::DatabaseError)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS work_tasks (
                taskid TEXT PRIMARY KEY,
                task_type TEXT NOT NULL,
                owner_plan_id TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                completed_size INTEGER NOT NULL,
                state TEXT NOT NULL,
                create_time INTEGER NOT NULL,
                update_time INTEGER NOT NULL,
                item_count INTEGER NOT NULL,
                completed_item_count INTEGER NOT NULL,
                wait_transfer_item_count INTEGER NOT NULL,
                restore_config TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                checkpoint_type TEXT NOT NULL,
                checkpoint_name TEXT NOT NULL,
                prev_checkpoint_id TEXT,
                state TEXT NOT NULL,
                extra_info TEXT NOT NULL,
                create_time INTEGER NOT NULL,
                last_update_time INTEGER NOT NULL,
                item_list_id TEXT,
                item_count INTEGER,
                total_size INTEGER,
                owner_plan_id TEXT NOT NULL,
                crpyto_config TEXT,
                crypto_key TEXT,
                org_item_list_id TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_plans (
                plan_id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                source_url TEXT NOT NULL,
                target_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_url TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                type_str TEXT NOT NULL,
                last_checkpoint_index INTEGER NOT NULL,
                policy TEXT NOT NULL DEFAULT '[]',
                priority INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        // Ensure new columns exist for older databases
        let _ = conn.execute(
            "ALTER TABLE backup_plans ADD COLUMN policy TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE backup_plans ADD COLUMN priority INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE backup_plans ADD COLUMN target_id TEXT", []);
        let _ = conn.execute(
            "UPDATE backup_plans SET target_id = target_url WHERE target_id IS NULL OR target_id = ''",
            [],
        );
        let _ = conn.execute(
            "UPDATE backup_plans
                SET target_id = (
                    SELECT target_id FROM backup_targets
                    WHERE backup_targets.url = backup_plans.target_url
                    LIMIT 1
                )
             WHERE target_url IS NOT NULL
               AND target_id = target_url
               AND EXISTS (
                    SELECT 1 FROM backup_targets
                    WHERE backup_targets.url = backup_plans.target_url
                )",
            [],
        );

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_items (
                item_id TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                chunk_id TEXT,
                local_chunk_id TEXT,
                state TEXT NOT NULL,
                size INTEGER NOT NULL,
                last_update_time INTEGER NOT NULL,
                PRIMARY KEY (item_id, checkpoint_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS worktask_log (
                log_id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                level TEXT NOT NULL,
                owner_task TEXT NOT NULL,
                log_content TEXT NOT NULL,
                log_event_type TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS restore_items (
                item_id TEXT NOT NULL,
                owner_taskid TEXT NOT NULL,
                item_type TEXT NOT NULL,
                chunk_id TEXT,
                quick_hash TEXT,
                state TEXT NOT NULL,
                size INTEGER NOT NULL,
                last_modify_time INTEGER NOT NULL,
                create_time INTEGER NOT NULL,
                PRIMARY KEY (item_id, owner_taskid)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_targets (
                target_id TEXT PRIMARY KEY,
                target_type TEXT NOT NULL,
                url TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL DEFAULT 'UNKNOWN',
                used INTEGER NOT NULL DEFAULT 0,
                total INTEGER NOT NULL DEFAULT 0,
                last_error TEXT NOT NULL DEFAULT '',
                config TEXT
            )",
            [],
        )?;

        Ok(())
    }

    pub fn load_task_by_id(&self, taskid: &str) -> Result<WorkTask> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT * FROM work_tasks WHERE taskid = ?")?;

        let task = stmt
            .query_row(params![taskid], |row| {
                Ok(WorkTask {
                    taskid: row.get(0)?,
                    task_type: row.get(1)?,
                    owner_plan_id: row.get(2)?,
                    checkpoint_id: row.get(3)?,
                    total_size: row.get(4)?,
                    completed_size: row.get(5)?,
                    state: row.get(6)?,
                    create_time: row.get(7)?,
                    update_time: row.get(8)?,
                    item_count: row.get(9)?,
                    completed_item_count: row.get(10)?,
                    wait_transfer_item_count: row.get(11)?,
                    restore_config: row.get(12)?,
                })
            })
            .map_err(|_| BackupDbError::NotFound(taskid.to_string()))?;

        Ok(task)
    }

    pub fn create_task(&self, task: &WorkTask) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO work_tasks VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                task.taskid,
                task.task_type,
                task.owner_plan_id,
                task.checkpoint_id,
                task.total_size,
                task.completed_size,
                task.state,
                task.create_time,
                task.update_time,
                task.item_count,
                task.completed_item_count,
                task.wait_transfer_item_count,
                task.restore_config,
            ],
        )?;
        Ok(())
    }

    pub fn update_task(&self, task: &WorkTask) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let new_task_state;
        match task.state {
            TaskState::Done | TaskState::Failed(_) => {
                new_task_state = task.state.clone();
            }
            _ => new_task_state = TaskState::Paused,
        };
        let rows_affected = conn.execute(
            "UPDATE work_tasks SET 
                task_type = ?2,
                owner_plan_id = ?3,
                checkpoint_id = ?4,
                total_size = ?5,
                completed_size = ?6,
                state = ?7,
                update_time = ?8,
                item_count = ?9,
                completed_item_count = ?10,
                wait_transfer_item_count = ?11
            WHERE taskid = ?1",
            params![
                task.taskid,
                task.task_type,
                task.owner_plan_id,
                task.checkpoint_id,
                task.total_size,
                task.completed_size,
                new_task_state,
                chrono::Utc::now().timestamp_millis() as u64,
                task.item_count,
                task.completed_item_count,
                task.wait_transfer_item_count,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(task.taskid.clone()));
        }
        Ok(())
    }

    pub fn load_checkpoint_by_id(&self, checkpoint_id: &str) -> Result<LocalBackupCheckpoint> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id,checkpoint_type, checkpoint_name, prev_checkpoint_id, state, extra_info, create_time, last_update_time, item_list_id, item_count, total_size, owner_plan_id, crpyto_config, crypto_key, org_item_list_id FROM checkpoints WHERE checkpoint_id = ?"
        )?;

        let checkpoint = stmt
            .query_row(params![checkpoint_id], |row| {
                let checkpoint = BackupCheckpoint {
                    checkpoint_type: row.get(1)?,
                    checkpoint_name: row.get(2)?,
                    prev_checkpoint_id: row.get(3)?,
                    state: row.get(4)?,
                    extra_info: row.get(5)?,
                    create_time: row.get(6)?,
                    last_update_time: row.get(7)?,
                    item_list_id: row.get(8)?,
                    item_count: row.get(9)?,
                    total_size: row.get(10)?,
                };
                Ok(LocalBackupCheckpoint {
                    checkpoint: checkpoint,
                    checkpoint_id: row.get(0)?,
                    owner_plan_id: row.get(11)?,
                    crpyto_config: row.get(12)?,
                    crypto_key: row.get(13)?,
                    org_item_list_id: row.get(14)?,
                })
            })
            .map_err(|_| BackupDbError::InvalidCheckpointId(checkpoint_id.to_string()))?;

        Ok(checkpoint)
    }

    pub fn cancel_task(&self, taskid: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE work_tasks SET state = ? WHERE taskid = ?",
            params![TaskState::Failed("CANCEL".to_string()), taskid],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(taskid.to_string()));
        }
        Ok(())
    }

    pub fn save_backup_item(&self, checkpoint_id: &str, item: &BackupChunkItem) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let local_chunk_id = item.local_chunk_id.clone().map(|id| id.to_string());
        conn.execute(
            "INSERT INTO backup_items VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item.item_id,
                checkpoint_id,
                item.chunk_id.to_string(),
                local_chunk_id.unwrap_or("".to_string()),
                item.state,
                item.size,
                item.last_update_time,
            ],
        )?;
        Ok(())
    }

    pub fn save_itemlist_to_checkpoint(
        &self,
        checkpoint_id: &str,
        item_list: &Vec<BackupChunkItem>,
    ) -> Result<()> {
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;

        // optimize: per checkpoint per table?
        // tx.execute(
        //     "CREATE TABLE IF NOT EXISTS {}_backup_items (
        //         item_id TEXT NOT NULL,
        //         checkpoint_id TEXT NOT NULL,
        //         item_type TEXT NOT NULL,
        //         chunk_id TEXT,
        //         quick_hash TEXT,
        //         state TEXT NOT NULL,
        //         size INTEGER NOT NULL,
        //         last_modify_time INTEGER NOT NULL,
        //         create_time INTEGER NOT NULL,
        //         PRIMARY KEY (item_id, checkpoint_id)
        //     )",
        //     [],
        // )?;

        for item in item_list {
            let local_chunk_id = item.local_chunk_id.clone().map(|id| id.to_string());
            tx.execute(
                "INSERT INTO backup_items VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    item.item_id,
                    checkpoint_id,
                    item.chunk_id.to_string(),
                    local_chunk_id.unwrap_or("".to_string()),
                    item.state,
                    item.size,
                    item.last_update_time,
                ],
            )?;
        }

        tx.commit()?;
        info!(
            "taskdb.save_item_list_to_checkpoint: {} {} items",
            checkpoint_id,
            item_list.len()
        );
        Ok(())
    }

    pub fn create_checkpoint(&self, checkpoint: &LocalBackupCheckpoint) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO checkpoints VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                checkpoint.checkpoint_id,
                checkpoint.checkpoint_type,
                checkpoint.checkpoint_name,
                checkpoint.prev_checkpoint_id,
                checkpoint.state,
                checkpoint.extra_info,
                checkpoint.create_time,
                checkpoint.last_update_time,
                checkpoint.item_list_id,
                checkpoint.item_count,
                checkpoint.total_size,
                checkpoint.owner_plan_id,
                checkpoint.crpyto_config,
                checkpoint.crypto_key,
                checkpoint.org_item_list_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_checkpoint_state(
        &self,
        checkpoint_id: &str,
        state: CheckPointState,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE checkpoints SET state = ? WHERE checkpoint_id = ?",
            params![state, checkpoint_id],
        )?;
        Ok(())
    }

    pub fn set_checkpoint_ready(
        &self,
        checkpoint_id: &str,
        total_size: u64,
        item_count: u64,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE checkpoints SET total_size = ?, item_count = ?, state = 'PREPARED',last_update_time = ? WHERE checkpoint_id = ?",
            params![total_size, item_count, buckyos_kit::buckyos_get_unix_timestamp() as u64, checkpoint_id],
        )?;
        if rows_affected == 0 {
            return Err(BackupDbError::InvalidCheckpointId(
                checkpoint_id.to_string(),
            ));
        }
        Ok(())
    }

    // pub fn update_checkpoint(&self, checkpoint: &LocalBackupCheckpoint) -> Result<()> {
    //     let conn = Connection::open(&self.db_path)?;
    //     let rows_affected = conn.execute(
    //         "UPDATE checkpoints SET
    //             depend_checkpoint_id = ?2,
    //             prev_checkpoint_id = ?3,
    //             state = ?4,
    //             owner_plan = ?5,
    //             checkpoint_hash = ?6,
    //             checkpoint_index = ?7,
    //             create_time = ?8
    //         WHERE checkpoint_id = ?1",
    //         params![
    //             checkpoint.checkpoint_id,
    //             checkpoint.depend_checkpoint_id,
    //             checkpoint.prev_checkpoint_id,
    //             checkpoint.state,
    //             checkpoint.owner_plan,
    //             checkpoint.checkpoint_hash,
    //             checkpoint.checkpoint_index,
    //             checkpoint.create_time,
    //         ],
    //     )?;

    //     if rows_affected == 0 {
    //         return Err(BackupTaskError::InvalidCheckpointId);
    //     }
    //     Ok(())
    // }

    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "DELETE FROM checkpoints WHERE checkpoint_id = ?",
            params![checkpoint_id],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::InvalidCheckpointId(
                checkpoint_id.to_string(),
            ));
        }
        Ok(())
    }

    pub fn load_backup_chunk_items_by_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<Vec<BackupChunkItem>> {
        unimplemented!()
    }

    pub fn pop_wait_backup_item(&self, checkpoint_id: &str) -> Result<Option<BackupChunkItem>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT item_id, chunk_id, size,last_update_time FROM backup_items WHERE checkpoint_id = ? AND state = 'NEW'  ORDER BY last_update_time LIMIT 1"
        )?;
        let item = stmt.query_row(params![checkpoint_id], |row| {
            let chunk_id_str: String = row.get(1)?;
            let chunk_id = ChunkId::new(&chunk_id_str).unwrap();
            Ok(BackupChunkItem {
                item_id: row.get(0)?,
                chunk_id: chunk_id,
                local_chunk_id: None,
                size: row.get(2)?,
                state: BackupItemState::New,
                last_update_time: row.get(3)?,
            })
        });
        //处理没返回记录的情况
        match item {
            Ok(item) => Ok(Some(item)),
            Err(e) => {
                if let rusqlite::Error::QueryReturnedNoRows = e {
                    return Ok(None);
                }
                return Err(BackupDbError::DatabaseError(e));
            }
        }
    }

    pub fn check_is_checkpoint_items_all_done(&self, checkpoint_id: &str) -> Result<bool> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM backup_items WHERE checkpoint_id = ? AND state != 'DONE'",
        )?;
        let count: i32 = stmt.query_row(params![checkpoint_id], |row| row.get(0))?;
        Ok(count == 0)
    }

    pub fn update_backup_item_state(
        &self,
        checkpoint_id: &str,
        item_id: &str,
        state: BackupItemState,
    ) -> Result<()> {
        info!(
            "taskdb.update_backup_item_state: {} {} {:?}",
            checkpoint_id, item_id, state
        );
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE backup_items SET state = ?1 
            WHERE checkpoint_id = ?2 AND item_id = ?3",
            params![state, checkpoint_id, item_id,],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(format!(
                "{}/{}",
                checkpoint_id, item_id
            )));
        }

        Ok(())
    }

    pub fn create_backup_plan(&self, plan: &BackupPlanConfig) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let policy_str = serde_json::to_string(&plan.policy)
            .map_err(|e| BackupDbError::DataFormatError(e.to_string()))?;
        let target_record = self.get_backup_target(&plan.target)?;
        conn.execute(
            "INSERT INTO backup_plans (
                plan_id,
                source_type,
                source_url,
                target_id,
                target_type,
                target_url,
                title,
                description,
                type_str,
                last_checkpoint_index,
                policy,
                priority
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                plan.get_plan_key(),
                match &plan.source {
                    BackupSource::Directory(_) => "directory",
                    BackupSource::ChunkList(_) => "chunklist",
                },
                plan.source.get_source_url(),
                plan.target.as_str(),
                target_record.target_type,
                target_record.url,
                plan.title,
                plan.description,
                plan.type_str,
                plan.last_checkpoint_index,
                policy_str,
                plan.priority,
            ],
        )?;
        Ok(())
    }

    pub fn update_backup_plan(&self, plan: &BackupPlanConfig) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let policy_str = serde_json::to_string(&plan.policy)
            .map_err(|e| BackupDbError::DataFormatError(e.to_string()))?;
        let target_record = self.get_backup_target(&plan.target)?;
        let rows_affected = conn.execute(
            "UPDATE backup_plans SET 
                source_type = ?2,
                source_url = ?3,
                target_id = ?4,
                target_type = ?5,
                target_url = ?6,
                title = ?7,
                description = ?8,
                type_str = ?9,
                last_checkpoint_index = ?10,
                policy = ?11,
                priority = ?12
            WHERE plan_id = ?1",
            params![
                plan.get_plan_key(),
                match &plan.source {
                    BackupSource::Directory(_) => "directory",
                    BackupSource::ChunkList(_) => "chunklist",
                },
                plan.source.get_source_url(),
                plan.target.as_str(),
                target_record.target_type,
                target_record.url,
                plan.title,
                plan.description,
                plan.type_str,
                plan.last_checkpoint_index,
                policy_str,
                plan.priority,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(plan.get_plan_key()));
        }
        Ok(())
    }

    pub fn delete_backup_plan(&self, plan_id: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "DELETE FROM backup_plans WHERE plan_id = ?",
            params![plan_id],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(plan_id.to_string()));
        }
        Ok(())
    }

    pub fn list_backup_plans(&self) -> Result<Vec<BackupPlanConfig>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT
                plan_id,
                source_type,
                source_url,
                target_id,
                target_type,
                target_url,
                title,
                description,
                type_str,
                last_checkpoint_index,
                policy,
                priority
            FROM backup_plans",
        )?;

        let plans = stmt
            .query_map([], |row| {
                let plan_id: String = row.get(0)?;
                let source_type: String = row.get(1)?;
                let source_url: String = row.get(2)?;
                let target_id: String = row.get::<_, Option<String>>(3)?.unwrap_or_default();
                let target_url: String = row.get(5)?;
                let policy_json: String = row.get(10)?;
                let priority: i64 = row.get(11)?;

                let policy_value = match serde_json::from_str(&policy_json) {
                    Ok(value) => value,
                    Err(err) => {
                        warn!(
                            "failed to parse policy json for plan {}: {}. Using empty array.",
                            plan_id, err
                        );
                        Value::Array(vec![])
                    }
                };

                let target = if target_id.is_empty() {
                    warn!(
                        "backup plan {} missing target_id in database, falling back to target_url",
                        plan_id
                    );
                    target_url.clone()
                } else {
                    target_id
                };

                Ok(BackupPlanConfig {
                    source: match source_type.as_str() {
                        "directory" => BackupSource::Directory(source_url),
                        "chunklist" => BackupSource::ChunkList(source_url),
                        _ => panic!("Invalid source type in database"),
                    },
                    target,
                    title: row.get(6)?,
                    description: row.get(7)?,
                    type_str: row.get(8)?,
                    last_checkpoint_index: row.get(9)?,
                    policy: policy_value,
                    priority,
                })
            })?
            .collect::<SqlResult<Vec<BackupPlanConfig>>>()?;

        Ok(plans)
    }

    pub fn create_backup_target(&self, target: &BackupTargetRecord) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let used = i64::try_from(target.used).map_err(|_| {
            BackupDbError::DataFormatError("target.used exceeds i64 range".to_string())
        })?;
        let total = i64::try_from(target.total).map_err(|_| {
            BackupDbError::DataFormatError("target.total exceeds i64 range".to_string())
        })?;
        let config_str = match &target.config {
            Some(value) => Some(
                serde_json::to_string(value)
                    .map_err(|e| BackupDbError::DataFormatError(e.to_string()))?,
            ),
            None => None,
        };

        conn.execute(
            "INSERT INTO backup_targets (
                target_id,
                target_type,
                url,
                name,
                description,
                state,
                used,
                total,
                last_error,
                config
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                target.target_id,
                target.target_type,
                target.url,
                target.name,
                target.description,
                target.state,
                used,
                total,
                target.last_error,
                config_str,
            ],
        )?;
        Ok(())
    }

    pub fn list_backup_target_ids(&self) -> Result<Vec<String>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt =
            conn.prepare("SELECT target_id FROM backup_targets ORDER BY name COLLATE NOCASE")?;
        let targets = stmt
            .query_map([], |row| Ok(row.get(0)?))?
            .collect::<SqlResult<Vec<String>>>()?;
        Ok(targets)
    }

    pub fn get_backup_target(&self, target_id: &str) -> Result<BackupTargetRecord> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT
                target_id,
                target_type,
                url,
                name,
                description,
                state,
                used,
                total,
                last_error,
                config
            FROM backup_targets WHERE target_id = ?",
        )?;

        let row = stmt
            .query_row(params![target_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    BackupDbError::NotFound(target_id.to_string())
                }
                _ => BackupDbError::DatabaseError(err),
            })?;

        let (
            target_id,
            target_type,
            url,
            name,
            description,
            state,
            used_raw,
            total_raw,
            last_error,
            config_raw,
        ) = row;

        let config = match config_raw {
            Some(text) => Some(
                serde_json::from_str(&text)
                    .map_err(|e| BackupDbError::DataFormatError(e.to_string()))?,
            ),
            None => None,
        };

        Ok(BackupTargetRecord {
            target_id,
            target_type,
            url,
            name,
            description,
            state,
            used: u64::try_from(used_raw).map_err(|_| {
                BackupDbError::DataFormatError("target.used stored value is negative".to_string())
            })?,
            total: u64::try_from(total_raw).map_err(|_| {
                BackupDbError::DataFormatError("target.total stored value is negative".to_string())
            })?,
            last_error,
            config,
        })
    }

    pub fn update_backup_target(&self, target: &BackupTargetRecord) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let used = i64::try_from(target.used).map_err(|_| {
            BackupDbError::DataFormatError("target.used exceeds i64 range".to_string())
        })?;
        let total = i64::try_from(target.total).map_err(|_| {
            BackupDbError::DataFormatError("target.total exceeds i64 range".to_string())
        })?;
        let config_str = match &target.config {
            Some(value) => Some(
                serde_json::to_string(value)
                    .map_err(|e| BackupDbError::DataFormatError(e.to_string()))?,
            ),
            None => None,
        };

        let rows_affected = conn.execute(
            "UPDATE backup_targets SET
                target_type = ?2,
                url = ?3,
                name = ?4,
                description = ?5,
                state = ?6,
                used = ?7,
                total = ?8,
                last_error = ?9,
                config = ?10
            WHERE target_id = ?1",
            params![
                target.target_id,
                target.target_type,
                target.url,
                target.name,
                target.description,
                target.state,
                used,
                total,
                target.last_error,
                config_str,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(target.target_id.clone()));
        }
        Ok(())
    }

    pub fn remove_backup_target(&self, target_id: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "DELETE FROM backup_targets WHERE target_id = ?",
            params![target_id],
        )?;
        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(target_id.to_string()));
        }
        Ok(())
    }

    //return all task ids
    pub fn list_worktasks(&self, filter: &str) -> Result<Vec<String>> {
        let conn = Connection::open(&self.db_path)?;
        let sql;
        match filter {
            "running" => sql = "SELECT taskid FROM work_tasks WHERE state = 'RUNNING'",
            "paused" => sql = "SELECT taskid FROM work_tasks WHERE state = 'PAUSED'",
            "failed" => sql = "SELECT taskid FROM work_tasks WHERE state = 'FAILED'",
            "pending" => sql = "SELECT taskid FROM work_tasks WHERE state = 'PENDING'",
            "done" => sql = "SELECT taskid FROM work_tasks WHERE state = 'DONE'",
            _ => sql = "SELECT taskid FROM work_tasks",
        }
        let mut stmt = conn.prepare(sql)?;
        let tasks = stmt
            .query_map([], |row| Ok(row.get(0)?))?
            .collect::<SqlResult<Vec<String>>>()?;
        Ok(tasks)
    }

    pub fn add_worktask_log(
        &self,
        timestamp: u64,
        level: &str,
        owner_task: &str,
        log_content: &str,
        log_event_type: &str,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO worktask_log (timestamp, level, owner_task, log_content, log_event_type) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timestamp, level, owner_task, log_content, log_event_type],
        )?;
        Ok(())
    }

    pub fn get_worktask_logs(
        &self,
        owner_task: &str,
    ) -> Result<Vec<(u64, String, String, String, String)>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT timestamp, level, owner_task, log_content, log_event_type FROM worktask_log WHERE owner_task = ?"
        )?;

        let logs = stmt
            .query_map(params![owner_task], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<SqlResult<Vec<(u64, String, String, String, String)>>>()?;

        Ok(logs)
    }

    // pub fn save_restore_item_list_to_task(&self, owner_taskid: &str, item_list: &Vec<BackupChunkItem>) -> Result<()> {
    //     let mut conn = Connection::open(&self.db_path)?;
    //     let tx = conn.transaction()?;

    //     for item in item_list {
    //         tx.execute(
    //             "INSERT INTO restore_items (
    //                 item_id,
    //                 owner_taskid,
    //                 item_type,
    //                 chunk_id,
    //                 quick_hash,
    //                 state,
    //                 size,
    //                 last_modify_time,
    //                 create_time
    //             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    //             params![
    //                 item.item_id,
    //                 owner_taskid,
    //                 item.item_type,
    //                 item.chunk_id,
    //                 item.quick_hash,
    //                 item.state,
    //                 item.size,
    //                 item.last_modify_time,
    //                 item.create_time,
    //             ],
    //         )?;
    //     }

    //     tx.commit()?;
    //     info!("taskdb.save_restore_item_list_to_task: {} {} items", owner_taskid, item_list.len());
    //     Ok(())
    // }

    // pub fn load_restore_items_by_task(&self, owner_taskid: &str,state: &BackupItemState) -> Result<Vec<BackupChunkItem>> {
    //     let conn = Connection::open(&self.db_path)?;
    //     let mut stmt = conn.prepare(
    //         "SELECT item_id, item_type, chunk_id, quick_hash, state, size,
    //                 last_modify_time, create_time, progress, diff_info
    //          FROM restore_items WHERE owner_taskid = ? AND state = ?"
    //     )?;

    //     let items = stmt.query_map(params![owner_taskid, state], |row| {
    //         Ok(BackupChunkItem {
    //             item_id: row.get(0)?,
    //             item_type: row.get(1)?,
    //             chunk_id: row.get(2)?,
    //             quick_hash: row.get(3)?,
    //             state: row.get(4)?,
    //             size: row.get(5)?,
    //             last_modify_time: row.get(6)?,
    //             create_time: row.get(7)?,
    //             have_cache: false,
    //             progress: row.get(8)?,
    //             diff_info: Some(row.get(9)?),
    //         })
    //     })?
    //     .collect::<SqlResult<Vec<BackupChunkItem>>>()?;

    //     Ok(items)
    // }

    // pub fn update_restore_item(&self, owner_taskid: &str, item: &BackupChunkItem) -> Result<()> {
    //     info!("taskdb.update_restore_item: {} {} {:?}", owner_taskid, item.item_id, item.state);
    //     let conn = Connection::open(&self.db_path)?;
    //     let rows_affected = conn.execute(
    //         "UPDATE restore_items SET
    //             item_type = ?1,
    //             chunk_id = ?2,
    //             quick_hash = ?3,
    //             state = ?4,
    //             size = ?5,
    //             last_modify_time = ?6,
    //             create_time = ?7
    //         WHERE owner_taskid = ?8 AND item_id = ?9",
    //         params![
    //             item.item_type,
    //             item.chunk_id,
    //             item.quick_hash,
    //             item.state,
    //             item.size,
    //             item.last_modify_time,
    //             item.create_time,
    //             owner_taskid,
    //             item.item_id,
    //         ],
    //     )?;

    //     if rows_affected == 0 {
    //         return Err(BackupTaskError::TaskNotFound);
    //     }

    //     Ok(())
    // }

    pub fn update_restore_item_state(
        &self,
        owner_taskid: &str,
        item_id: &str,
        state: BackupItemState,
    ) -> Result<()> {
        info!(
            "taskdb.update_restore_item_state: {} {} {:?}",
            owner_taskid, item_id, state
        );
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE restore_items SET state = ?1 
            WHERE owner_taskid = ?2 AND item_id = ?3",
            params![state, owner_taskid, item_id,],
        )?;

        if rows_affected == 0 {
            return Err(BackupDbError::NotFound(format!(
                "{}/{}",
                owner_taskid, item_id
            )));
        }

        Ok(())
    }

    // pub fn load_wait_transfer_restore_items(&self, owner_taskid: &str) -> Result<Vec<BackupChunkItem>> {
    //     let conn = Connection::open(&self.db_path)?;
    //     let mut stmt = conn.prepare(
    //         "SELECT item_id, item_type, chunk_id, quick_hash, size,
    //                 last_modify_time, create_time, progress, diff_info
    //          FROM restore_items
    //          WHERE owner_taskid = ? AND state = ?"
    //     )?;

    //     let items = stmt.query_map(
    //         params![
    //             owner_taskid,
    //             BackupItemState::LocalDone,
    //         ],
    //         |row| {
    //             Ok(BackupChunkItem {
    //                 item_id: row.get(0)?,
    //                 item_type: row.get(1)?,
    //                 chunk_id: row.get(2)?,
    //                 quick_hash: row.get(3)?,
    //                 state: row.get(4)?,
    //                 size: row.get(5)?,
    //                 last_modify_time: row.get(6)?,
    //                 create_time: row.get(7)?,
    //                 have_cache: false,
    //                 progress: row.get(8)?,
    //                 diff_info: Some(row.get(9)?),
    //             })
    //         }
    //     )?
    //     .collect::<SqlResult<Vec<BackupChunkItem>>>()?;

    //     Ok(items)
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    fn setup_test_db() -> (BackupTaskDb, String) {
        let db_path = "/tmp/test.db".to_string();
        println!("db_path: {}", db_path);
        let db = BackupTaskDb::new(&db_path);
        (db, db_path)
    }

    #[test]
    fn test_create_and_load_task() {
        let (db, _) = setup_test_db();

        // Create a test task
        let task = WorkTask::new("test_plan", "test_checkpoint", TaskType::Backup);
        db.create_task(&task).unwrap();

        // Load and verify the task
        let loaded_task = db.load_task_by_id(&task.taskid).unwrap();
        assert_eq!(loaded_task.taskid, task.taskid);
        assert_eq!(loaded_task.owner_plan_id, task.owner_plan_id);
        assert_eq!(loaded_task.checkpoint_id, task.checkpoint_id);
        assert_eq!(loaded_task.task_type, task.task_type);
    }

    #[test]
    fn test_update_task() {
        let (db, _) = setup_test_db();

        // Create initial task
        let mut task = WorkTask::new("test_plan", "test_checkpoint", TaskType::Backup);
        db.create_task(&task).unwrap();

        // Update task
        task.total_size = 1000;
        task.completed_size = 500;
        db.update_task(&task).unwrap();

        // Verify updates
        let loaded_task = db.load_task_by_id(&task.taskid).unwrap();
        assert_eq!(loaded_task.total_size, 1000);
        assert_eq!(loaded_task.completed_size, 500);
    }

    #[test]
    fn test_task_state_transitions() {
        let (db, _) = setup_test_db();

        // Create task
        let task = WorkTask::new("test_plan", "test_checkpoint", TaskType::Backup);
        let task_id = task.taskid.clone();
        db.create_task(&task).unwrap();

        // Test pause
        //db.pause_task(&task_id).unwrap();
        //let paused_task = db.load_task_by_id(&task_id).unwrap();
        //assert_eq!(paused_task.state, TaskState::Paused);

        // Test resume
        //db.resume_task(&task_id).unwrap();
        //let resumed_task = db.load_task_by_id(&task_id).unwrap();
        //assert_eq!(resumed_task.state, TaskState::Running);

        // Test cancel
        db.cancel_task(&task_id).unwrap();
        let cancelled_task = db.load_task_by_id(&task_id).unwrap();
        assert_eq!(cancelled_task.state, TaskState::Failed);
        //db.delete_task(&task_id).unwrap();
    }

    #[test]
    fn test_checkpoint_operations() {
        let (db, _) = setup_test_db();

        let checkpoint = BackupCheckpoint::new(
            CHECKPOINT_TYPE_CHUNK.to_string(),
            "test_checkpoint".to_string(),
            None,
            None,
        );
        let checkpoint_id = uuid::Uuid::new_v4().to_string();
        // Create checkpoint
        let local_checkpoint =
            LocalBackupCheckpoint::new(checkpoint, checkpoint_id.clone(), "test_plan".to_string());
        db.create_checkpoint(&local_checkpoint).unwrap();

        // Load and verify
        let loaded_cp = db.load_checkpoint_by_id(&checkpoint_id).unwrap();
        assert_eq!(loaded_cp.checkpoint_id, checkpoint_id);
        assert_eq!(loaded_cp.owner_plan_id, "test_plan");

        // Update checkpoint
        db.update_checkpoint_state(&checkpoint_id, CheckPointState::Prepared)
            .unwrap();

        // Verify update
        let loaded_cp = db.load_checkpoint_by_id(&checkpoint_id).unwrap();
        assert_eq!(loaded_cp.state, CheckPointState::Prepared);
    }

    #[test]
    fn test_error_handling() {
        let (db, _) = setup_test_db();

        // Test loading non-existent task
        let result = db.load_task_by_id("non_existent_task");
        assert!(matches!(result, Err(BackupDbError::NotFound(_))));

        // Test loading non-existent checkpoint
        let result = db.load_checkpoint_by_id("non_existent_checkpoint");
        assert!(matches!(result, Err(BackupDbError::InvalidCheckpointId(_))));
    }
}
