#![allow(unused)]
use thiserror::Error;
use buckyos_backup_lib::*;
use sha2::Sha256;
use uuid::Uuid;


#[derive(Error, Debug)]
pub enum BackupTaskError {
    #[error("task not found")]
    TaskNotFound,
    #[error("invalid checkpoint id")]
    InvalidCheckpointId,
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


#[derive(Debug, Clone, PartialEq)]
pub enum CheckPointState {
    New,
    Prepared,//所有的backup item确认了
    Evaluated,//所有的backup item都计算了hash和diff(如有需要)
    Done,
    Failed,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
    Running,
    Pending,
    Paused,
    Done,
    Failed,
}


#[derive(Clone,Debug,PartialEq)]
pub enum TaskType {
    Backup,
    Restore,
}

#[derive(Clone,Debug)]
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
}

#[derive(Clone)]
pub struct BackupTaskDb {
    db_path: String,
}

impl BackupTaskDb {
    pub fn new(db_path: &str) -> Self {
        Self {
            db_path: db_path.to_string(),
        }
    }
    
    pub fn load_task_by_id(&self, taskid: &str) -> Result<WorkTask> {
        unimplemented!()
    }


    pub fn load_last_checkpoint(&self, taskid: &str, count:Option<u32>) -> Result<BackupCheckPoint> {
        unimplemented!()
    }

    pub fn load_checkpoint_by_id(&self, checkpoint_id: &str) -> Result<BackupCheckPoint> {
        unimplemented!()
    }

    pub fn create_task(&self, task: &WorkTask) -> Result<()> {
        unimplemented!()
    }
    
    pub fn pause_task(&self, taskid: &str) -> Result<()> {
        unimplemented!()
    }

    pub fn resume_task(&self, taskid: &str) -> Result<()> {
        unimplemented!()
    }

    pub fn cancel_task(&self, taskid: &str) -> Result<()> {
        unimplemented!()
    }

    pub fn update_task(&self, task: &WorkTask) -> Result<()> {
        unimplemented!()
    }

    pub fn save_item_list_to_checkpoint(&self, checkpoint_id: &str, item_list: &Vec<BackupItem>) -> Result<()> {
        unimplemented!()
    }

    pub fn create_checkpoint(&self, checkpoint: &BackupCheckPoint) -> Result<()> {
        unimplemented!()
    }

    pub fn update_checkpoint(&self, checkpoint: &BackupCheckPoint) -> Result<()> {
        unimplemented!()
    }

    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        unimplemented!()
    }

    pub fn load_work_backup_items(&self, checkpoint_id: &str) -> Result<Vec<BackupItem>> {
        unimplemented!()
    }

    pub fn load_wait_transfer_backup_items(&self, checkpoint_id: &str) -> Result<Vec<BackupItem>> {
        unimplemented!()
    }

    pub fn update_backup_item(&self, checkpoint_id: &str, item: &BackupItem) -> Result<()> {
        unimplemented!()
    }

    pub fn update_backup_item_state(&self, checkpoint_id: &str, item_id: &str, state: BackupItemState) -> Result<()> {
        unimplemented!()
    }

    pub fn create_backup_plan(&self, plan: &BackupPlanConfig) -> Result<()> {
        unimplemented!()
    }

    pub fn update_backup_plan(&self, plan: &BackupPlanConfig) -> Result<()> {
        unimplemented!()
    }

    pub fn delete_backup_plan(&self, plan_id: &str) -> Result<()> {
        unimplemented!()
    }

    pub fn list_backup_plans(&self) -> Result<Vec<BackupPlanConfig>> {
        unimplemented!()
    }
}


