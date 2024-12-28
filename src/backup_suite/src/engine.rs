// engine 是backup_suite的核心，负责统一管理配置，备份任务
#![allow(unused)]
use std::future::Future;
use std::io::SeekFrom;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Ok;
use buckyos_kit::buckyos_get_unix_timestamp;
use buckyos_kit::get_buckyos_service_data_dir;
use futures::stream::futures_unordered::IterMut;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use std::io::Cursor;
use tokio::io::AsyncRead;
use anyhow::Result;
use base64;
use sha2::{Sha256, Digest};
use log::*;
use serde::{Serialize, Deserialize};
use url::Url;
use dyn_clone::DynClone;
use ndn_lib::*;
use buckyos_backup_lib::*;
use tokio::time::{timeout, Duration};
use lazy_static::lazy_static;

use std::result::Result as StdResult;

use crate::task_db::*;
use crate::work_task::*;

const SMALL_CHUNK_SIZE:u64 = 1024*1024;//1MB
const LARGE_CHUNK_SIZE:u64 = 1024*1024*256; //256MB 
const HASH_CHUNK_SIZE:u64 = 1024*1024*16; //16MB

lazy_static!{
    pub static ref DEFAULT_ENGINE : Arc<Mutex<BackupEngine>> = {
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
    all_checkpoints: Arc<Mutex<HashMap<String, Arc<Mutex<BackupCheckPoint>>>>>,
    small_file_content_cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    is_strict_mode: bool,
    task_db: BackupTaskDb,
    task_session: Arc<Mutex<HashMap<String,Arc<Mutex<BackupTaskSession>>>>>,
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
        }
    }

    pub async fn start(&self) -> Result<()> {
        let plans = self.task_db.list_backup_plans()?;
        for plan in plans { 
            let plan_key = plan.get_plan_key();
            self.all_plans.lock().await.insert(plan_key.clone(), Arc::new(Mutex::new(plan)));
            info!("load backup plan: {}", plan_key);
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        // stop all running task
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
    pub async fn create_backup_plan(&self, plan_config: BackupPlanConfig) -> Result<String> {
        let plan_key = plan_config.get_plan_key();
        let mut all_plans = self.all_plans.lock().await;
        if all_plans.contains_key(&plan_key) {
            return Err(anyhow::anyhow!("plan already exists"));
        }

        self.task_db.create_backup_plan(&plan_config)?;
        info!("create backup plan: [{}] {:?}", plan_key, plan_config);
        all_plans.insert(plan_key.clone(), Arc::new(Mutex::new(plan_config)));
        Ok(plan_key)
    }

    pub async fn get_backup_plan(&self, plan_id: &str) -> Result<BackupPlanConfig> {
        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(plan_id);
        if plan.is_none() {
            return Err(anyhow::anyhow!("plan {} not found", plan_id));
        }
        let plan = plan.unwrap().lock().await;
        Ok(plan.clone())
    }

    pub async fn delete_backup_plan(&self, plan_id: &str) -> Result<()> {
        unimplemented!()
    }

    pub async fn list_backup_plans(&self) -> Result<Vec<String>> {
        let all_plans = self.all_plans.lock().await;
        Ok(all_plans.keys().map(|k| k.clone()).collect())
    }

    //create a backup task will create a new checkpoint
    pub async fn create_backup_task(&self, plan_id: &str,parent_checkpoint_id: Option<&str>) -> Result<String> {
        if self.is_plan_have_running_backup_task(plan_id).await {
            return Err(anyhow::anyhow!("plan {} already has a running backup task", plan_id));
        }

        let mut all_plans = self.all_plans.lock().await;
        let mut plan = all_plans.get_mut(plan_id);
        if plan.is_none() {
            return Err(anyhow::anyhow!("plan {} not found", plan_id));
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
        drop(plan);
        drop(all_plans);

        let new_checkpoint = BackupCheckPoint::new(plan_id, 
            parent_checkpoint_id, last_checkpoint_index);
        let new_checkpoint_id = new_checkpoint.checkpoint_id.clone();
        let mut all_checkpoints = self.all_checkpoints.lock().await;
        self.task_db.create_checkpoint(&new_checkpoint)?;
        all_checkpoints.insert(new_checkpoint.checkpoint_id.clone(), Arc::new(Mutex::new(new_checkpoint)));
        drop(all_checkpoints);

        info!("create new checkpoint: {} @ plan: {}", new_checkpoint_id, plan_id);

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

    async fn complete_backup_item(&self,checkpoint_id: &str,item: &BackupItem,owner_task:Arc<Mutex<WorkTask>>,done_items:Arc<Mutex<HashMap<String,u64>>>) -> Result<()> {
        self.task_db.update_backup_item_state(checkpoint_id, &item.item_id, BackupItemState::Done)?;
      
        let mut real_done_items = done_items.lock().await;
        real_done_items.insert(item.item_id.clone(), item.size);
        drop(real_done_items);

        let mut real_task = owner_task.lock().await;
        real_task.completed_item_count += 1;
        real_task.completed_size += item.size;
        self.task_db.update_task(&real_task)?;
        drop(real_task);
        Ok(())
    }

    async fn run_chunk2chunk_backup_task(&self,backup_task:Arc<Mutex<WorkTask>>,checkpoint_id: String,
        source:BackupChunkSourceProvider, target:BackupChunkTargetProvider) -> Result<()> {
        let source2 = self.get_chunk_source_provider(source.get_source_url().as_str()).await?;
        let source3 = self.get_chunk_source_provider(source.get_source_url().as_str()).await?;
        let target2 = self.get_chunk_target_provider(target.get_target_url().as_str()).await?;
        let backup_task_eval = backup_task.clone();
        let backup_task_trans = backup_task.clone();
        
        let is_strict_mode = self.is_strict_mode;
    
        let mut all_checkpoints = self.all_checkpoints.lock().await;
        let mut checkpoint = all_checkpoints.get(checkpoint_id.as_str());
        if checkpoint.is_none() {
            let real_checkpoint = self.task_db.load_checkpoint_by_id(checkpoint_id.as_str())?;
            all_checkpoints.insert(checkpoint_id.clone(), Arc::new(Mutex::new(real_checkpoint)));
            checkpoint = all_checkpoints.get(checkpoint_id.as_str());
        }
        let checkpoint = checkpoint.unwrap().clone();
        drop(all_checkpoints);

        let checkpoint2 = checkpoint.clone();
        let checkpoint3 = checkpoint.clone();
        let checkpoint4 = checkpoint.clone();

        let real_backup_task = backup_task.lock().await;
        let task_id = real_backup_task.taskid.clone();
        let task_id2 = task_id.clone();
        let task_session = Arc::new(Mutex::new(BackupTaskSession::new(task_id)));
        drop(real_backup_task);
        let task_session_eval = task_session.clone();
        let task_session_trans = task_session.clone();

        let engine_prepare = self.clone();
        let source_prepare_thread = tokio::spawn(async move {
            let prepare_result = BackupEngine::backup_chunk_source_prepare_thread(engine_prepare,source,
                backup_task.clone(),task_session.clone(),checkpoint.clone()).await;
            if prepare_result.is_err() {
                error!("prepare thread error: {}", prepare_result.err().unwrap());
            }
        });
        let engine_eval = self.clone();

        let eval_thread = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            let eval_result =BackupEngine::backup_chunk_source_eval_thread(engine_eval,source2,target,
                backup_task_eval,task_session_eval,checkpoint2).await;
            if eval_result.is_err() {
                error!("eval thread error: {}", eval_result.err().unwrap());
            }
        });

        let engine_transfer = self.clone();
        let transfer_thread = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
            let transfer_result = BackupEngine::backup_work_thread(engine_transfer,source3,target2,
                backup_task_trans,task_session_trans,checkpoint3).await;
            if transfer_result.is_err() {
                error!("transfer thread error: {}", transfer_result.err().unwrap());
            }
        });

        tokio::join!(source_prepare_thread, eval_thread, transfer_thread);
        let is_all_done = self.task_db.check_is_checkpoint_items_all_done(&checkpoint_id)?;
        if is_all_done {
            info!("checkpoint {} is all done, set to DONE", checkpoint_id);
            let mut real_checkpoint = checkpoint4.lock().await;
            real_checkpoint.state = CheckPointState::Done;
            self.task_db.update_checkpoint(&real_checkpoint)?;
        }
        info!("backup task {} is done, main thread exit", task_id2);
        
        Ok(())
    }

    pub async fn backup_chunk_source_prepare_thread(engine:BackupEngine,source:BackupChunkSourceProvider,
        backup_task:Arc<Mutex<WorkTask>>,task_session:Arc<Mutex<BackupTaskSession>>,checkpoint:Arc<Mutex<BackupCheckPoint>>) -> Result<()> {
        let real_checkpoint = checkpoint.lock().await;
        let have_depend_checkpoint = real_checkpoint.depend_checkpoint_id.is_some();
        let checkpoint_id = real_checkpoint.checkpoint_id.clone();
        drop(real_checkpoint);

        let real_task_session = task_session.lock().await;
        let eval_queue_sender = real_task_session.eval_queue.clone();
        let eval_cache_queue_sender = real_task_session.eval_cache_queue.clone();
        let transfer_cache_queue = real_task_session.transfer_cache_queue.clone();
        let transfer_queue = real_task_session.transfer_queue.clone();
        //let transfer_queue_sender = real_task_session.transfer_queue.clone_sender();
        drop(real_task_session);

        loop {
            //TODO:在prepare参数里传入 task的cache_queue,方便在prepare的时候就可以服用io
            let (mut this_item_list,is_done) = source.prepare_items().await.map_err(|e| {
                error!("{} source.prepare_items error: {}", checkpoint_id.as_str(), e);
                anyhow::anyhow!("source.prepare_items error")
            })?;

            let mut total_size = 0;
            let mut item_count = 0;
            for mut item in this_item_list.into_iter() {
                total_size += item.size;
                item_count += 1;
                if item.chunk_id.is_some() && (item.size > SMALL_CHUNK_SIZE || !have_depend_checkpoint) {
                    item.state = BackupItemState::LocalDone;
                } 
                
                engine.task_db.save_backup_item(checkpoint_id.as_str(), &item)?;
                if item.have_cache {
                    if item.state == BackupItemState::LocalDone {
                        debug!("item {}, push to transfer_cache_queue", item.item_id);
                        transfer_cache_queue.push(item);
                    } else {
                        debug!("item {}, push to eval_cache_queue", item.item_id);
                        eval_cache_queue_sender.push(item);
                    }
                } else {
                    if item.state == BackupItemState::LocalDone {
                        debug!("item {}, push to transfer_queue", item.item_id);
                        transfer_queue.push(item);
                    } else {
                        debug!("item {}, push to eval_queue", item.item_id);
                        eval_queue_sender.push(item);
                    }
                }
            }
            
            let mut real_backup_task = backup_task.lock().await;
            real_backup_task.total_size += total_size;
            real_backup_task.item_count += item_count;
            engine.task_db.update_task(&real_backup_task)?;
            if is_done {
                break;
            }
        }

        info!("{} source.prepare_items return done, all items are prepared", checkpoint_id.as_str());
        let mut real_checkpoint = checkpoint.lock().await;
        real_checkpoint.state = CheckPointState::Prepared;
        engine.task_db.update_checkpoint(&real_checkpoint)?;
        drop(real_checkpoint);
        Ok(())
    }



    async fn cacl_item_hash_and_diff(backup_item:&BackupItem,mut item_reader:Pin<Box<dyn ChunkReadSeek + Send + Sync + Unpin>>,need_diff:bool) -> Result<(ChunkId,Option<DiffObject>)> {
        //let chunk_id_str = backup_item.chunk_id.as_ref().unwrap();
        let cache_node_key = backup_item.item_id.as_str();
        item_reader.seek(SeekFrom::Start(0)).await;
        
        let mut offset = 0;
        let mut full_hash_context = ChunkHasher::new(None).map_err(|e| anyhow::anyhow!("{}",e))?;
        debug!("start calc full hash for item: {}, size: {}", backup_item.item_id, backup_item.size);
        let mut full_id = None;
        let mut cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
        let mut cache_node = cache_mgr.get_chunk_cache_node(cache_node_key);
        if cache_node.is_none() {
            cache_mgr.create_chunk_cache(cache_node_key,0).await?;
            cache_node = cache_mgr.get_chunk_cache_node(cache_node_key);
        }
        let mut total_size = cache_mgr.total_size.clone();
        let max_cache_size = cache_mgr.max_size;
        let mut cache_node = cache_node.unwrap();
        drop(cache_mgr);
        
        loop {
            debug!("calc full hash for item: {}, offset: {},len: {}", backup_item.item_id, offset, backup_item.size);

            let (content, mut is_last_piece) = if offset + HASH_CHUNK_SIZE >= backup_item.size {
                let mut content_buffer = vec![0u8; (backup_item.size - offset) as usize];
                item_reader.read_exact(&mut content_buffer).await?;
                debug!("read last piece for item: {}, offset: {},len: {}", backup_item.item_id, offset, backup_item.size);
                (content_buffer, true)
            } else {
                let mut content_buffer = vec![0u8; HASH_CHUNK_SIZE as usize];
                item_reader.read_exact(&mut content_buffer).await?;
                (content_buffer, false)
            };
            let content_len = content.len() as u64;
          
            full_hash_context.update_from_bytes(&content);
            //add to chunk cache
            loop {
                if total_size.load(Ordering::Relaxed) < max_cache_size {
                    total_size.fetch_add(content_len, Ordering::Relaxed);
                    let mut real_cache_node = cache_node.lock().await;
                    real_cache_node.add_piece(content);
                    debug!("add piece to cache, size: {},total_cache_size: {} MB", content_len, total_size.load(Ordering::Relaxed) / 1024 / 1024);
                    break;
                } else {
                    //sleep
                    //debug!("cache is full, sleep 1ms");
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
                }
            }

            offset += content_len;
            if is_last_piece {
                full_id = Some(full_hash_context.finalize_chunk_id());
                break;
            }
        };

        let full_id = full_id.unwrap();
        info!("calc full hash for item: {}, full_id: {}", backup_item.item_id, full_id.to_string());
        Ok((full_id,None))
    }

    pub async fn backup_chunk_source_eval_thread(engine:BackupEngine,source:BackupChunkSourceProvider,target:BackupChunkTargetProvider,
        backup_task:Arc<Mutex<WorkTask>>,task_session:Arc<Mutex<BackupTaskSession>>,checkpoint:Arc<Mutex<BackupCheckPoint>>) -> Result<()> {
        
        let real_task_session = task_session.lock().await;
        let eval_queue = real_task_session.eval_queue.clone();
        let eval_cache_queue = real_task_session.eval_cache_queue.clone();
        let transfer_cache_queue = real_task_session.transfer_cache_queue.clone();
        let transfer_queue = real_task_session.transfer_queue.clone();
        let done_items = real_task_session.done_items.clone();
        drop(real_task_session);

        let real_checkpoint = checkpoint.lock().await;
        let checkpoint_id = real_checkpoint.checkpoint_id.clone();
        let need_diff = real_checkpoint.depend_checkpoint_id.is_some();
        drop(real_checkpoint);
        info!("eval thread start, checkpoint: {}", checkpoint_id);
        loop {
            let real_checkpoint = checkpoint.lock().await;
            if real_checkpoint.state == CheckPointState::Evaluated {
                info!("checkpoint {} is evaluated, exit eval thread", real_checkpoint.checkpoint_id);
                drop(real_checkpoint);
                break;
            }
            drop(real_checkpoint);
          
            loop {
                let real_task = backup_task.lock().await;
                if real_task.state != TaskState::Running {
                    info!("backup task {} is not running, exit eval thread", real_task.taskid);
                    return Err(anyhow::anyhow!("backup task {} is not running", real_task.taskid));
                }
                drop(real_task);

                let mut next_item = eval_cache_queue.pop(); 
                if next_item.is_none() {
                    next_item = eval_queue.pop();
                }
               
                if next_item.is_some() {
                    //process item
                    let mut backup_item = next_item.unwrap();
                    debug!("eval thread process item {}", backup_item.item_id);
                    let real_done_items = done_items.lock().await;
                    if real_done_items.contains_key(&backup_item.item_id) {
                        debug!("item {} is already done, skip", backup_item.item_id);
                        continue;
                    }
                    drop(real_done_items);

                    let mut item_chunk_id = None;
                    if backup_item.chunk_id.is_some() {
                        item_chunk_id = Some(ChunkId::new(backup_item.chunk_id.as_ref().unwrap()).unwrap());
                    } else if backup_item.size > SMALL_CHUNK_SIZE && !engine.is_strict_mode {
                        let item_reader = source.open_item(&backup_item.item_id).await;
                        
                        if item_reader.is_err() {
                            let err = item_reader.err().unwrap();
                            match err {
                                BuckyBackupError::TryLater(msg) => {
                                    warn!("open item {} reader error: {}, try later", backup_item.item_id, msg);
                                    continue;
                                }
                                _ => {
                                    warn!("open item {} reader error", backup_item.item_id);
                                    return Err(anyhow::anyhow!("open item {} reader error", backup_item.item_id));
                                }
                            }
                        }
                        
                        let mut item_reader = item_reader.unwrap();
                        let quick_hash = calc_quick_hash(&mut item_reader, Some(backup_item.size)).await?;
                        info!("{}'s quick_hash: {}", backup_item.item_id, quick_hash.to_string());
                        backup_item.quick_hash = Some(quick_hash.to_string());
                        item_chunk_id = Some(quick_hash);
                    }

                    if item_chunk_id.is_some() {
                        let real_chunk_id = item_chunk_id.unwrap();
                        let (is_exist,chunk_size) = target.is_chunk_exist(&real_chunk_id).await?;
                        if is_exist {
                            //如果item_chunk_id是quick_hash,则需要查询并更新chunk_id
                            let mut is_item_done = true;
                            if backup_item.quick_hash.is_some() {
                                let full_chunk_id = target.query_link_target(&real_chunk_id).await?;
                                if full_chunk_id.is_some() {
                                    let full_chunk_id = full_chunk_id.unwrap();
                                    debug!("query link target for chunk {} success, full_chunk_id: {}", real_chunk_id.to_string(), full_chunk_id.to_string());
                                    backup_item.chunk_id = Some(full_chunk_id.to_string());
                                    engine.task_db.update_backup_item(checkpoint_id.as_str(), &backup_item)?;
                                } else {
                                    warn!("query link target for chunk {} error", real_chunk_id.to_string());
                                    is_item_done = false;
                                }
                            }
                            if is_item_done {
                                info!("item {} 's chunk_id: {}, is exist! will skip", backup_item.item_id, real_chunk_id.to_string());
                                engine.complete_backup_item(checkpoint_id.as_str(), &backup_item, backup_task.clone(),done_items.clone()).await?;
                                continue;
                            }
                        } 
                    }

                    let item_reader = source.open_item(&backup_item.item_id).await;
                    if item_reader.is_err() {
                        let err = item_reader.err().unwrap();
                        match err {
                            BuckyBackupError::TryLater(msg) => {
                                warn!("open item {} reader error: {}, try later", backup_item.item_id, msg);
                                continue;
                            }
                            _ => {
                                warn!("open item {} reader error", backup_item.item_id);
                                return Err(anyhow::anyhow!("open item {} reader error", backup_item.item_id));
                            }
                        }
                    }

                    let item_reader = item_reader.unwrap();
                    let real_transfer_cache_queue = transfer_cache_queue.clone();
                    let backup_item2 = backup_item.clone();
                    if backup_item.quick_hash.is_some() {
                        tokio::spawn(async move {   
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                            real_transfer_cache_queue.push(backup_item2); 
                        });
                    }
                    let (chunk_id,diff_object) = BackupEngine::cacl_item_hash_and_diff(&backup_item,item_reader,need_diff).await?;

                    backup_item.chunk_id = Some(chunk_id.to_string());
                    backup_item.state = BackupItemState::LocalDone;
                    engine.task_db.update_backup_item(checkpoint_id.as_str(), &backup_item)?;
                    if backup_item.quick_hash.is_some() {
                        info!("link chunk_id: {} to quick_hash: {}", chunk_id.to_string(), backup_item.quick_hash.as_ref().unwrap());
                        let quick_hash = backup_item.quick_hash.as_ref().unwrap();
                        let quick_hash_id = ChunkId::new(quick_hash).unwrap();
                        target.link_chunkid(&quick_hash_id,&chunk_id).await?;
                    } else {
                        info!("cacl item {} ,chunk_id: {} complete.", backup_item.item_id, chunk_id.to_string());
                        transfer_cache_queue.push(backup_item); 
                    }
                } else {
                    //idle
                    debug!("eval thread idle...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    break;
                }
            }
            let real_checkpoint = checkpoint.lock().await;
            if real_checkpoint.state == CheckPointState::Prepared {
                info!("checkpoint {} is prepared, try load new backup items from db...", real_checkpoint.checkpoint_id);
                drop(real_checkpoint);
                let new_item_list = engine.task_db.load_wait_cacl_backup_items(&checkpoint_id)?;
                debug!("eval thread load new backup items done, item count: {}", new_item_list.len());
                if !new_item_list.is_empty() {
                    info!("{} new backup items are loaded to eval", new_item_list.len());
                    for item in new_item_list {
                        eval_queue.push(item);
                    }
                } else {
                    info!("all items are calculated, exit eval thread");
                    break;
                }
            }
        }

        let mut real_checkpoint = checkpoint.lock().await;
        real_checkpoint.state = CheckPointState::Evaluated;
        engine.task_db.update_checkpoint(&real_checkpoint)?;
        drop(real_checkpoint);
        info!("eval thread exit,checpoint {} is evaluated", checkpoint_id);
        Ok(())
    }

    pub async fn backup_work_thread(engine:BackupEngine,source:BackupChunkSourceProvider,target:BackupChunkTargetProvider,
        backup_task:Arc<Mutex<WorkTask>>,task_session:Arc<Mutex<BackupTaskSession>>,checkpoint:Arc<Mutex<BackupCheckPoint>>) -> Result<()> {
        let real_task_session = task_session.lock().await;
        let transfer_cache_queue = real_task_session.transfer_cache_queue.clone();
        let transfer_queue = real_task_session.transfer_queue.clone();
        let done_items = real_task_session.done_items.clone();

        drop(real_task_session);
        let backup_task2 = backup_task.clone();
        info!("transfer thread start");
        loop {
            let real_checkpoint = checkpoint.lock().await;
            let checkpoint_id = real_checkpoint.checkpoint_id.clone();
            if real_checkpoint.state == CheckPointState::Done {
                info!("checkpoint {} is done, exit transfer thread", real_checkpoint.checkpoint_id);
                drop(real_checkpoint);
                break;
            }

            if real_checkpoint.state == CheckPointState::Evaluated {
                info!("checkpoint {} is evaluated, try load new backup items from db...", real_checkpoint.checkpoint_id);
                let real_checkpoint_id = real_checkpoint.checkpoint_id.clone();
                drop(real_checkpoint);
                let new_item_list = engine.task_db.load_wait_transfer_backup_items(&real_checkpoint_id)?;
                
                if !new_item_list.is_empty() {
                    info!("{} new backup items are loaded to transfer", new_item_list.len());
                    for item in new_item_list {
                        transfer_queue.push(item);
                    }
                } else {
                    info!("all items are transferred, exit transfer thread");
                    break;
                }
            }
          
            loop {
                let real_task = backup_task.lock().await;
                if real_task.state != TaskState::Running {
                    info!("backup task {} is not running, exit transfer thread", real_task.taskid);
                    return Err(anyhow::anyhow!("backup task {} is not running", real_task.taskid));
                }
                drop(real_task);

                let mut next_item = transfer_cache_queue.pop();
                if next_item.is_none() {
                    next_item = transfer_queue.pop();
                }

                if next_item.is_some() {
                    
                    //do transfer 实现的核目标是:
                    // 1) 实现"只IO"一次的目标,尽量释放chunk piece cache
                    // 2) 减少临时文件(diff)的占用,尽快完成并删除                
                    let backup_item = next_item.unwrap();
                    debug!("transfer thread process item {}", backup_item.item_id);
                    let real_done_items = done_items.lock().await;
                    if real_done_items.contains_key(&backup_item.item_id) {
                        debug!("item {} is already done, skip", backup_item.item_id);
                        continue;
                    }
                    drop(real_done_items);

                    let chunk_id_str = if let Some(chunk_id) = &backup_item.chunk_id {
                        chunk_id
                    } else {
                        backup_item.quick_hash.as_ref().unwrap()
                    };
                    debug!("will upload chunk_id_str: {}", chunk_id_str);
                    let chunk_id = ChunkId::new(chunk_id_str).unwrap();
                    let real_chunk_id = chunk_id.clone();
            
                    let open_result = target.open_chunk_writer(&chunk_id,0,backup_item.size).await;
                    if open_result.is_err() {
                        let err = open_result.err().unwrap();
                        match err {
                            BuckyBackupError::AlreadyDone(msg) => {
                                info!("chunk {} already exist, skip upload", chunk_id.to_string());
                                engine.complete_backup_item(checkpoint_id.as_str(), &backup_item, backup_task.clone(),done_items.clone()).await?;
                                let mut cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
                                cache_mgr.free_chunk_cache(backup_item.chunk_id.as_ref().unwrap()).await;
                                drop(cache_mgr);
                                continue;
                            }
                            BuckyBackupError::TryLater(msg) => {
                                warn!("open chunk {} writer error: {}, try later", chunk_id.to_string(), msg);
                                continue;
                            }
                            _ => {
                                warn!("open chunk {} writer error: {}", chunk_id.to_string(), err.to_string());
                                return Err(anyhow::anyhow!("open chunk {} writer error: {}", chunk_id.to_string(), err.to_string()));
                            }
                        }
                    }
                    let (mut writer,init_offset) = open_result.unwrap();
                    let mut offset = init_offset;
                    
                    info!("start upload chunk {} , offset: {}, size: {}", chunk_id_str, offset, backup_item.size);
                    let mut this_item_cache_node = None;
                    let mut cache_start_offset = 0;
                    let mut cache_end_offset = 0;
                    let cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
                    let mgr_total_size = cache_mgr.total_size.clone();
                    let chunk_cache_node = cache_mgr.get_chunk_cache_node(backup_item.item_id.as_str());
                    drop(cache_mgr);

                    if chunk_cache_node.is_some() {
                        let chunk_cache_node = chunk_cache_node.unwrap();
                        //let mut chunk_cache_node = chunk_cache_node.unwrap();
                        this_item_cache_node = Some(chunk_cache_node.clone());
                        let mut chunk_cache_node = chunk_cache_node.lock().await;
                        let free_size = chunk_cache_node.free_piece_before_offset(offset);
                        if free_size > 0 {
                            debug!("free cache size: {},offset: {},cache_start_pos: {}", free_size, offset, chunk_cache_node.start_offset);
                            mgr_total_size.fetch_sub(free_size, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                   
                    let mut upload_done = false;
                    let mut real_reader = None;
                    loop {
                        if offset == backup_item.size {
                            upload_done = true;
                            break;
                        }
                        if this_item_cache_node.is_none() {
                            let cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
                            let chunk_cache_node = cache_mgr.get_chunk_cache_node(backup_item.item_id.as_str());
                            if chunk_cache_node.is_some() {
                                let chunk_cache_node = chunk_cache_node.unwrap();
                                this_item_cache_node = Some(chunk_cache_node.clone());
                            }
                            drop(cache_mgr);
                        } 
                        
                        if this_item_cache_node.is_some() {
                            let chunk_cache_node = this_item_cache_node.as_mut().unwrap().lock().await;
                            cache_start_offset = chunk_cache_node.start_offset;
                            cache_end_offset = chunk_cache_node.end_offset;
                            debug!("cache node start offset: {}, end offset: {}", cache_start_offset, cache_end_offset);
                        }
                        
                        let mut send_buf = vec![0u8; COPY_CHUNK_BUFFER_SIZE];
                        let mut upload_len:u64 = 0;  
                        if offset < cache_start_offset || offset >= cache_end_offset {
                            if real_reader.is_none() {
                                debug!("open item {} reader, offset: {}", backup_item.item_id, offset);
                                let mut reader = source.open_item_chunk_reader(&backup_item.item_id,offset).await;
                                if reader.is_err() {
                                    let err = reader.err().unwrap();
                                    match err {
                                        BuckyBackupError::TryLater(msg) => {
                                            warn!("open item {} reader error: {}, try later", backup_item.item_id, msg);
                                            break;
                                        }
                                        _ => {
                                            warn!("open item {} reader error", backup_item.item_id);
                                            return Err(anyhow::anyhow!("open item {} reader error", backup_item.item_id));
                                        }
                                    }
                                }
                                let reader = reader.unwrap();
                                real_reader = Some(reader);
                            }
                            
                            let mut reader = real_reader.as_mut().unwrap();
                            let mut read_len = 0;
                            let read_result;
                            if offset < cache_start_offset {
                                if cache_start_offset - offset > send_buf.len() as u64 {
                                    read_result = reader.read(&mut send_buf).await;
                                } else {
                                    read_result = reader.read(&mut send_buf[..(cache_start_offset - offset) as usize]).await;
                                }
                            } else {
                                read_result = reader.read(&mut send_buf).await;
                            }
                            if read_result.is_err() {
                                warn!("read item {} error: {}", backup_item.item_id, read_result.err().unwrap().to_string());
                                break;
                            } 

                            read_len = read_result.unwrap();
                            if read_len == 0 {
                                warn!("read item {} unexpect EOF", backup_item.item_id);
                                break;
                            }
                            upload_len = read_len as u64;
                            writer.write_all(&send_buf[..read_len]).await?;
                            debug!("upload chunk {} & read from source, offset: {} + {} , size: {}", chunk_id_str, offset, upload_len, backup_item.size);
                        } else {
                            let chunk_cache_node = this_item_cache_node.as_mut().unwrap();
                            let mut chunk_cache_node = chunk_cache_node.lock().await;
                            debug!("cache pieces: {:?}",chunk_cache_node.cache_pieces);
                            let cache_piece = chunk_cache_node.cache_pieces.pop();

                            if cache_piece.is_some() {
                                let (piece_start_offset,cache_piece) = cache_piece.unwrap();
                                debug!("pop cache piece start offset: {}, piece len: {}", piece_start_offset, cache_piece.len());
                                if piece_start_offset != offset {
                                    warn!("cache piece start offset: {} not equal to offset: {}", piece_start_offset, offset);
                                    return Err(anyhow::anyhow!("cache piece start offset: {} not equal to offset: {}", piece_start_offset, offset));
                                }

                                upload_len = cache_piece.len() as u64;
                                chunk_cache_node.start_offset += upload_len;
                                cache_start_offset = chunk_cache_node.start_offset;
                                mgr_total_size.fetch_sub(upload_len, std::sync::atomic::Ordering::Relaxed);
                                drop(chunk_cache_node);
                                //debug!("hit cache piece for chunk {}, offset: {} + {} = {} , size: {}", chunk_id_str, offset, upload_len, offset + upload_len, backup_item.size);
                                writer.write_all(&cache_piece).await?;
                                debug!("upload chunk {} & pop cache piece, offset: {} + {} = {} , size: {}", chunk_id_str, offset, upload_len, offset + upload_len, backup_item.size);
                            } else {
                                debug!("no cache piece for chunk {}, offset: {}, size: {}, cache_start_offset: {},cache_end_offset: {}", 
                                chunk_id_str, offset, backup_item.size,cache_start_offset,cache_end_offset);
                                break;
                            }
                        }

                        offset += upload_len;
                        let mut real_task = backup_task.lock().await;
                        real_task.completed_size += upload_len;
                        if real_task.state != TaskState::Running {
                            debug!("backup task {} is not running, break upload loop", real_task.taskid);
                            break;
                        }
                        drop(real_task);
                    }

                    if upload_done {
                        target.complete_chunk_writer(&chunk_id).await?;
                        engine.complete_backup_item(checkpoint_id.as_str(), &backup_item, backup_task.clone(),done_items.clone()).await?;
                        info!("chunk {} backup done", chunk_id_str);
                    } else {
                        info!("chunk {} backup not done", chunk_id_str);
                    }
                    let mut cache_mgr = CHUNK_TASK_CACHE_MGR.lock().await;
                    cache_mgr.free_chunk_cache(backup_item.item_id.as_str()).await;
                    drop(cache_mgr);

                } else {
                    //idle
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    break;
                }
            }
        }
        
        let mut real_task = backup_task.lock().await;
        real_task.state = TaskState::Done;
        engine.task_db.update_task(&real_task)?;
        info!("backup task {} done", real_task.taskid);

        Ok(())
    }


    //return taskid
    pub async fn create_restore_task(&self,plan_id: &str,check_point_id: &str, restore_config: RestoreConfig) -> Result<String> {
        if self.is_plan_have_running_backup_task(plan_id).await {
            return Err(anyhow::anyhow!("plan {} already has a running backup task", plan_id));
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

    fn check_all_check_point_exist(&self,checkpoint_id: &str) -> Result<bool> {
        let checkpoint = self.task_db.load_checkpoint_by_id(checkpoint_id)?;
        if checkpoint.state != CheckPointState::Done {
            info!("checkpoint {} is not done! cannot restore", checkpoint_id);
            return Ok(false);
        }

        if checkpoint.depend_checkpoint_id.is_none() {
            return Ok(true);
        }
        debug!("checkpoint {} depend checkpoint: {}", checkpoint_id, checkpoint.depend_checkpoint_id.as_ref().unwrap());
        let parent_checkpoint_id = checkpoint.depend_checkpoint_id.as_ref().unwrap();
        let result = self.check_all_check_point_exist(parent_checkpoint_id)?;
        Ok(result)
    }


    async fn run_chunk2chunk_restore_task(&self,restore_task:Arc<Mutex<WorkTask>>,checkpoint_id: String,
        source:BackupChunkSourceProvider, target:BackupChunkTargetProvider) -> Result<()>{
        
        let mut real_task = restore_task.lock().await;
        let need_build_items = real_task.item_count == 0;
        let real_task_id = real_task.taskid.clone();
        let restore_config = real_task.restore_config.clone();
        if restore_config.is_none() {
            return Err(anyhow::anyhow!("restore config is none"));
        }
        let restore_config = restore_config.unwrap();

        let mut restore_item_list;
        if need_build_items {
            drop(real_task);
            source.init_for_restore(&restore_config).await?;
            restore_item_list = Vec::new();
            if !self.check_all_check_point_exist(&checkpoint_id)? {
                return Err(anyhow::anyhow!("checkpoint {} not exist", checkpoint_id));
            }
            
            let backup_items = self.task_db.load_backup_items_by_checkpoint(&checkpoint_id)?;
            info!("load {} backup items for checkpoint: {}", backup_items.len(), checkpoint_id);
           
            let now = buckyos_get_unix_timestamp();
            let mut total_size = 0;
            for item in backup_items {
                let restore_item = BackupItem {
                    item_id: item.item_id.clone(),
                    item_type: item.item_type,
                    chunk_id: item.chunk_id,
                    quick_hash: item.quick_hash,
                    state: BackupItemState::New,
                    size: item.size,
                    last_modify_time: now,
                    create_time: now,
                    have_cache: false,
                    progress: "".to_string(),
                    diff_info: None,
                };
                restore_item_list.push(restore_item);
                total_size += item.size;
            }
            let mut real_task = restore_task.lock().await;
            self.task_db.save_restore_item_list_to_task(&real_task.taskid, &restore_item_list)?;
            real_task.item_count = restore_item_list.len() as u64;
            real_task.total_size = total_size;
            real_task.update_time = now;
            self.task_db.update_task(&real_task)?;

        } else {
            //load restore item from db
            restore_item_list = self.task_db.load_restore_items_by_task(&real_task_id, &BackupItemState::New)?;
            let uncomplete_size = restore_item_list.iter().map(|item| item.size).sum::<u64>();
            real_task.completed_item_count = real_task.item_count - restore_item_list.len() as u64;
            real_task.completed_size = real_task.total_size - uncomplete_size;
            self.task_db.update_task(&real_task)?;
            drop(real_task);
        }
        
        for item in restore_item_list {
            info!("start restore item: {:?} ... ", item);
            if item.chunk_id.is_none() {
                warn!("restore item {} has no chunk_id,skip restore", item.item_id);
                return Err(anyhow::anyhow!("restore item {} has no chunk_id, in-complete checkpoint? skip restore", item.item_id));
            }
            let mut offset = 0;
            let mut real_hash_state:Option<ChunkHasher> = None;
            if item.progress.len() > 2  {
                let json_value = serde_json::from_str::<serde_json::Value>(&item.progress);
                if json_value.is_err() {
                    warn!("invalid progress info:{}",item.progress.as_str());
                } else {
                    let json_value = json_value.unwrap();
                    let hash_state = ChunkHasher::restore_from_state(json_value);
                    if hash_state.is_err() {
                        warn!("invalid progress info:{}",item.progress.as_str());
                    } else {
                        let hash_state = hash_state.unwrap();
                        offset = hash_state.pos;
                        real_hash_state  = Some(hash_state);
                        info!("load progress sucess!,pos:{}",offset);
                    }
                }
            } 

            let open_resulut = source.open_writer_for_restore(&item,&restore_config,offset).await;
            if open_resulut.is_err() {
                warn!("item {} already exist~ skip restore.",item.item_id);
                let mut real_task = restore_task.lock().await;
                real_task.completed_item_count += 1;
                real_task.completed_size += item.size;
                self.task_db.update_restore_item_state(&real_task_id, &item.item_id, BackupItemState::Done)?;
                continue;
            }

            let (mut chunk_writer,real_offset) = open_resulut.unwrap();
            if real_offset != offset {
                offset = 0;
                (chunk_writer,_)= source.open_writer_for_restore(&item,&restore_config,offset).await?;
            }
            if offset == 0 {
                real_hash_state = Some(ChunkHasher::new(None).unwrap());
            }

            let chunk_id = ChunkId::new(item.chunk_id.as_ref().unwrap()).unwrap();
            let mut chunk_reader = target.open_chunk_reader_for_restore(&chunk_id, offset).await?;

            let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
            let progress_callback = {
                Some(move |chunk_id: ChunkId, pos: u64, hasher: &Option<ChunkHasher>| {
                    let this_chunk_id = chunk_id.clone();
                    let mut json_progress_str = String::new();
                    if let Some(hasher) = hasher {
                        let state = hasher.save_state();
                        json_progress_str = serde_json::to_string(&state).unwrap(); 
                    }
                    let counter = counter.clone();
    
                    Box::pin(async move {
                        let count = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count % 16 == 0 {
                            info!("restore item {} progress: {}", chunk_id.to_string(), json_progress_str);
                        }
                        NdnResult::Ok(())
                    }) as Pin<Box<dyn Future<Output = NdnResult<()>> + Send>>
                })
            };

            let copy_bytes = copy_chunk(chunk_id, &mut chunk_reader, &mut chunk_writer, real_hash_state,progress_callback).await?;
            
            //set item state to done & update task state
            let mut real_task = restore_task.lock().await;
            real_task.completed_item_count += 1;
            real_task.completed_size += item.size;
            self.task_db.update_restore_item_state(&real_task_id, &item.item_id, BackupItemState::Done)?;
            info!("restore item {} done", item.item_id);
        }

        Ok(())
    }

    async fn run_dir2chunk_restore_task(&self, plan_id: &str, check_point_id: &str) -> Result<()> {
        unimplemented!()
    }

    async fn run_dir2dir_restore_task(&self, plan_id: &str, check_point_id: &str) -> Result<()> {
        unimplemented!()
    }

    async fn get_chunk_source_provider(&self, source_url:&str) -> Result<BackupChunkSourceProvider> {
        let url = Url::parse(source_url)?;
        assert_eq!(url.scheme(), "file");
        
        let store = LocalDirChunkProvider::new(url.path().to_string()).await?;
        Ok(Box::new(store))
    }

    async fn get_chunk_target_provider(&self, target_url:&str) -> Result<BackupChunkTargetProvider> {
        let url = Url::parse(target_url)?;
        assert_eq!(url.scheme(), "file");
        let store = LocalChunkTargetProvider::new(url.path().to_string()).await?;
        Ok(Box::new(store))
        //Ok(store)
    }

    pub async fn list_backup_tasks(&self, filter:&str) -> Result<Vec<String>> {
        self.task_db.list_worktasks(filter).map_err(|e| {
            let err_str = e.to_string();
            warn!("list work tasks error: {}", err_str.as_str());
            anyhow::anyhow!("list work tasks error: {}", err_str)
        })
    }

    pub async fn get_task_info(&self, taskid: &str) -> Result<WorkTask> {
        let mut all_tasks = self.all_tasks.lock().await;
        let mut backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            let _backup_task = self.task_db.load_task_by_id(taskid)?;
            all_tasks.insert(taskid.to_string(), Arc::new(Mutex::new(_backup_task)));
            backup_task = all_tasks.get(taskid);
        }

        if backup_task.is_none() {
            return Err(anyhow::anyhow!("task not found"));
        }
        let backup_task = backup_task.unwrap().lock().await.clone();
        Ok(backup_task)
    }

    pub async fn resume_restore_task(&self, taskid: &str) -> Result<()> {
        let mut all_tasks = self.all_tasks.lock().await;
        let mut restore_task = all_tasks.get(taskid);
        if restore_task.is_none() {
            error!("restore task not found: {}", taskid);
            return Err(anyhow::anyhow!("task not found"));
        }
        let restore_task = restore_task.unwrap().clone();
        drop(all_tasks);

        let mut real_restore_task = restore_task.lock().await;
        if real_restore_task.state != TaskState::Paused {
            warn!("restore task is not paused, ignore resume");
            return Err(anyhow::anyhow!("restore task is not paused"));
        }
        real_restore_task.state = TaskState::Running;
        let task_id = real_restore_task.taskid.clone();
        let checkpoint_id = real_restore_task.checkpoint_id.clone();
        let owner_plan_id = real_restore_task.owner_plan_id.clone();

        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(&owner_plan_id);
        if plan.is_none() {
            error!("task plan not found: {} plan_id: {}", taskid,owner_plan_id.as_str());
            return Err(anyhow::anyhow!("task plan not found"));
        }
        let plan = plan.unwrap().lock().await;
        let task_type = plan.type_str.clone();
        let source_provider = self.get_chunk_source_provider(plan.source.get_source_url()).await?;
        let target_provider = self.get_chunk_target_provider(plan.target.get_target_url()).await?;

        drop(plan);
        drop(all_plans);

        info!("resume restore task: {} type: {}", taskid, task_type.as_str());
        let taskid = task_id.clone();
        let engine:BackupEngine = self.clone();
        let restore_task = restore_task.clone();
        tokio::spawn(async move {
            let task_result = match task_type.as_str() {
                "c2c" => engine.run_chunk2chunk_restore_task(restore_task.clone(), checkpoint_id, source_provider, target_provider).await,
                //"d2c" => engine.run_dir2chunk_backup_task(backup_task, source_provider, target_provider).await,
                //"d2d" => engine.run_dir2dir_backup_task(backup_task, source_provider, target_provider).await,
                //"c2d" => engine.run_chunk2dir_backup_task(backup_task, source_provider, target_provider).await,
                _ => Err(anyhow::anyhow!("unknown plan type: {}", task_type)),
            };

            let mut real_restore_task = restore_task.lock().await;
            if task_result.is_err() {
                info!("restore task failed: {} {}", taskid.as_str(), task_result.err().unwrap());
                real_restore_task.state = TaskState::Failed;
            } else {
                info!("restore task done: {} ", taskid.as_str());
                real_restore_task.state = TaskState::Done;
            }
            engine.task_db.update_task(&real_restore_task);
        }); 
        
        Ok(())
    }

    pub async fn resume_work_task(&self, taskid: &str) -> Result<()> {
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
        if real_backup_task.state != TaskState::Paused {
            warn!("task is not paused, ignore resume");
            return Err(anyhow::anyhow!("task is not paused"));
        }
        real_backup_task.state = TaskState::Running;
        let task_id = real_backup_task.taskid.clone();
        let checkpoint_id = real_backup_task.checkpoint_id.clone();
        let owner_plan_id = real_backup_task.owner_plan_id.clone();
       

        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(&owner_plan_id);
        if plan.is_none() {
            error!("task plan not found: {} plan_id: {}", taskid,owner_plan_id.as_str());
            return Err(anyhow::anyhow!("task plan not found"));
        }
        let plan = plan.unwrap().lock().await;
        let task_type = plan.type_str.clone();
        let source_provider = self.get_chunk_source_provider(plan.source.get_source_url()).await?;
        let target_provider = self.get_chunk_target_provider(plan.target.get_target_url()).await?;
    
        drop(plan);
        drop(all_plans);

        info!("resume backup task: {} type: {}", taskid, task_type.as_str());
        let taskid = task_id.clone();
        let engine:BackupEngine = self.clone();
        let backup_task = backup_task.clone();
        tokio::spawn(async move {
            let task_result = match task_type.as_str() {
                "c2c" => engine.run_chunk2chunk_backup_task(backup_task.clone(), checkpoint_id, source_provider, target_provider).await,
                //"d2c" => engine.run_dir2chunk_backup_task(backup_task, source_provider, target_provider).await,
                //"d2d" => engine.run_dir2dir_backup_task(backup_task, source_provider, target_provider).await,
                //"c2d" => engine.run_chunk2dir_backup_task(backup_task, source_provider, target_provider).await,
                _ => Err(anyhow::anyhow!("unknown plan type: {}", task_type)),
            };

            //let all_tasks = engine.all_tasks.lock().await;
            // let mut backup_task = all_tasks.get_mut(taskid);
            let mut real_backup_task = backup_task.lock().await;
            if task_result.is_err() {
                info!("backup task failed: {} {}", taskid.as_str(), task_result.err().unwrap());
                real_backup_task.state = TaskState::Failed;
            } else {
                info!("backup task done: {} ", taskid.as_str());
                real_backup_task.state = TaskState::Done;
            }
            engine.task_db.update_task(&real_backup_task);
        });

        Ok(())
    }

    pub async fn pause_work_task(&self, taskid: &str) -> Result<()> {
        let all_tasks = self.all_tasks.lock().await;
        let backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            error!("task not found: {}", taskid);
            return Err(anyhow::anyhow!("task not found"));
        }
        let mut backup_task = backup_task.unwrap().lock().await;
        if backup_task.state != TaskState::Running {
            warn!("task is not running, ignore pause");
            return Err(anyhow::anyhow!("task is not running"));
        }
        backup_task.state = TaskState::Paused;
        //self.task_db.pause_task(taskid)?;
        Ok(())
    }

    pub async fn cancel_backup_task(&self, taskid: &str) -> Result<()> {
        unimplemented!()
    }

}



//impl kRPC for BackupEngine


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_c2c_backup_task() {
        std::env::set_var("BUCKY_LOG", "debug");
        buckyos_kit::init_logging("bucky_backup_test");
        let tempdb = "/opt/buckyos/data/backup_suite/bucky_backup.db";
        //delete db file if exists
        if std::path::Path::new(tempdb).exists() {
            std::fs::remove_file(tempdb).unwrap();
        }

        let engine = BackupEngine::new();
        engine.start().await.unwrap();
        let new_plan = BackupPlanConfig::chunk2chunk("file:///tmp/test", "file:///tmp/bucky_backup_result", "testc2c", "testc2c desc");
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
        buckyos_kit::init_logging("bucky_backup_test");

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

        let task_id = engine.create_restore_task(&plan_id, &checkpoint_id, restore_config).await.unwrap();
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


