use ndn_lib::ChunkId;
use thiserror::Error;
use uuid::Uuid;
use serde_json::{Value, json};
use rusqlite::{Connection, params, Result as SqlResult};
use rusqlite::types::{ToSql, FromSql, ValueRef};
use buckyos_backup_lib::*;
use log::*;


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
pub enum BackupTaskError {
    #[error("task not found")]
    TaskNotFound,
    #[error("invalid checkpoint id")]
    InvalidCheckpointId,
    #[error("database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, BackupTaskError>;

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

#[derive(Debug, Clone)]
pub enum BackupTarget {
    Directory(String),
    ChunkList(String),
}

impl BackupTarget {
    pub fn get_target_url(&self) -> &str {
        match self {
            BackupTarget::Directory(url) => url.as_str(),
            BackupTarget::ChunkList(url) => url.as_str(),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckPointState {
    New,
    Prepared,//所有的backup item确认了
    Evaluated,//所有的backup item都计算了hash和diff(如有需要)
    Done,
    Failed,
}

impl ToSql for CheckPointState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            CheckPointState::New => "NEW",
            CheckPointState::Prepared => "PREPARED",
            CheckPointState::Evaluated => "EVALUATED",
            CheckPointState::Done => "DONE",
            CheckPointState::Failed => "FAILED",
        };
        Ok(s.into())
    }
}

impl FromSql for CheckPointState {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(|s| match s {
            "NEW" => CheckPointState::New,
            "PREPARED" => CheckPointState::Prepared,
            "EVALUATED" => CheckPointState::Evaluated,
            "DONE" => CheckPointState::Done,
            "FAILED" => CheckPointState::Failed,
            _ => CheckPointState::Failed, // 默认失败状态
        })
    }
}

pub struct BackupCheckPoint {
    pub checkpoint_id: String,
    pub parent_checkpoint_id: Option<String>,
    pub state:CheckPointState,
    pub owner_plan:String,
    pub checkpoint_hash:Option<String>,
    pub checkpoint_index:u64,
    pub create_time: u64, //checkpoint的顺序很重要，因此不能用时间来排序（这可能会因为时间错误带来严重的BUG）

    //pub small_content_cache:HashMap<String, Vec<u8>>,
}

impl BackupCheckPoint {
    pub fn new(owner_plan: &str, parent_checkpoint_id: Option<&str>, checkpoint_index: u64) -> Self {
        let new_id = format!("chk_{}" ,Uuid::new_v4());
        Self {
            checkpoint_id: new_id,
            owner_plan: owner_plan.to_string(),
            parent_checkpoint_id: parent_checkpoint_id.map(|s| s.to_string()),
            state: CheckPointState::New,
            checkpoint_hash: None,
            checkpoint_index,
            create_time: (chrono::Utc::now().timestamp_millis() as u64),
        }
    }
}


#[derive(Debug, Clone)]
pub struct BackupPlanConfig {
    pub source: BackupSource,
    pub target: BackupTarget,
    pub title: String,
    pub description: String,
    pub type_str: String,
    pub last_checkpoint_index: u64,
}

impl BackupPlanConfig {
    pub fn to_json_value(&self) -> Value {
        let result = json!({
            "source": self.source.get_source_url(),
            "target": self.target.get_target_url(),
            "title": self.title,
            "description": self.description,
            "type_str": self.type_str,
            "last_checkpoint_index": self.last_checkpoint_index,
        });
        result
    }

    pub fn chunk2chunk(source:&str,target_url: &str, title: &str, description: &str) -> Self {
        let source = BackupSource::ChunkList(source.to_string());
        let target = BackupTarget::ChunkList(target_url.to_string());
        Self { 
            source, 
            target,
            title: title.to_string(), 
            description: description.to_string() ,
            type_str: "c2c".to_string(),
            last_checkpoint_index: 1024,
        }
    }

    pub fn dir2chunk(source:&str,target_url: &str, title: &str, description: &str) -> Self {
        unimplemented!()
    }

    pub fn dir2dir(source:&str,target_url: &str, title: &str, description: &str) -> Self {
        unimplemented!()
    }

    pub fn get_plan_key(&self) -> String {
        let key =format!("{}-{}-{}",self.type_str, self.source.get_source_url(), self.target.get_target_url());
        return key;
    }

}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Running,
    Pending,
    Paused,
    Done,
    Failed,
}

impl TaskState {
    pub fn to_string(&self) -> &str {
        match self {
            TaskState::Running => "RUNNING",
            TaskState::Pending => "PENDING",
            TaskState::Paused => "PAUSED",
            TaskState::Done => "DONE",
            TaskState::Failed => "FAILED",
        }
    }
}

impl ToSql for TaskState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let s = match self {
            TaskState::Running => "RUNNING",
            TaskState::Pending => "PENDING",
            TaskState::Paused => "PAUSED",
            TaskState::Done => "DONE",
            TaskState::Failed => "FAILED",
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
            "FAILED" => TaskState::Failed,
            _ => TaskState::Failed, // 默认失败状态
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

#[derive(Clone, Debug)]
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
}

impl WorkTask {
    pub fn new(owner_plan_id: &str, checkpoint_id: &str, task_type: TaskType) -> Self {
        let new_id = format!("chk_{}" ,Uuid::new_v4());
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
        }
    }

    pub fn to_json_value(&self) -> Value {
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
        result
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
        let dir = std::path::Path::new(&self.db_path).parent()
            .ok_or(BackupTaskError::DatabaseError(rusqlite::Error::InvalidPath(std::path::PathBuf::from(self.db_path.clone()))))?;
        std::fs::create_dir_all(dir)
            .map_err(|_| BackupTaskError::DatabaseError(rusqlite::Error::InvalidPath(std::path::PathBuf::from(self.db_path.clone()))))?;
        
        let conn = Connection::open(&self.db_path).map_err(BackupTaskError::DatabaseError)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS work_tasks (
                taskid TEXT PRIMARY KEY,
                task_type task_type NOT NULL,
                owner_plan_id TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                completed_size INTEGER NOT NULL,
                state task_state NOT NULL,
                create_time INTEGER NOT NULL,
                update_time INTEGER NOT NULL,
                item_count INTEGER NOT NULL,
                completed_item_count INTEGER NOT NULL,
                wait_transfer_item_count INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                parent_checkpoint_id TEXT,
                state TEXT NOT NULL,
                owner_plan TEXT NOT NULL,
                checkpoint_hash TEXT,
                checkpoint_index INTEGER NOT NULL,
                create_time INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_plans (
                plan_id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                source_url TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_url TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                type_str TEXT NOT NULL,
                last_checkpoint_index INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_items (
                item_id TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                item_type TEXT NOT NULL,
                chunk_id TEXT,
                quick_hash TEXT,
                state TEXT NOT NULL,
                size INTEGER NOT NULL,
                last_modify_time INTEGER NOT NULL,
                create_time INTEGER NOT NULL,
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

        Ok(())
    }

    pub fn load_task_by_id(&self, taskid: &str) -> Result<WorkTask> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT * FROM work_tasks WHERE taskid = ?"
        )?;
        
        let task = stmt.query_row(params![taskid], |row| {
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
            })
        }).map_err(|_| BackupTaskError::TaskNotFound)?;

        Ok(task)
    }

    pub fn create_task(&self, task: &WorkTask) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO work_tasks VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
            ],
        )?;
        Ok(())
    }

    pub fn update_task(&self, task: &WorkTask) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let new_task_state;
        if task.state == TaskState::Done || task.state == TaskState::Failed || task.state == TaskState::Pending {
            new_task_state = task.state.clone();
        } else {
            new_task_state = TaskState::Paused;
        }
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
            return Err(BackupTaskError::TaskNotFound);
        }
        Ok(())
    }


    pub fn load_last_checkpoint(&self, taskid: &str, count:Option<u32>) -> Result<BackupCheckPoint> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT * FROM checkpoints WHERE taskid = ?1 ORDER BY create_time DESC LIMIT ?2")?;
        let mut rows = stmt.query(params![taskid, count.unwrap_or(1)])?;

        if let Some(row) = rows.next()? {
            let checkpoint = BackupCheckPoint {
                checkpoint_id: row.get(0)?,
                parent_checkpoint_id: row.get(1)?,
                state: row.get(2)?,
                owner_plan: row.get(3)?,
                checkpoint_hash: row.get(4)?,
                checkpoint_index: row.get(5)?,
                create_time: row.get(6)?,
            };
            Ok(checkpoint)
        } else {
            Err(BackupTaskError::InvalidCheckpointId)
        }
    }

    pub fn load_checkpoint_by_id(&self, checkpoint_id: &str) -> Result<BackupCheckPoint> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT * FROM checkpoints WHERE checkpoint_id = ?"
        )?;
        
        let checkpoint = stmt.query_row(params![checkpoint_id], |row| {
            Ok(BackupCheckPoint {
                checkpoint_id: row.get(0)?,
                parent_checkpoint_id: row.get(1)?,
                state: row.get(2)?,
                owner_plan: row.get(3)?,
                checkpoint_hash: row.get(4)?,
                checkpoint_index: row.get(5)?,
                create_time: row.get(6)?,
            })
        }).map_err(|_| BackupTaskError::InvalidCheckpointId)?;

        Ok(checkpoint)
    }


    pub fn cancel_task(&self, taskid: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE work_tasks SET state = ? WHERE taskid = ?",
            params![
                TaskState::Failed,
                taskid
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupTaskError::TaskNotFound);
        }
        Ok(())
    }

    pub fn save_item_list_to_checkpoint(&self, checkpoint_id: &str, item_list: &Vec<BackupItem>) -> Result<()> {
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
            tx.execute(
                "INSERT INTO backup_items (
                    item_id,
                    checkpoint_id,
                    item_type,
                    chunk_id,
                    quick_hash,
                    state,
                    size,
                    last_modify_time,
                    create_time
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    item.item_id,
                    checkpoint_id,
                    item.item_type,
                    item.chunk_id,
                    item.quick_hash,
                    item.state,
                    item.size,
                    item.last_modify_time,
                    item.create_time,
                ],
            )?;
        }

        tx.commit()?;
        info!("taskdb.save_item_list_to_checkpoint: {} {} items", checkpoint_id, item_list.len());
        Ok(())
    }

    pub fn create_checkpoint(&self, checkpoint: &BackupCheckPoint) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO checkpoints VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                checkpoint.checkpoint_id,
                checkpoint.parent_checkpoint_id,
                checkpoint.state,
                checkpoint.owner_plan,
                checkpoint.checkpoint_hash,
                checkpoint.checkpoint_index,
                checkpoint.create_time,
            ],
        )?;
        Ok(())
    }

    pub fn update_checkpoint(&self, checkpoint: &BackupCheckPoint) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE checkpoints SET 
                parent_checkpoint_id = ?2,
                state = ?3,
                owner_plan = ?4,
                checkpoint_hash = ?5,
                checkpoint_index = ?6,
                create_time = ?7
            WHERE checkpoint_id = ?1",
            params![
                checkpoint.checkpoint_id,
                checkpoint.parent_checkpoint_id,
                checkpoint.state,
                checkpoint.owner_plan,
                checkpoint.checkpoint_hash,
                checkpoint.checkpoint_index,
                checkpoint.create_time,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupTaskError::InvalidCheckpointId);
        }
        Ok(())
    }

    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "DELETE FROM checkpoints WHERE checkpoint_id = ?",
            params![checkpoint_id],
        )?;

        if rows_affected == 0 {
            return Err(BackupTaskError::InvalidCheckpointId);
        }
        Ok(())
    }

    pub fn load_work_backup_items(&self, checkpoint_id: &str) -> Result<Vec<BackupItem>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT item_id, item_type, chunk_id, quick_hash, state, size, 
                    last_modify_time, create_time 
             FROM backup_items WHERE checkpoint_id = ?"
        )?;
        
        let items = stmt.query_map(params![checkpoint_id], |row| {
            Ok(BackupItem {
                item_id: row.get(0)?,
                item_type: row.get(1)?,
                chunk_id: row.get(2)?,
                quick_hash: row.get(3)?,
                state: row.get(4)?,
                size: row.get(5)?,
                last_modify_time: row.get(6)?,
                create_time: row.get(7)?,
            })
        })?
        .collect::<SqlResult<Vec<BackupItem>>>()?;

        Ok(items)
    }

    pub fn load_wait_cacl_backup_items(&self, checkpoint_id: &str) -> Result<Vec<BackupItem>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT item_id, item_type, chunk_id, quick_hash, size, 
                    last_modify_time, create_time 
             FROM backup_items 
             WHERE checkpoint_id = ? AND state = ?"
        )?;

        let items = stmt.query_map(
            params![checkpoint_id, BackupItemState::New],
            |row| {
                Ok(BackupItem {
                    item_id: row.get(0)?,
                    item_type: row.get(1)?,
                    chunk_id: row.get(2)?,
                    quick_hash: row.get(3)?,
                    state: row.get(4)?, 
                    size: row.get(5)?,
                    last_modify_time: row.get(6)?,
                    create_time: row.get(7)?,
                })
            }
        )?
        .collect::<SqlResult<Vec<BackupItem>>>()?;

        Ok(items)
    }

    pub fn load_wait_transfer_backup_items(&self, checkpoint_id: &str) -> Result<Vec<BackupItem>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT item_id, item_type, chunk_id, quick_hash, size, 
                    last_modify_time, create_time 
             FROM backup_items 
             WHERE checkpoint_id = ? AND state = ?"
        )?;
        
        let items = stmt.query_map(
            params![
                checkpoint_id,
                BackupItemState::LocalDone,
            ],
            |row| {
                Ok(BackupItem {
                    item_id: row.get(0)?,
                    item_type: row.get(1)?,
                    chunk_id: row.get(2)?,
                    quick_hash: row.get(3)?,
                    state: row.get(4)?,
                    size: row.get(5)?,
                    last_modify_time: row.get(6)?,
                    create_time: row.get(7)?,
                })
            }
        )?
        .collect::<SqlResult<Vec<BackupItem>>>()?;

        Ok(items)
    }

    pub fn update_backup_item(&self, checkpoint_id: &str, item: &BackupItem) -> Result<()> {
        info!("taskdb.update_backup_item: {} {} {:?}", checkpoint_id, item.item_id, item.state);
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE backup_items SET 
                item_type = ?1,
                chunk_id = ?2,
                quick_hash = ?3,
                state = ?4,
                size = ?5,
                last_modify_time = ?6,
                create_time = ?7
            WHERE checkpoint_id = ?8 AND item_id = ?9",
            params![
                item.item_type,
                item.chunk_id,
                item.quick_hash,
                item.state,
                item.size,
                item.last_modify_time,
                item.create_time,
                checkpoint_id,
                item.item_id,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupTaskError::TaskNotFound);
        }

        Ok(())
    }

    pub fn update_backup_item_state(&self, checkpoint_id: &str, item_id: &str, state: BackupItemState) -> Result<()> {
        info!("taskdb.update_backup_item_state: {} {} {:?}", checkpoint_id, item_id, state);
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE backup_items SET state = ?1 
            WHERE checkpoint_id = ?2 AND item_id = ?3",
            params![
                state,
                checkpoint_id,
                item_id,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupTaskError::TaskNotFound);
        }

        Ok(())
    }

    pub fn create_backup_plan(&self, plan: &BackupPlanConfig) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO backup_plans VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                plan.get_plan_key(),
                match &plan.source {
                    BackupSource::Directory(_) => "directory",
                    BackupSource::ChunkList(_) => "chunklist",
                },
                plan.source.get_source_url(),
                match &plan.target {
                    BackupTarget::Directory(_) => "directory",
                    BackupTarget::ChunkList(_) => "chunklist",
                },
                plan.target.get_target_url(),
                plan.title,
                plan.description,
                plan.type_str,
                plan.last_checkpoint_index,
            ],
        )?;
        Ok(())
    }

    pub fn update_backup_plan(&self, plan: &BackupPlanConfig) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let rows_affected = conn.execute(
            "UPDATE backup_plans SET 
                source_type = ?2,
                source_url = ?3,
                target_type = ?4,
                target_url = ?5,
                title = ?6,
                description = ?7,
                type_str = ?8,
                last_checkpoint_index = ?9
            WHERE plan_id = ?1",
            params![
                plan.get_plan_key(),
                match &plan.source {
                    BackupSource::Directory(_) => "directory",
                    BackupSource::ChunkList(_) => "chunklist",
                },
                plan.source.get_source_url(),
                match &plan.target {
                    BackupTarget::Directory(_) => "directory",
                    BackupTarget::ChunkList(_) => "chunklist",
                },
                plan.target.get_target_url(),
                plan.title,
                plan.description,
                plan.type_str,
                plan.last_checkpoint_index,
            ],
        )?;

        if rows_affected == 0 {
            return Err(BackupTaskError::TaskNotFound);
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
            return Err(BackupTaskError::TaskNotFound);
        }
        Ok(())
    }

    pub fn list_backup_plans(&self) -> Result<Vec<BackupPlanConfig>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT * FROM backup_plans")?;
        
        let plans = stmt.query_map([], |row| {
            let source_type: String = row.get(1)?;
            let source_url: String = row.get(2)?;
            let target_type: String = row.get(3)?;
            let target_url: String = row.get(4)?;
            
            Ok(BackupPlanConfig {
                source: match source_type.as_str() {
                    "directory" => BackupSource::Directory(source_url),
                    "chunklist" => BackupSource::ChunkList(source_url),
                    _ => panic!("Invalid source type in database"),
                },
                target: match target_type.as_str() {
                    "directory" => BackupTarget::Directory(target_url),
                    "chunklist" => BackupTarget::ChunkList(target_url),
                    _ => panic!("Invalid target type in database"),
                },
                title: row.get(5)?,
                description: row.get(6)?,
                type_str: row.get(7)?,
                last_checkpoint_index: row.get(8)?,
            })
        })?
        .collect::<SqlResult<Vec<BackupPlanConfig>>>()?;

        Ok(plans)
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
        let tasks = stmt.query_map([], |row| {      
            Ok(row.get(0)?)
        })?
        .collect::<SqlResult<Vec<String>>>()?;
        Ok(tasks)
    }

    pub fn add_worktask_log(&self, timestamp: u64, level: &str, owner_task: &str, log_content: &str, log_event_type: &str) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO worktask_log (timestamp, level, owner_task, log_content, log_event_type) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timestamp, level, owner_task, log_content, log_event_type],
        )?;
        Ok(())
    }

    pub fn get_worktask_logs(&self, owner_task: &str) -> Result<Vec<(u64, String, String, String, String)>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT timestamp, level, owner_task, log_content, log_event_type FROM worktask_log WHERE owner_task = ?"
        )?;
        
        let logs = stmt.query_map(params![owner_task], |row| {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::Path;

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
        db.pause_task(&task_id).unwrap();
        let paused_task = db.load_task_by_id(&task_id).unwrap();
        assert_eq!(paused_task.state, TaskState::Paused);

        // Test resume
        db.resume_task(&task_id).unwrap();
        let resumed_task = db.load_task_by_id(&task_id).unwrap();
        assert_eq!(resumed_task.state, TaskState::Running);

        // Test cancel
        db.cancel_task(&task_id).unwrap();
        let cancelled_task = db.load_task_by_id(&task_id).unwrap();
        assert_eq!(cancelled_task.state, TaskState::Failed);
    }

    #[test]
    fn test_checkpoint_operations() {
        let (db, _) = setup_test_db();
        
        // Create checkpoint
        let checkpoint = BackupCheckPoint::new("test_plan", None, 1);
        let checkpoint_id = checkpoint.checkpoint_id.clone();
        db.create_checkpoint(&checkpoint).unwrap();

        // Load and verify
        let loaded_cp = db.load_checkpoint_by_id(&checkpoint_id).unwrap();
        assert_eq!(loaded_cp.checkpoint_id, checkpoint_id);
        assert_eq!(loaded_cp.owner_plan, "test_plan");

        // Update checkpoint
        let mut updated_cp = checkpoint;
        updated_cp.state = CheckPointState::Prepared;
        db.update_checkpoint(&updated_cp).unwrap();

        // Verify update
        let loaded_cp = db.load_checkpoint_by_id(&checkpoint_id).unwrap();
        assert_eq!(loaded_cp.state, CheckPointState::Prepared);
    }

    #[test]
    fn test_error_handling() {
        let (db, _) = setup_test_db();
        
        // Test loading non-existent task
        let result = db.load_task_by_id("non_existent_task");
        assert!(matches!(result, Err(BackupTaskError::TaskNotFound)));

        // Test loading non-existent checkpoint
        let result = db.load_checkpoint_by_id("non_existent_checkpoint");
        assert!(matches!(result, Err(BackupTaskError::InvalidCheckpointId)));
    }
}


