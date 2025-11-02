// engine 是backup_suite的核心，负责统一管理配置，备份任务
#![allow(unused)]

use base64;
use buckyos_backup_lib::*;
use buckyos_kit::buckyos_get_unix_timestamp;
use buckyos_kit::get_buckyos_service_data_dir;
use dyn_clone::DynClone;
use futures::stream::futures_unordered::IterMut;
use lazy_static::lazy_static;
use log::*;
use ndn_lib::*;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::future::Future;
use std::io::Cursor;
use std::io::SeekFrom;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use url::Url;

use std::result::Result as StdResult;


use crate::*;
use crate::task_db::*;
use crate::work_task::*;

use buckyos_backup_lib::BackupResult;
use buckyos_backup_lib::BuckyBackupError;

const SMALL_CHUNK_SIZE: u64 = 1024 * 1024; //1MB
const LARGE_CHUNK_SIZE: u64 = 1024 * 1024 * 256; //256MB
const HASH_CHUNK_SIZE: u64 = 1024 * 1024 * 16; //16MB

lazy_static! {
    pub static ref DEFAULT_ENGINE: Arc<Mutex<BackupEngine>> = {
        let engine = BackupEngine::new();
        Arc::new(Mutex::new(engine))
    };
}

pub struct TransferCacheNode {
    pub item_id: String,
    pub chunk_id: String,
    pub total_size: u64,
    pub offset: u64,
    pub is_last_piece: bool,
    pub content: Vec<u8>,
    pub full_id: Option<String>,
}

//理解基本术语
//1. 相同的source url和target url只能创建一个BackupPlan (1个源可以备份到多个目的地)
//2  同一个BackupPlan只能同时运行一个BackupTask或RestoreTask (Running Task)
//3. BackupTask运行成功会创建CheckPoint,CheckPoint可以依赖一个之前存在CheckPoint（支持增量备份）
//4. RestoreTask的创建必须指定CheckPointId

#[derive(Clone)]
pub struct BackupEngine {
    all_plans: Arc<Mutex<HashMap<String, Arc<Mutex<BackupPlanConfig>>>>>,
    all_tasks: Arc<Mutex<HashMap<String, Arc<Mutex<WorkTask>>>>>,
    all_checkpoints: Arc<Mutex<HashMap<String, Arc<Mutex<LocalBackupCheckpoint>>>>>,
    small_file_content_cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    is_strict_mode: bool,
    task_db: BackupTaskDb,
    task_session: Arc<Mutex<HashMap<String, Arc<Mutex<BackupTaskSession>>>>>,
    all_chunk_source_providers: Arc<Mutex<HashMap<String, (BackupSourceProviderDesc, BackupChunkSourceCreateFunc)>>>,
    all_chunk_target_providers: Arc<Mutex<HashMap<String, (BackupTargetProviderDesc, BackupChunkTargetCreateFunc)>>>,
}

impl BackupEngine {
    pub fn new() -> Self {
        let task_db_path = get_buckyos_service_data_dir("backup_suite").join("bucky_backup.db");

        Self {
            all_plans: Arc::new(Mutex::new(HashMap::new())),
            all_tasks: Arc::new(Mutex::new(HashMap::new())),
            all_checkpoints: Arc::new(Mutex::new(HashMap::new())),
            task_db: BackupTaskDb::new(task_db_path.to_str().unwrap()),
            small_file_content_cache: Arc::new(Mutex::new(HashMap::new())),
            is_strict_mode: false,
            task_session: Arc::new(Mutex::new(HashMap::new())),
            all_chunk_source_providers: Arc::new(Mutex::new(HashMap::new())),
            all_chunk_target_providers: Arc::new(Mutex::new(HashMap::new())),

        }
    }

    pub async fn start(&self) -> BackupResult<()> {
        let plans = self.task_db.list_backup_plans().map_err(|e| {
            error!("list backup plans error: {}", e.to_string());
            BuckyBackupError::Failed(e.to_string())
        })?;
        for plan in plans {
            let plan_key = plan.get_plan_key();
            self.all_plans
                .lock()
                .await
                .insert(plan_key.clone(), Arc::new(Mutex::new(plan)));
            info!("load backup plan: {}", plan_key);
        }
        Ok(())
    }

    pub async fn stop(&self) -> BackupResult<()> {
        // stop all running task
        Ok(())
    }

    pub async fn register_backup_chunk_source_provider(
        &self,
        desc: BackupSourceProviderDesc,
        create_func: BackupChunkSourceCreateFunc,
    ) -> BackupResult<()> {
        let mut all_chunk_source_providers = self.all_chunk_source_providers.lock().await;
        if all_chunk_source_providers.contains_key(&desc.type_id) {
            return Err(BuckyBackupError::Failed(format!("chunk source provider already registered")));
        }
        all_chunk_source_providers.insert(desc.type_id.clone(), (desc, create_func));
        Ok(())
    }

    pub async fn register_backup_chunk_target_provider(
        &self,
        desc: BackupTargetProviderDesc,
        create_func: BackupChunkTargetCreateFunc,
    ) -> BackupResult<()> {
        let mut all_chunk_target_providers = self.all_chunk_target_providers.lock().await;
        if all_chunk_target_providers.contains_key(&desc.type_id) {
            return Err(BuckyBackupError::Failed(format!("chunk target provider already registered")));
        }
        all_chunk_target_providers.insert(desc.type_id.clone(), (desc, create_func));
        Ok(())
    }

    pub async fn is_plan_have_running_backup_task(&self, plan_id: &str) -> bool {
        let all_tasks = self.all_tasks.lock().await;
        for (task_id, task) in all_tasks.iter() {
            let real_task = task.lock().await;
            if real_task.owner_plan_id == plan_id && real_task.state == TaskState::Running {
                return true;
            }
        }
        false
    }

    //return planid
    pub async fn create_backup_plan(&self, plan_config: BackupPlanConfig) -> BackupResult<String> {
        let plan_key = plan_config.get_plan_key();
        let mut all_plans = self.all_plans.lock().await;
        if all_plans.contains_key(&plan_key) {
            return Err(BuckyBackupError::Failed(format!("plan already exists")));
        }

        self.task_db.create_backup_plan(&plan_config)?;
        info!("create backup plan: [{}] {:?}", plan_key, plan_config);
        all_plans.insert(plan_key.clone(), Arc::new(Mutex::new(plan_config)));
        Ok(plan_key)
    }

    pub async fn get_backup_plan(&self, plan_id: &str) -> BackupResult<BackupPlanConfig> {
        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(plan_id);
        if plan.is_none() {
            return Err(BuckyBackupError::NotFound(format!("plan {} not found", plan_id)));
        }
        let plan = plan.unwrap().lock().await;
        Ok(plan.clone())
    }

    pub async fn delete_backup_plan(&self, plan_id: &str) -> BackupResult<()> {
        unimplemented!()
    }

    pub async fn list_backup_plans(&self) -> BackupResult<Vec<String>> {
        let all_plans = self.all_plans.lock().await;
        Ok(all_plans.keys().map(|k| k.clone()).collect())
    }

    //create a backup task will create a new checkpoint
    pub async fn create_backup_task(
        &self,
        plan_id: &str,
        parent_checkpoint_id: Option<&str>,
    ) -> BackupResult<String> {
        if self.is_plan_have_running_backup_task(plan_id).await {
            return Err(BuckyBackupError::Failed(format!(
                "plan {} already has a running backup task",
                plan_id
            )));
        }

        let mut all_plans = self.all_plans.lock().await;
        let mut plan = all_plans.get_mut(plan_id);
        if plan.is_none() {
            return Err(BuckyBackupError::NotFound(format!(
                "plan {} not found",
                plan_id
            )));
        }
        let mut plan = plan.unwrap().lock().await;
        if parent_checkpoint_id.is_some() {
            //如果parent_checkpoint_id存在，则需要验证是否存在
            warn!("parent_checkpoint_id is not supported yet");
            unimplemented!()
        }
        plan.last_checkpoint_index += 1;
        let last_checkpoint_index = plan.last_checkpoint_index;
        self.task_db.update_backup_plan(&plan)?;
        let checkpoint_type = plan.get_checkpiont_type();
        drop(plan);
        drop(all_plans);

        let new_checkpoint_id = uuid::Uuid::new_v4().to_string();
        let new_checkpoint = BackupCheckpoint::new(checkpoint_type, "test_checkpoint".to_string(), parent_checkpoint_id.map(|id| id.to_string()), None);
        let local_checkpoint = LocalBackupCheckpoint::new(new_checkpoint, new_checkpoint_id.clone(), plan_id.to_string());
        let mut all_checkpoints = self.all_checkpoints.lock().await;
        self.task_db.create_checkpoint(&local_checkpoint)?;
        all_checkpoints.insert(
            new_checkpoint_id.clone(),
            Arc::new(Mutex::new(local_checkpoint)),
        );
        drop(all_checkpoints);

        info!(
            "create new checkpoint: {} @ plan: {}",
            new_checkpoint_id, plan_id
        );

        let new_task = WorkTask::new(plan_id, new_checkpoint_id.as_str(), TaskType::Backup);
        let new_task_id = new_task.taskid.clone();
        self.task_db.create_task(&new_task)?;
        info!("create new backup task: {:?}", new_task);
        let mut all_tasks = self.all_tasks.lock().await;
        all_tasks.insert(new_task_id.clone(), Arc::new(Mutex::new(new_task)));
        return Ok(new_task_id);
    }

    // async fn run_chunk2dir_backup_task(&self,backup_task: WorkTask,
    //     source:BackupChunkSourceProvider, target:BackupDirTargetProvider) -> Result<()> {
    //     unimplemented!()
    // }

    // async fn run_dir2chunk_backup_task(&self,backup_task: WorkTask,
    //     source:BackupDirSourceProvider, target: impl ChunkTarget) -> Result<()> {
    //     unimplemented!()
    // }

    // async fn run_dir2dir_backup_task(&self,backup_task: WorkTask,
    //     source:BackupDirSourceProvider, target:BackupDirTargetProvider) -> Result<()> {
    //     unimplemented!()
    // }


    async fn complete_backup_checkpoint(
        &self,
        checkpoint_id: &str,
        state: CheckPointState,
        owner_task: Arc<Mutex<WorkTask>>,
    ) -> BackupResult<()> {
        unimplemented!()
    }

    async fn complete_backup_item(
        &self,
        checkpoint_id: &str,
        item: &BackupChunkItem,
        owner_task: Arc<Mutex<WorkTask>>,
    ) -> BackupResult<()> {
        self.task_db.update_backup_item_state(
            checkpoint_id,
            &item.item_id,
            BackupItemState::Done,
        )?;

        // let mut real_done_items = done_items.lock().await;
        // real_done_items.insert(item.item_id.clone(), item.size);
        // drop(real_done_items);

        let mut real_task = owner_task.lock().await;
        real_task.completed_item_count += 1;
        real_task.completed_size += item.size;
        self.task_db.update_task(&real_task).map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        drop(real_task);
        Ok(())
    }

    async fn run_chunk2chunk_backup_task(
        &self,
        backup_task: Arc<Mutex<WorkTask>>,
        checkpoint_id: String,
        source: BackupChunkSourceProvider,
        target: BackupChunkTargetProvider,
    ) -> BackupResult<()> {
        unimplemented!()
    }

    pub async fn on_backup_source_prepare_callback() {
        unimplemented!()
    }

    pub async fn backup_chunk_source_prepare_thread(
        engine: BackupEngine,
        source: BackupChunkSourceProvider,
        backup_task: Arc<Mutex<WorkTask>>,
        task_session: Arc<Mutex<BackupTaskSession>>,
        checkpoint: Arc<Mutex<LocalBackupCheckpoint>>,
    ) -> BackupResult<()> {
        let real_checkpoint = checkpoint.lock().await;
        let checkpoint_id = real_checkpoint.checkpoint_id.clone();
        drop(real_checkpoint);

        loop {
            //TODO:在prepare参数里传入 task的cache_queue,方便在prepare的时候就可以服用io
            let (mut this_item_list, is_done) = source.prepare_items(checkpoint_id.as_str(),None).await?;
            //add items to checkpoint
 
            if is_done {
                let mut real_checkpoint = checkpoint.lock().await;
                real_checkpoint.state = CheckPointState::Prepared;
                engine.task_db.update_checkpoint_state(checkpoint_id.as_str(), CheckPointState::Prepared)?;
                drop(real_checkpoint);    
                break;
            }
        }
        warn!("checkpoint {} 's prepare thread exit.", checkpoint_id);
        return Ok(());
    }
    async fn update_backup_item_state_by_remote_checkpoint_state(
        &self,
        checkpoint_items_state: &RemoteBackupCheckPointItemStatus,
    ) -> BackupResult<()> {
        unimplemented!()
    }

    async fn pop_wait_backup_item(
        &self,
        checkpoint_id: &str,
    ) -> BackupResult<Option<BackupChunkItem>> {
        unimplemented!()
    }

    pub async fn backup_work_thread(
        engine: BackupEngine,
        source: BackupChunkSourceProvider,
        target: BackupChunkTargetProvider,
        backup_task: Arc<Mutex<WorkTask>>,
        task_session: Arc<Mutex<BackupTaskSession>>,
        //checkpoint: Arc<Mutex<BackupCheckPoint>>,
    ) -> BackupResult<()> {
    
        let real_task = backup_task.lock().await;
        let checkpoint_id = real_task.checkpoint_id.clone();
        let task_id = real_task.taskid.clone();
        drop(real_task);

        info!("task {} transfer thread start", task_id);
        loop {
            let real_task = backup_task.lock().await;
            if real_task.state != TaskState::Running {
                info!("backup task {} is not running, exit transfer thread", real_task.taskid);
                break;
            }
            drop(real_task);

            let (remote_checkpoint, checkpoint_items_state) = target.query_check_point_state(checkpoint_id.as_str()).await?;
            //self.update_task_state_by_remote_checkpoint_state(&remote_checkpoint_state);
            engine.update_backup_item_state_by_remote_checkpoint_state(&checkpoint_items_state).await?;
            match remote_checkpoint.state {
                CheckPointState::New => {
                    warn!("checkpoint {} remote state is new, need allocate checkpoint at remote", checkpoint_id);
                    let checkpoint = engine.task_db.load_checkpoint_by_id(checkpoint_id.as_str())?;
                    let alloc_result = target.alloc_checkpoint(&checkpoint).await;
                    if alloc_result.is_err() {
                        let err_string = alloc_result.err().unwrap().to_string();
                        warn!("allocate checkpoint {} at backup target error: {}", checkpoint_id, err_string.as_str());
                        engine.complete_backup_checkpoint(checkpoint_id.as_str(), CheckPointState::Failed(err_string),backup_task.clone()).await?;
                        continue;
                    }
                    
                    warn!("checkpoint {} allocated at backup target.", checkpoint_id);
                    continue;
                }
                CheckPointState::Prepared => {
                    error!("checkpoint {} remote state is prepared,but remote checkpint state NEVER be prepared. something wrong,exit working thread", checkpoint_id);
                    break;
                }
                CheckPointState::Done => {
                    warn!("checkpoint {} remote state is done, exit working thread", checkpoint_id);
                    engine.complete_backup_checkpoint(checkpoint_id.as_str(), CheckPointState::Done,backup_task.clone()).await?;
                    break;
                }
                CheckPointState::Failed(msg) => {
                    warn!("checkpoint {} remote state is failed: {}, exit working thread", checkpoint_id, msg.as_str());
                    engine.complete_backup_checkpoint(checkpoint_id.as_str(), CheckPointState::Failed(msg),backup_task.clone()).await?;
                    break;
                }
                CheckPointState::WaitTrans => {
                    //try put item list to target
                    warn!("checkpoint {} remote state is wait trans, wait remote 5 seconds", checkpoint_id);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                CheckPointState::Working => {
                    let item = engine.pop_wait_backup_item(checkpoint_id.as_str()).await?;
                   
                    if item.is_some() {
                        let item = item.unwrap();
                        let mut is_item_done = false;
                        let mut writer = target.open_chunk_writer(checkpoint_id.as_str(), &item.chunk_id).await;
                        match writer {
                            Ok((mut writer, init_offset)) => {
                                let mut reader = source.open_item_chunk_reader(checkpoint_id.as_str(), &item,init_offset).await?;
                                let trans_bytes = tokio::io::copy(&mut reader, &mut writer).await
                                    .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
                                target.complete_chunk_writer(checkpoint_id.as_str(), &item.chunk_id).await?;
                                is_item_done = true;
                            }
                            Err(e) => {
                                match e {
                                    BuckyBackupError::TryLater(msg) => {
                                        warn!("open chunk writer error: {}, try later", msg);
                                        continue;
                                    }
                                    BuckyBackupError::AlreadyDone(msg) => {
                                        warn!("chunk {} already exist, skip upload", item.chunk_id.to_string());
                                        is_item_done = true;
                                    }
                                    _ => {
                                        warn!("open chunk writer error: {}", e.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                        if is_item_done {
                            engine.complete_backup_item(checkpoint_id.as_str(), &item, backup_task.clone()).await?;
                        }
                    } else {
                        //no item to backup, check point completed
                        if checkpoint_items_state == RemoteBackupCheckPointItemStatus::NotSupport {
                            warn!("checkpoint {} remote state is not support checkpoint level check, complete backup checkpoint by all local items done.", checkpoint_id);
                            engine.complete_backup_checkpoint(checkpoint_id.as_str(), CheckPointState::Done,backup_task.clone()).await?;
                            break;
                        } 
                    }
                }
            }
        }

        Ok(())
    }

    //return taskid
    pub async fn create_restore_task(
        &self,
        plan_id: &str,
        check_point_id: &str,
        restore_config: RestoreConfig,
    ) -> BackupResult<String> {
        if self.is_plan_have_running_backup_task(plan_id).await {
            return Err(BuckyBackupError::Failed(format!(
                "plan {} already has a running backup task",
                plan_id
            )));
        }

        let checkpoint = self.task_db.load_checkpoint_by_id(check_point_id)?;
        let mut new_task = WorkTask::new(plan_id, check_point_id, TaskType::Restore);
        new_task.set_restore_config(restore_config);
        let new_task_id = new_task.taskid.clone();
        self.task_db.create_task(&new_task)?;
        info!("create new restore task: {:?}", new_task);
        let mut all_tasks = self.all_tasks.lock().await;
        all_tasks.insert(new_task_id.clone(), Arc::new(Mutex::new(new_task)));
        Ok(new_task_id)
    }

    fn check_all_check_point_exist(&self, checkpoint_id: &str) -> BackupResult<bool> {
        let checkpoint = self.task_db.load_checkpoint_by_id(checkpoint_id)?;
        if checkpoint.state != CheckPointState::Done {
            info!("checkpoint {} is not done! cannot restore", checkpoint_id);
            return Ok(false);
        }

        if checkpoint.prev_checkpoint_id.is_none() {
            return Ok(true);
        }
        debug!(
            "checkpoint {} depend checkpoint: {}",
            checkpoint_id,
            checkpoint.prev_checkpoint_id.as_ref().unwrap()
        );
        let parent_checkpoint_id = checkpoint.prev_checkpoint_id.as_ref().unwrap();
        let result = self.check_all_check_point_exist(parent_checkpoint_id)?;
        Ok(result)
    }

    async fn run_chunk2chunk_restore_task(
        &self,
        restore_task: Arc<Mutex<WorkTask>>,
        checkpoint_id: String,
        source: BackupChunkSourceProvider,
        target: BackupChunkTargetProvider,
    ) -> BackupResult<()> {
        unimplemented!()
    }

    async fn run_dir2chunk_restore_task(&self, plan_id: &str, check_point_id: &str) -> BackupResult<()> {
        unimplemented!()
    }

    async fn run_dir2dir_restore_task(&self, plan_id: &str, check_point_id: &str) -> BackupResult<()> {
        unimplemented!()
    }

    async fn get_chunk_source_provider(
        &self,
        source_url: &str,
    ) -> BackupResult<BackupChunkSourceProvider> {
        let url = Url::parse(source_url).map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        let mut all_chunk_source_providers = self.all_chunk_source_providers.lock().await;
        //assert_eq!(url.scheme(), "file");
        let create_func = all_chunk_source_providers.get(url.scheme());
        if create_func.is_none() {
            return Err(BuckyBackupError::NotFound(format!("create chunk backup source failed, unsupported source url scheme: {}", url.scheme())));
        }
        let (desc, create_func) = create_func.unwrap();
        let mut local_path = url.path();
        //let result = create_func(local_path.to_string()).await?;
        //Ok(result)
        unimplemented!()
    }

    async fn get_chunk_target_provider(
        &self,
        target_url: &str,
    ) -> BackupResult<BackupChunkTargetProvider> {
        let url = Url::parse(target_url).map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        let mut all_chunk_target_providers = self.all_chunk_target_providers.lock().await;
        let create_func = all_chunk_target_providers.get(url.scheme());
        if create_func.is_none() {
            return Err(BuckyBackupError::NotFound(format!("create chunk backup target failed, unsupported target url scheme: {}", url.scheme())));
        }
        let (desc, create_func) = create_func.unwrap();
        //let result = create_func(target_url).await?;
        //Ok(result)
        unimplemented!()
        // match url.scheme() {
        //     "file" => {
        //         let mut local_path = url.path();
        //         #[cfg(windows)]
        //         {
        //             local_path = local_path.trim_start_matches('/');
        //         }
        //         let store = LocalChunkTargetProvider::new(local_path.to_string(), "default".to_string())
        //             .await
        //             .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        //         Ok(Box::new(store))
        //     }
        //     "s3" => {
        //         // 从 URL 中提取 S3 配置参数
        //         let store = S3ChunkTarget::with_url(url)
        //             .await
        //             .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        //         Ok(Box::new(store))
        //     }
        //     _ => Err(BuckyBackupError::Failed(format!(
        //         "不支持的 target URL scheme: {}",
        //         url.scheme()
        //     ))),
        // }
    }

    pub async fn list_backup_tasks(&self, filter: &str) -> BackupResult<Vec<String>> {
        self.task_db.list_worktasks(filter).map_err(|e| {
            let err_str = e.to_string();
            warn!("list work tasks error: {}", err_str.as_str());
            BuckyBackupError::Failed(format!("list work tasks error: {}", err_str))
        })
    }

    pub async fn get_task_info(&self, taskid: &str) -> BackupResult<WorkTask> {
        let mut all_tasks = self.all_tasks.lock().await;
        let mut backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            let _backup_task = self.task_db.load_task_by_id(taskid)?;
            all_tasks.insert(taskid.to_string(), Arc::new(Mutex::new(_backup_task)));
            backup_task = all_tasks.get(taskid);
        }

        if backup_task.is_none() {
            return Err(BuckyBackupError::NotFound("task not found".to_string()));
        }
        let backup_task = backup_task.unwrap().lock().await.clone();
        Ok(backup_task)
    }

    pub async fn resume_restore_task(&self, taskid: &str) -> BackupResult<()> {
        let mut all_tasks = self.all_tasks.lock().await;
        let mut restore_task = all_tasks.get(taskid);
        if restore_task.is_none() {
            error!("restore task not found: {}", taskid);
            return Err(BuckyBackupError::NotFound("task not found".to_string()));
        }
        let restore_task = restore_task.unwrap().clone();
        drop(all_tasks);

        let mut real_restore_task = restore_task.lock().await;
        if real_restore_task.task_type != TaskType::Restore {
            error!("try resume a BackupTask as Restore.");
            return Err(BuckyBackupError::Failed("try resume a BackupTask as Restore".to_string()));
        }
        if real_restore_task.state != TaskState::Paused {
            warn!("restore task is not paused, ignore resume");
            return Err(BuckyBackupError::Failed("restore task is not paused".to_string()));
        }
        real_restore_task.state = TaskState::Running;
        let task_id = real_restore_task.taskid.clone();
        let checkpoint_id = real_restore_task.checkpoint_id.clone();
        let owner_plan_id = real_restore_task.owner_plan_id.clone();

        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(&owner_plan_id);
        if plan.is_none() {
            error!(
                "task plan not found: {} plan_id: {}",
                taskid,
                owner_plan_id.as_str()
            );
            return Err(BuckyBackupError::NotFound("task plan not found".to_string()));
        }
        let plan = plan.unwrap().lock().await;
        let task_type = plan.type_str.clone();
        let source_provider = self
            .get_chunk_source_provider(plan.source.get_source_url())
            .await?;
        let target_provider = self
            .get_chunk_target_provider(plan.target.get_target_url())
            .await?;

        drop(plan);
        drop(all_plans);

        info!(
            "resume restore task: {} type: {}",
            taskid,
            task_type.as_str()
        );
        let taskid = task_id.clone();
        let engine: BackupEngine = self.clone();
        let restore_task = restore_task.clone();
        tokio::spawn(async move {
            let task_result = match task_type.as_str() {
                "c2c" => {
                    engine
                        .run_chunk2chunk_restore_task(
                            restore_task.clone(),
                            checkpoint_id,
                            source_provider,
                            target_provider,
                        )
                        .await
                }
                //"d2c" => engine.run_dir2chunk_backup_task(backup_task, source_provider, target_provider).await,
                //"d2d" => engine.run_dir2dir_backup_task(backup_task, source_provider, target_provider).await,
                //"c2d" => engine.run_chunk2dir_backup_task(backup_task, source_provider, target_provider).await,
                _ => Err(BuckyBackupError::Failed(format!("unknown plan type: {}", task_type))),
            };

            let mut real_restore_task = restore_task.lock().await;
            if task_result.is_err() {
                info!(
                    "restore task failed: {} {}",
                    taskid.as_str(),
                    task_result.err().unwrap()
                );
                real_restore_task.state = TaskState::Failed;
            } else {
                info!("restore task done: {} ", taskid.as_str());
                real_restore_task.state = TaskState::Done;
            }
            engine.task_db.update_task(&real_restore_task);
        });

        Ok(())
    }

    pub async fn resume_backup_task(&self, taskid: &str) -> BackupResult<()> {
        // load task from db
        let mut all_tasks = self.all_tasks.lock().await;
        let mut backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            info!("task not found: {} at memory,try load from db", taskid);
            let _backup_task = self.task_db.load_task_by_id(taskid)?;
            all_tasks.insert(taskid.to_string(), Arc::new(Mutex::new(_backup_task)));
            backup_task = all_tasks.get(taskid);
        }
        let backup_task = backup_task.unwrap().clone();
        drop(all_tasks);

        let mut real_backup_task = backup_task.lock().await;
        if real_backup_task.task_type != TaskType::Backup {
            error!("try resume a RestoreTask as Backup.");
            return Err(BuckyBackupError::Failed("try resume a RestoreTask as Backup".to_string()));
        }
        if real_backup_task.state != TaskState::Paused {
            warn!("task is not paused, ignore resume");
            return Err(BuckyBackupError::Failed("task is not paused".to_string()));
        }
        real_backup_task.state = TaskState::Running;
        let task_id = real_backup_task.taskid.clone();
        let checkpoint_id = real_backup_task.checkpoint_id.clone();
        let owner_plan_id = real_backup_task.owner_plan_id.clone();

        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(&owner_plan_id);
        if plan.is_none() {
            error!(
                "task plan not found: {} plan_id: {}",
                taskid,
                owner_plan_id.as_str()
            );
            return Err(BuckyBackupError::NotFound("task plan not found".to_string()));
        }
        let plan = plan.unwrap().lock().await;
        let task_type = plan.type_str.clone();
        let source_provider = self
            .get_chunk_source_provider(plan.source.get_source_url())
            .await?;
        let target_provider = self
            .get_chunk_target_provider(plan.target.get_target_url())
            .await?;

        drop(plan);
        drop(all_plans);

        info!(
            "resume backup task: {} type: {}",
            taskid,
            task_type.as_str()
        );
        let taskid = task_id.clone();
        let engine: BackupEngine = self.clone();
        let backup_task = backup_task.clone();
        tokio::spawn(async move {
            let task_result = match task_type.as_str() {
                "c2c" => {
                    engine
                        .run_chunk2chunk_backup_task(
                            backup_task.clone(),
                            checkpoint_id,
                            source_provider,
                            target_provider,
                        )
                        .await
                }
                //"d2c" => engine.run_dir2chunk_backup_task(backup_task, source_provider, target_provider).await,
                //"d2d" => engine.run_dir2dir_backup_task(backup_task, source_provider, target_provider).await,
                //"c2d" => engine.run_chunk2dir_backup_task(backup_task, source_provider, target_provider).await,
                _ => Err(BuckyBackupError::Failed(format!("unknown plan type: {}", task_type))),
            };

            //let all_tasks = engine.all_tasks.lock().await;
            // let mut backup_task = all_tasks.get_mut(taskid);
            let mut real_backup_task = backup_task.lock().await;
            if task_result.is_err() {
                info!(
                    "backup task failed: {} {}",
                    taskid.as_str(),
                    task_result.err().unwrap()
                );
                real_backup_task.state = TaskState::Failed;
            } else {
                info!("backup task done: {} ", taskid.as_str());
                real_backup_task.state = TaskState::Done;
            }
            engine.task_db.update_task(&real_backup_task);
        });

        Ok(())
    }

    pub async fn resume_work_task(&self, taskid: &str) -> BackupResult<()> {
        let mut all_tasks = self.all_tasks.lock().await;
        let mut backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            info!("task not found: {} at memory,try load from db", taskid);
            let _backup_task = self.task_db.load_task_by_id(taskid)?;
            all_tasks.insert(taskid.to_string(), Arc::new(Mutex::new(_backup_task)));
            backup_task = all_tasks.get(taskid);
        }
        let backup_task = backup_task.unwrap().clone();
        drop(all_tasks);

        let mut real_backup_task = backup_task.lock().await;
        let task_type = real_backup_task.task_type.clone();
        drop(real_backup_task);

        match task_type {
            TaskType::Backup => self.resume_backup_task(taskid).await,
            TaskType::Restore => self.resume_restore_task(taskid).await,
        }
    }

    pub async fn pause_work_task(&self, taskid: &str) -> BackupResult<()> {
        let all_tasks = self.all_tasks.lock().await;
        let backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            error!("task not found: {}", taskid);
            return Err(BuckyBackupError::NotFound("task not found".to_string()));
        }
        let mut backup_task = backup_task.unwrap().lock().await;
        if backup_task.state != TaskState::Running {
            warn!("task is not running, ignore pause");
            return Err(BuckyBackupError::Failed("task is not running".to_string()));
        }
        backup_task.state = TaskState::Paused;
        self.task_db.update_task(&backup_task)?;
        Ok(())
    }

    pub async fn cancel_backup_task(&self, taskid: &str) -> BackupResult<()> {
        unimplemented!()
    }
}

//impl kRPC for BackupEngine

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_c2c_backup_task() {
        //std::env::set_var("BUCKY_LOG", "debug");
        buckyos_kit::init_logging("bucky_backup_test",false);
        let tempdb = "/opt/buckyos/data/backup_suite/bucky_backup.db";
        //delete db file if exists
        if std::path::Path::new(tempdb).exists() {
            std::fs::remove_file(tempdb).unwrap();
        }

        let engine = BackupEngine::new();
        engine.start().await.unwrap();
        let new_plan = BackupPlanConfig::chunk2chunk(
            "file:///Users/liuzhicong/Downloads",
            "file:///tmp/bucky_backup_result",
            "testc2c",
            "testc2c desc",
        );
        let plan_id = engine.create_backup_plan(new_plan).await.unwrap();
        info!("create backup plan: {}", plan_id);
        let task_id = engine.create_backup_task(&plan_id, None).await.unwrap();
        info!("create backup task: {}", task_id);
        engine.resume_work_task(&task_id).await.unwrap();
        let task_info = engine.get_task_info(&task_id).await.unwrap();
        let check_point_id = task_info.checkpoint_id.clone();
        let mut step = 0;
        loop {
            step += 1;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let task_info = engine.get_task_info(&task_id).await.unwrap();
            if task_info.state == TaskState::Done {
                println!("backup task done");
                break;
            }
            if step > 600 {
                panic!("task run too long");
            }
        }
    }

    #[tokio::test]
    async fn test_run_c2c_restore_task() {
        std::env::set_var("BUCKY_LOG", "debug");
        buckyos_kit::init_logging("bucky_backup_test",false);

        let engine = BackupEngine::new();
        engine.start().await.unwrap();

        let checkpoint_id = "chk_f4c56225-8f3f-4641-a569-5388a369cb3d".to_string();
        let plan_id = "c2c-file:///tmp/test-file:///tmp/bucky_backup_result".to_string();
        info!("checkpoint_id: {}", checkpoint_id);
        info!("plan_id: {}", plan_id);
        let restore_config = RestoreConfig {
            restore_location_url: "file:///tmp/restore_result".to_string(),
            is_clean_restore: true,
            params: None,
        };

        let task_id = engine
            .create_restore_task(&plan_id, &checkpoint_id, restore_config)
            .await
            .unwrap();
        info!("restore task_id: {}", task_id);
        engine.resume_restore_task(&task_id).await.unwrap();
        let mut step = 0;
        loop {
            step += 1;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let task_info = engine.get_task_info(&task_id).await.unwrap();
            if task_info.state == TaskState::Done {
                println!("restore task done");
                break;
            }
            if step > 600 {
                panic!("task run too long");
            }
        }
    }
}
