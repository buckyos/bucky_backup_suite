// engine 是backup_suite的核心，负责统一管理配置，备份任务
#![allow(unused)]
use std::io::SeekFrom;
use std::sync::Arc;
use std::collections::HashMap;
use futures::stream::futures_unordered::IterMut;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
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

const SMALL_CHUNK_SIZE:u64 = 1024*1024;//1MB
const LARGE_CHUNK_SIZE:u64 = 1024*1024*256; //256MB 
const HASH_CHUNK_SIZE:u64 = 1024*1024*16; //16MB

lazy_static!{
    pub static ref DEFAULT_ENGINE : Arc<Mutex<BackupEngine>> = Arc::new(Mutex::new(BackupEngine::new()));
}


pub struct RestoreConfig {
    pub restore_target: BackupSource,
    pub description: String,
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
    task_db: BackupTaskDb,
    small_file_content_cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    is_strict_mode: bool,
}

impl BackupEngine {
    pub fn new() -> Self {
        Self {
            all_plans: Arc::new(Mutex::new(HashMap::new())),
            all_tasks: Arc::new(Mutex::new(HashMap::new())),
            all_checkpoints: Arc::new(Mutex::new(HashMap::new())),
            task_db: BackupTaskDb::new("bucky_backup.db"),
            small_file_content_cache: Arc::new(Mutex::new(HashMap::new())),
            is_strict_mode: false,
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
    
    async fn is_plan_have_running_backup_task(&self, plan_id: &str) -> bool {
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

    //return taskid
    pub async fn create_restore_task(&self,plan_id: &str,check_point_id: &str, restore_config: RestoreConfig) -> Result<String> {
        unimplemented!()
    }

    async fn run_chunk2chunk_backup_task(&self,backup_task:Arc<Mutex<WorkTask>>,checkpoint_id: String,
        source:BackupChunkSourceProvider, target:BackupChunkTargetProvider) -> Result<()> {
        //this is source prepare thread
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
        let wait_cacle_item_list = Arc::new(Mutex::new(vec![]));
        let mut real_checkpoint = checkpoint.lock().await;
        match real_checkpoint.state {
            CheckPointState::Done => {
                info!("checkpoint show already done: {},bakcup task ended", checkpoint_id.as_str());
                return Ok(());
            },
            CheckPointState::Failed => {
                error!("CheckPointState::failed: {},bakcup task ended", checkpoint_id.as_str());
                return Err(anyhow::anyhow!("CheckPointState::failed"));
            },
            CheckPointState::New => {
                info!("start source.prepare backup_item_list for checkpoint: {}", checkpoint_id.as_str());
                drop(real_checkpoint);
                //因为prepare的过程可能中断，这里是否要先删除所有的backup item?
                loop {
                    //chunk source 比较简单，一次调用就可以得到所有的chunk,dir需要一直调用prepare直到返回完成。
                    //dir source的prepare_items方法需要更多的参数，方便在prepare的过程中“完成更多的工作”
                    let (this_item_list,is_done) = source.prepare_items().await.map_err(|e| {
                        error!("{} source.prepare_items error: {}", checkpoint_id.as_str(), e);
                        anyhow::anyhow!("source.prepare_items error")
                    })?;
                   
                    let total_size:u64 = this_item_list.iter().map(|item| item.size).sum();
                    let mut real_backup_task = backup_task.lock().await;
                    real_backup_task.total_size += total_size;
                    real_backup_task.item_count += this_item_list.len() as u64;

                    self.task_db.update_task(&real_backup_task)?;
                    self.task_db.save_item_list_to_checkpoint(&real_backup_task.checkpoint_id.as_str(), &this_item_list)?;
                    let mut real_wait_cacle_item_list = wait_cacle_item_list.lock().await;
                    real_wait_cacle_item_list.extend(this_item_list);  

                    if is_done {
                        info!("{} source.prepare_items return done, all items are prepared", checkpoint_id.as_str());
                        let mut real_checkpoint = checkpoint.lock().await;
                        real_checkpoint.state = CheckPointState::Prepared;
                        self.task_db.update_checkpoint(&real_checkpoint)?;
                        drop(real_checkpoint);
                        break;
                    }
                }
            },
            _ => {
                //all item confirmed and there is some backup work to do
                //item_list = self.task_db.load_work_backup_items(&checkpoint_id)?;
                info!("{} checkpoint is already prepared,skip prepare", checkpoint_id.as_str());
            }
        }

        let engine = self.clone();
        let engine2 = self.clone();
        let engine3 = self.clone();
        let source_url = source.get_source_url();

        let source2 = self.get_chunk_source_provider(source.get_source_url().as_str()).await?;
        let target2 = self.get_chunk_target_provider(target.get_target_url().as_str()).await?;

        //eval线程和transfer线程的逻辑未来可以通用化（为所有类型的task共享）
        let backup_task_eval = backup_task.clone();
        let backup_task_trans = backup_task.clone();
        let backup_task_readitem = backup_task.clone();

        let checkpoint_id2 = checkpoint_id.clone();
        let checkpoint2 = checkpoint.clone();
        let checkpoint_id3 = checkpoint_id.clone();
        let checkpoint3 = checkpoint.clone();
        //transfer cache 的大小很重要，32片数据的内存消耗最大是512MB
        let (tx_transfer_cache, mut rx_transfer_cache) = mpsc::channel::<TransferCacheNode>(64);
        let tx_transfer_cache2 = tx_transfer_cache.clone();

        //读取未完成的item,并根据状态决定是发送到eval线程还是trans线程
        //create engine.eval thread to cacle hash and diff

        let local_eval_thread = tokio::spawn(async move {
            info!("start engine.eval thread,cacl hash and diff for all backup items");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            loop {
                let mut backup_task = backup_task.lock().await;
                if backup_task.state != TaskState::Running {
                    return Err(anyhow::anyhow!("backup task is not running,exit eval thread"));
                }
                drop(backup_task);

                let mut calc_item_list = vec![];
                let mut real_wait_cacle_item_list = wait_cacle_item_list.lock().await;
                if !real_wait_cacle_item_list.is_empty() {
                    calc_item_list.extend(real_wait_cacle_item_list.drain(..));
                    info!("{} items are ready to eval", calc_item_list.len());
                    drop(real_wait_cacle_item_list);
                } else {
                    drop(real_wait_cacle_item_list);
                    calc_item_list = engine.task_db.load_wait_cacl_backup_items(&checkpoint_id2)?;
                    if calc_item_list.is_empty() {
                        let mut real_checkpoint = checkpoint2.lock().await;
                        if real_checkpoint.state == CheckPointState::Prepared { 
                            info!("all items are calculated, exit eval thread");
                            real_checkpoint.state = CheckPointState::Evaluated;
                            engine.task_db.update_checkpoint(&real_checkpoint)?;
                            return Ok(());
                        } else {
                            info!("wait source prepare items, sleep 2 seconds");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                    }
                }

                while let Some(mut backup_item) = calc_item_list.pop() {
                    let mut real_backup_task = backup_task_eval.lock().await;
                    if real_backup_task.state != TaskState::Running {
                        return Err(anyhow::anyhow!("backup task is not running,exit eval thread"));
                    }
                    drop(real_backup_task);

                    info!("eval item: {} checkpoint: {}", backup_item.item_id, checkpoint_id2);
                    if backup_item.size < SMALL_CHUNK_SIZE {
                        //给出警告，太小的Chunk并不适合Chunk Target这种模式
                        warn!("chunk backup item {} is too small,some thing wrong?", backup_item.item_id);
                        let item_content = source.get_item_data(&backup_item.item_id).await;
                        if item_content.is_err() {
                            warn!("get smallitem {} content error", backup_item.item_id);
                            continue;
                        }

                        let item_content = item_content.unwrap();
                        let mut full_hasher = ChunkHasher::new(None);
                        let hash_result = full_hasher.calc_from_bytes(&item_content);
                        let chunk_id = ChunkId::from_sha256_result(&hash_result);
                        let chunk_id_str = chunk_id.to_string();
            
                        let mut small_file_cache = engine.small_file_content_cache.lock().await;
                        small_file_cache.insert(chunk_id_str.clone(), item_content);
                        drop(small_file_cache);

                        backup_item.state = BackupItemState::LocalDone;
                        backup_item.chunk_id = Some(chunk_id_str);
                        engine.task_db.update_backup_item(checkpoint_id2.as_str(), &backup_item)?;
                        info!("small backup item {} cacl full_hash done.", backup_item.item_id);
                    } else {
                        let mut item_reader = source.open_item(&backup_item.item_id).await;
                        if item_reader.is_err() {
                            warn!("open item {} reader error", backup_item.item_id);
                            continue;
                        }
                        let mut item_reader = item_reader.unwrap();
                        //info!("start calc quick hash for item: {}", backup_item.item_id);

                        let quick_hash = calc_quick_hash(&mut item_reader, Some(backup_item.size)).await?;
                        let quick_hash_str = quick_hash.to_string();
                        let quick_hash_str2 = quick_hash_str.clone();
                        info!("quick hash for item: {} is {}", backup_item.item_id, quick_hash_str.as_str());
                        let (is_exist,chunk_size) = target.is_chunk_exist(&quick_hash).await?;
                        if is_exist {
                            if !is_strict_mode {
                                backup_item.state = BackupItemState::Done;
                                //backup_item.chunk_id = Some(quick_hash_str2.clone());
                                backup_item.quick_hash = Some(quick_hash_str2.clone());
                                engine.task_db.update_backup_item(checkpoint_id2.as_str(), &backup_item)?;
                                info!("backup item {} skipped by quick check, quick_hash: {}", backup_item.item_id, quick_hash_str2.as_str());
                                continue;
                            } 
                        }

                        //使用quick_hash进行put操作，在传输完成后再进行 link_chankid
                        info!("start calc full hash for item: {}", backup_item.item_id);
                        item_reader.seek(SeekFrom::Start(0)).await?;
                        let mut offset = 0;
                        let mut full_hash_context = ChunkHasher::new(None);
                        let full_id = loop {
                            info!("calc full hash for item: {}, offset: {},len: {}", backup_item.item_id, offset, backup_item.size);

                            let (content, mut is_last_piece) = if offset + HASH_CHUNK_SIZE >= backup_item.size {
                                let mut content_buffer = vec![0u8; (backup_item.size - offset) as usize];
                                item_reader.read_exact(&mut content_buffer).await?;
                                info!("read last piece for item: {}, offset: {},len: {}", backup_item.item_id, offset, backup_item.size);
                                (content_buffer, true)
                            } else {
                                let mut content_buffer = vec![0u8; HASH_CHUNK_SIZE as usize];
                                item_reader.read_exact(&mut content_buffer).await?;
                                (content_buffer, false)
                            };
                            full_hash_context.update_from_bytes(&content);

                            if is_last_piece {
                                let hash_result = full_hash_context.finalize();
                                let full_id = ChunkId::from_sha256_result(&hash_result).to_string();
                                tx_transfer_cache.send(TransferCacheNode{
                                    item_id: backup_item.item_id.clone(),
                                    chunk_id: quick_hash_str2.clone(),
                                    offset,
                                    is_last_piece,
                                    content,
                                    full_id: Some(full_id.clone()),
                                    total_size: backup_item.size
                                }).await?;
                                info!("{} full_hash is {}", backup_item.item_id, full_id.as_str());
                                break full_id;
                            } else {
                                tx_transfer_cache.send(TransferCacheNode{
                                    item_id: backup_item.item_id.clone(),
                                    chunk_id: quick_hash_str2.clone(),
                                    offset,
                                    is_last_piece,
                                    content,
                                    full_id: None,
                                    total_size: backup_item.size
                                }).await?;
                            }
                            offset += HASH_CHUNK_SIZE;
                        };

                        backup_item.state = BackupItemState::LocalDone;
                        backup_item.chunk_id = Some(full_id);
                        engine.task_db.update_backup_item(checkpoint_id2.as_str(), &backup_item)?;
                        info!("backup item {} full hash cacl done", backup_item.item_id);
                    }
                }
            }
            Ok(())
        });


        let trans_thread = tokio::spawn(async move {
            info!("start engine.transfer thread,transfer item by item");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let tx_transfer_cache3 = tx_transfer_cache2.clone();
            let mut timeout_sec = 5;
            let mut already_create_read_item_thread = false;
            loop {
                let mut real_backup_task = backup_task_trans.lock().await;
                if real_backup_task.state != TaskState::Running {
                    return Err(anyhow::anyhow!("backup task is not running"));
                }
                drop(real_backup_task);

                //首先尝试清空小文件缓存
                let mut small_file_cache = engine3.small_file_content_cache.lock().await;
                if !small_file_cache.is_empty() {
                    let current_cache = std::mem::replace(&mut *small_file_cache, HashMap::new());
                    drop(small_file_cache);
                    info!("transfer {} small file cache to target", current_cache.len());

                    // target2.write_vectored(current_cache.into_iter().map(|(chunk_id, content)| {
                    //     let content_length = content.len() as u64;
                    //     ChunkWrite {
                    //         chunk_id: chunk_id.clone(), 
                    //         offset: 0, 
                    //         reader: Cursor::new(content), 
                    //         length: Some(content_length), 
                    //         tail: Some(content_length), 
                    //         full_id: None
                    //     }
                    // }).collect()).await?;
                    //发送成功，需要将这些backup item的状设置为done
                } else {
                    info!("no small file cache to transfer");
                    drop(small_file_cache);
                }
                
                match timeout(Duration::from_secs(timeout_sec), rx_transfer_cache.recv()).await {
                    StdResult::Ok(cache_node) => {
                        if cache_node.is_none() {
                            continue;
                        }
                        let cache_node = cache_node.unwrap();
                        let content_length = cache_node.content.len() as u64; 
                        let chunk_id = ChunkId::new(cache_node.chunk_id.as_str()).unwrap();
                        let (is_exist,chunk_size) = target2.is_chunk_exist(&chunk_id).await?;
                        if is_exist {
                            info!("chunk {} already exist,skip", cache_node.chunk_id);
                            engine3.task_db.update_backup_item_state(checkpoint_id3.as_str(),cache_node.item_id.as_str(),BackupItemState::Done)?;
                            let mut real_task = backup_task_trans.lock().await;
                            real_task.completed_size += content_length;
                            real_task.completed_item_count += 1;
                            engine3.task_db.update_task(&real_task)?;
                            drop(real_task);
                            continue;
                        }

                        if cache_node.is_last_piece {
                            let put_result;
                            if cache_node.offset == 0 {
                                put_result = target2.put_chunk(&chunk_id, &cache_node.content).await;
                            } else {
                                put_result = target2.append_chunk_data(&chunk_id, cache_node.offset, &cache_node.content, true,Some(cache_node.total_size)).await;
                            }

                            if put_result.is_err() {
                                warn!("put/append last chunk {} error: {}", cache_node.chunk_id, put_result.err().unwrap());
                                continue;
                            }
                            if cache_node.total_size > HASH_CHUNK_SIZE {
                                //do link
                                let full_chunk_id = ChunkId::new(cache_node.full_id.as_ref().unwrap()).unwrap();
                                info!("link chunk {:?} ===> {:?}", &full_chunk_id,&cache_node.chunk_id);
                                let link_result = target2.link_chunkid(&chunk_id, &full_chunk_id).await;
                                if link_result.is_err() {
                                    warn!("link chunk {} to {} error: {}", cache_node.chunk_id, cache_node.full_id.as_ref().unwrap(), link_result.err().unwrap());
                                }
                            }
                            info!("put/append chunk {} success", cache_node.chunk_id);
                            //crate backup item and set it state to done;
                            engine3.task_db.update_backup_item_state(checkpoint_id3.as_str(),cache_node.item_id.as_str(),BackupItemState::Done)?;
                            let mut real_task = backup_task_trans.lock().await;
                            real_task.completed_size += content_length;
                            real_task.completed_item_count += 1;
                            engine3.task_db.update_task(&real_task)?;
                            drop(real_task);
                            //udpate 
                        } else {
                            target2.append_chunk_data(&chunk_id, cache_node.offset, &cache_node.content, false,Some(cache_node.total_size)).await?;
                            engine3.task_db.update_backup_item_state(checkpoint_id3.as_str(),cache_node.item_id.as_str(),BackupItemState::Done)?;
                            let mut real_task = backup_task_trans.lock().await;
                            real_task.completed_size += content_length;
                            //engine3.task_db.update_task(&real_task)?;
                        }
                    }
                    StdResult::Err(_) => {
                        info!("transfer cache receive timeout after 5 seconds, continue...");
                        if already_create_read_item_thread {
                            info!("already create read item thread, exit transfer thread");
                            return Ok(());
                        }
                        //try load send cache from db
                        let mut real_checkpoint = checkpoint3.lock().await;
                        let checkpoint_state = real_checkpoint.state.clone();
                        let backup_task_trans2 = backup_task_trans.clone();
                        drop(real_checkpoint);

                        if checkpoint_state == CheckPointState::Evaluated {
                            already_create_read_item_thread = true;
                            let checkpoint_id4 = checkpoint_id3.clone();
                            let engine4 = engine3.clone();
                            
                            let source_url = source_url.clone();
                            let tx_transfer_cache4 = tx_transfer_cache3.clone();
                            let read_item_thread = tokio::spawn(async move {
                                let source3 = engine4.get_chunk_source_provider(source_url.as_str()).await?;
                                info!("all items are evaluated, start read_item_thread for checkpoint: {}", checkpoint_id4.as_str());
                                let item_list = engine4.task_db.load_wait_transfer_backup_items(&checkpoint_id4)?;
                                for item in item_list {
                                    let mut real_backup_task = backup_task_trans2.lock().await;
                                    if real_backup_task.state != TaskState::Running {
                                        return Err(anyhow::anyhow!("backup task is not running"));
                                    }
                                    drop(real_backup_task);

                                    let chunk_id_str;
                                    if item.chunk_id.is_none() {
                                        warn!("item {} has no chunk_id,skip transfer", item.item_id);
                                        return Err(anyhow::anyhow!("item has no chunk_id"));
                                    }
                                    chunk_id_str = item.chunk_id.as_ref().unwrap().clone();
                                    let mut offset = 0;
                                    let item_reader = source3.open_item(&item.item_id).await;
                                    if item_reader.is_err() {
                                        warn!("open item {} reader error", item.item_id);
                                        continue;
                                    }
                                    let mut item_reader = item_reader.unwrap();
                                    loop {
                                        let (content, is_last_piece) = if offset >= item.size - HASH_CHUNK_SIZE {
                                            let mut content_buffer = vec![0u8; (item.size - offset) as usize];
                                            item_reader.read_exact(&mut content_buffer).await?;
                                            (content_buffer, true)
                                        } else {
                                            let mut content_buffer = vec![0u8; HASH_CHUNK_SIZE as usize];
                                            item_reader.read_exact(&mut content_buffer).await?;
                                            (content_buffer, false)
                                        };

                                        tx_transfer_cache4.send(TransferCacheNode{
                                            item_id: item.item_id.clone(),
                                            chunk_id: chunk_id_str.clone(),
                                            offset,
                                            is_last_piece,
                                            content,
                                            full_id: Some(chunk_id_str.clone()),
                                            total_size: item.size
                                        }).await?;

                                        if is_last_piece {
                                            break;
                                        }
                                        offset += HASH_CHUNK_SIZE;
                                    }
                                }
                                info!("read item thread exit");
                                Ok(())
                            });
                        }
                    }
                }
            
            }
            info!("transfer thread exit");
            Ok(())
        });

        tokio::join!(local_eval_thread, trans_thread);
        Ok(())
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

    async fn run_chunk2chunk_restore_task(&self,backup_task: WorkTask) -> Result<()>{
        unimplemented!()
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

    pub async fn list_running_backup_tasks(&self, filter:&str) -> Result<Vec<String>> {
        unimplemented!()
    }

    pub async fn list_done_backup_tasks(&self, filter:&str) -> Result<Vec<String>> {
        unimplemented!()
    }

    pub async fn list_all_backup_tasks(&self, filter:&str) -> Result<Vec<String>> {
        unimplemented!()
    }

    pub async fn list_failed_backup_tasks(&self, filter:&str) -> Result<Vec<String>> {
        unimplemented!()
    }


    pub async fn get_task_info(&self, taskid: &str) -> Result<WorkTask> {
        let all_tasks = self.all_tasks.lock().await;
        let backup_task = all_tasks.get(taskid);
        if backup_task.is_none() {
            return Err(anyhow::anyhow!("task not found"));
        }
        let backup_task = backup_task.unwrap().lock().await.clone();
        Ok(backup_task)
    }

    pub async fn resume_backup_task(&self, taskid: &str) -> Result<()> {
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

    pub async fn pause_backup_task(&self, taskid: &str) -> Result<()> {
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
        self.task_db.pause_task(taskid)?;
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
        buckyos_kit::init_logging("bucky_backup_test");
        let tempdb = "bucky_backup.db";
        //delete db file if exists
        if std::path::Path::new(tempdb).exists() {
            std::fs::remove_file(tempdb).unwrap();
        }

        let engine = BackupEngine::new();
        engine.start().await.unwrap();
        let new_plan = BackupPlanConfig::chunk2chunk("file:///mnt/d/temp/test", "file:///mnt/d/temp/bucky_backup_result", "testc2c", "testc2c desc");
        let plan_id = engine.create_backup_plan(new_plan).await.unwrap();
        let task_id = engine.create_backup_task(&plan_id, None).await.unwrap();
        engine.resume_backup_task(&task_id).await.unwrap();
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

        let restore_config = RestoreConfig {
            restore_target: BackupSource::Directory("file:///d/temp/restore_result".to_string()),
            description: "test c2c restore".to_string(),
        };
        engine.create_restore_task(&plan_id, &check_point_id, restore_config).await.unwrap();
    }
}


