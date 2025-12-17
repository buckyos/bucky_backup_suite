// engine 是backup_suite的核心，负责统一管理配置，备份任务
#![allow(unused)]

use base64;
use buckyos_backup_lib::*;
use buckyos_kit::buckyos_get_unix_timestamp;
use buckyos_kit::get_buckyos_service_data_dir;
use dyn_clone::DynClone;
use futures::future::join_all;
use futures::future::select_all;
use futures::stream::futures_unordered::IterMut;
use lazy_static::lazy_static;
use log::*;
use ndn_lib::*;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::io::Cursor;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use tokio::fs;
use tokio::io::AsyncRead;
use tokio::io::BufWriter;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use url::Url;

use std::result::Result as StdResult;

use crate::file_system_monitor::FileSystemMonitor;
use crate::snapshot::{self, remove_snapshot_dir};
use crate::task_db::*;
use crate::work_task::*;
use crate::*;

use buckyos_backup_lib::BackupResult;
use buckyos_backup_lib::BuckyBackupError;
use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, LocalResult, NaiveDate, TimeZone,
    Timelike, Utc, Weekday,
};

const SMALL_CHUNK_SIZE: u64 = 1024 * 1024; //1MB
const LARGE_CHUNK_SIZE: u64 = 1024 * 1024 * 256; //256MB
const HASH_CHUNK_SIZE: u64 = 1024 * 1024 * 16; //16MB
const REMOVE_WAIT_ATTEMPTS: usize = 120;
const REMOVE_WAIT_INTERVAL_MS: u64 = 500;
const FAILED_RETRY_COOLDOWN_MS: u64 = 5 * 60 * 1000; // 5分钟冷却时间

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
    all_targets: Arc<Mutex<HashMap<String, Arc<Mutex<BackupTargetRecord>>>>>,
    small_file_content_cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    is_strict_mode: bool,
    task_db: BackupTaskDb,
    task_session: Arc<Mutex<HashMap<String, Arc<Mutex<BackupTaskSession>>>>>,
    all_chunk_source_providers: Arc<
        Mutex<
            HashMap<
                String,
                (
                    BackupSourceProviderDesc,
                    Arc<Mutex<BackupChunkSourceCreateFunc>>,
                ),
            >,
        >,
    >,
    all_chunk_target_providers: Arc<
        Mutex<
            HashMap<
                String,
                (
                    BackupTargetProviderDesc,
                    Arc<Mutex<BackupChunkTargetCreateFunc>>,
                ),
            >,
        >,
    >,
    lock_create_task: Arc<Mutex<()>>,
    cleanup_removed_task_lock: Arc<Mutex<()>>,
    fs_monitor: Option<Arc<FileSystemMonitor>>,
    event_watch_registry: Arc<Mutex<EventWatchRegistry>>,
}

#[derive(Debug, Default, Clone)]
pub struct BackupPlanUpdateParams {
    pub title: Option<String>,
    pub description: Option<String>,
    pub policy: Option<Value>,
    pub priority: Option<u64>,
    pub policy_disabled: Option<bool>,
    pub reserved_versions: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PlanPolicyRule {
    Period(PlanPolicyPeriod),
    Event(PlanPolicyEvent),
}

#[derive(Debug, Clone, Deserialize)]
struct PlanPolicyPeriod {
    minutes: u64,
    #[serde(default)]
    week: Option<u32>,
    #[serde(default)]
    date: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlanPolicyEvent {
    update_delay: u64,
}

#[derive(Default)]
struct EventWatchRegistry {
    plan_paths: HashMap<String, PathBuf>,
    path_ref_counts: HashMap<PathBuf, usize>,
}

impl EventWatchRegistry {
    fn plan_path(&self, plan_id: &str) -> Option<PathBuf> {
        self.plan_paths.get(plan_id).cloned()
    }

    fn has_path(&self, path: &PathBuf) -> bool {
        self.path_ref_counts.contains_key(path)
    }

    fn detach_plan(&mut self, plan_id: &str) -> Option<(PathBuf, bool)> {
        self.plan_paths.remove(plan_id).map(|path| {
            let should_remove = match self.path_ref_counts.get_mut(&path) {
                Some(count) => {
                    if *count <= 1 {
                        self.path_ref_counts.remove(&path);
                        true
                    } else {
                        *count -= 1;
                        false
                    }
                }
                None => true,
            };
            (path, should_remove)
        })
    }

    fn attach_plan(&mut self, plan_id: &str, path: PathBuf) -> bool {
        let counter = self.path_ref_counts.entry(path.clone()).or_insert(0);
        let need_add = *counter == 0;
        *counter += 1;
        self.plan_paths.insert(plan_id.to_string(), path);
        need_add
    }
}

impl BackupEngine {
    pub fn new() -> Self {
        let task_db_path = get_buckyos_service_data_dir("backup_suite").join("bucky_backup.db");
        let fs_monitor = match FileSystemMonitor::new() {
            Ok(monitor) => Some(Arc::new(monitor)),
            Err(err) => {
                warn!(
                    "failed to initialize file system monitor, event policies disabled: {}",
                    err
                );
                None
            }
        };

        let result = Self {
            all_plans: Arc::new(Mutex::new(HashMap::new())),
            all_tasks: Arc::new(Mutex::new(HashMap::new())),
            all_checkpoints: Arc::new(Mutex::new(HashMap::new())),
            all_targets: Arc::new(Mutex::new(HashMap::new())),
            task_db: BackupTaskDb::new(task_db_path.to_str().unwrap()),
            small_file_content_cache: Arc::new(Mutex::new(HashMap::new())),
            is_strict_mode: false,
            task_session: Arc::new(Mutex::new(HashMap::new())),
            all_chunk_source_providers: Arc::new(Mutex::new(HashMap::new())),
            all_chunk_target_providers: Arc::new(Mutex::new(HashMap::new())),
            lock_create_task: Arc::new(Mutex::new(())),
            cleanup_removed_task_lock: Arc::new(Mutex::new(())),
            fs_monitor,
            event_watch_registry: Arc::new(Mutex::new(EventWatchRegistry::default())),
        };

        return result;
    }

    pub async fn start(&self) -> BackupResult<()> {
        let local_chunk_source_desc = BackupSourceProviderDesc {
            name: "local filesystem backup source".to_string(),
            desc: "local filesystem backup source,support use a local filesystem as backup source"
                .to_string(),
            type_id: "file".to_string(),
            abilities: vec![ABILITY_CHUNK_LIST.to_string()],
        };

        self.register_backup_chunk_source_provider(
            local_chunk_source_desc,
            Box::new(move |local_path: String| {
                Box::pin(async move {
                    let result =
                        LocalDirChunkProvider::new(local_path, "backup_local_cache".to_string())
                            .await?;
                    Ok(Box::new(result) as BackupChunkSourceProvider)
                })
            }),
        )
        .await?;

        let local_chunk_target_desc = BackupTargetProviderDesc {
            name: "local filesystem backup target".to_string(),
            desc: "local filesystem backup target,support use a local filesystem as backup target"
                .to_string(),
            type_id: "file".to_string(),
            abilities: vec![ABILITY_CHUNK_LIST.to_string()],
        };

        self.register_backup_chunk_target_provider(
            local_chunk_target_desc,
            Box::new(move |local_path: String| {
                Box::pin(async move {
                    let result = LocalChunkTargetProvider::new(
                        local_path,
                        "backup_local_storage".to_string(),
                    )
                    .await?;
                    Ok(Box::new(result) as BackupChunkTargetProvider)
                })
            }),
        )
        .await?;

        let target_ids = self.task_db.list_backup_target_ids().map_err(|e| {
            error!("list backup targets error: {}", e.to_string());
            BuckyBackupError::Failed(e.to_string())
        })?;
        {
            let mut all_targets = self.all_targets.lock().await;
            for target_id in target_ids {
                match self.task_db.get_backup_target(&target_id) {
                    Ok(record) => {
                        all_targets.insert(target_id.clone(), Arc::new(Mutex::new(record)));
                    }
                    Err(err) => {
                        warn!(
                            "load backup target {} failed: {}",
                            target_id,
                            err.to_string()
                        );
                    }
                }
            }
        }

        let plans = self.task_db.list_backup_plans().map_err(|e| {
            error!("list backup plans error: {}", e.to_string());
            BuckyBackupError::Failed(e.to_string())
        })?;
        for plan in plans {
            let plan_key = plan.get_plan_key();
            {
                let mut all_plans = self.all_plans.lock().await;
                all_plans.insert(plan_key.clone(), Arc::new(Mutex::new(plan.clone())));
            }
            info!("load backup plan: {}", plan_key);
            let policies = parse_plan_policies(&plan_key, &plan.policy);
            self.sync_event_watch_for_plan(&plan_key, &plan, &policies)
                .await;
        }

        let scheduler_engine = self.clone();
        tokio::task::spawn(async move {
            scheduler_engine.schedule().await;
        });

        Ok(())
    }

    pub async fn stop(&self) -> BackupResult<()> {
        // stop all running task
        Ok(())
    }

    fn task_concurrency_limit(&self) -> usize {
        match self.task_db.get_setting("task_concurrency") {
            Ok(Some(value)) => value
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|limit| *limit > 0)
                .unwrap_or(5),
            Ok(None) => 5,
            Err(err) => {
                warn!(
                    "failed to load task_concurrency setting, fallback to 5: {}",
                    err
                );
                5
            }
        }
    }

    pub async fn schedule(&self) {
        info!("backup engine scheduler started");
        loop {
            let now_utc = Utc::now();
            if let Err(err) = self.auto_start_due_plans(now_utc).await {
                warn!("auto start plans failed: {}", err);
            }

            let concurrency_limit = self.task_concurrency_limit();

            let running_snapshots: Vec<_> = {
                let all_tasks = self.all_tasks.lock().await;
                all_tasks.values().cloned().collect()
            };
            let mut running_count = 0usize;
            for task_arc in running_snapshots {
                let task = task_arc.lock().await;
                if matches!(task.state, TaskState::Running) {
                    running_count += 1;
                }
            }

            if running_count < concurrency_limit {
                match self.task_db.list_schedulable_tasks() {
                    Ok(tasks) => {
                        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                        for task in tasks {
                            if running_count >= concurrency_limit {
                                break;
                            }

                            let current_state = {
                                let task_arc = {
                                    let mut all_tasks = self.all_tasks.lock().await;
                                    all_tasks.entry(task.taskid.clone()).or_insert(Arc::new(Mutex::new(task.clone()))).clone()
                                };
                                let task_guard = task_arc.lock().await;
                                task_guard.state.clone()
                            };

                            if !matches!(current_state, TaskState::Pending | TaskState::Failed(_)) {
                                continue;
                            }

                            if matches!(current_state, TaskState::Failed(_)) {
                                let last_failed_at = task.update_time;
                                if now_ms.saturating_sub(last_failed_at) < FAILED_RETRY_COOLDOWN_MS
                                {
                                    debug!(
                                        "skip scheduling task {} due to recent failure",
                                        task.taskid
                                    );
                                    continue;
                                }
                            }

                            if let Err(err) = self.resume_work_task(task.taskid.as_str()).await {
                                warn!("scheduler failed to start task {}: {}", task.taskid, err);
                                continue;
                            }

                            running_count += 1;
                        }
                    }
                    Err(err) => {
                        warn!("list schedulable tasks failed: {}", err);
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn auto_start_due_plans(&self, now_utc: DateTime<Utc>) -> BackupResult<()> {
        let plan_entries: Vec<(String, Arc<Mutex<BackupPlanConfig>>)> = {
            let all_plans = self.all_plans.lock().await;
            all_plans
                .iter()
                .map(|(plan_id, plan_arc)| (plan_id.clone(), plan_arc.clone()))
                .collect()
        };

        for (plan_id, plan_arc) in plan_entries {
            let plan_snapshot = plan_arc.lock().await.clone();
            if plan_snapshot.policy_disabled {
                continue;
            }
            if plan_snapshot
                .policy
                .as_array()
                .map(|policies| policies.is_empty())
                .unwrap_or(true)
            {
                continue;
            }
            if self.is_plan_have_runable_backup_task(&plan_id).await {
                continue;
            }

            match self
                .should_auto_start_plan(&plan_id, &plan_snapshot, now_utc)
                .await
            {
                Ok(true) => match self.create_backup_task(&plan_id, None).await {
                    Ok(task_id) => {
                        info!(
                            "auto scheduled backup task {} for plan {}",
                            task_id, plan_id
                        );
                        if let Err(err) = self.resume_work_task(&task_id).await {
                            warn!(
                                "failed to resume auto scheduled task {} for plan {}: {}",
                                task_id, plan_id, err
                            );
                        }
                    }
                    Err(err) => {
                        warn!(
                            "auto scheduling plan {} failed when creating task: {}",
                            plan_id, err
                        );
                    }
                },
                Ok(false) => {}
                Err(err) => {
                    warn!("auto scheduling check failed for plan {}: {}", plan_id, err);
                }
            }
        }

        Ok(())
    }

    async fn should_auto_start_plan(
        &self,
        plan_id: &str,
        plan: &BackupPlanConfig,
        now_utc: DateTime<Utc>,
    ) -> BackupResult<bool> {
        let policies = parse_plan_policies(plan_id, &plan.policy);
        if policies.is_empty() {
            return Ok(false);
        }

        let last_created = self
            .task_db
            .get_last_backup_task_create_time(plan_id)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        let last_completed = self
            .task_db
            .get_last_completed_backup_time(plan_id)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;

        let now_ms_raw = now_utc.timestamp_millis();
        let now_ms = if now_ms_raw <= 0 {
            0
        } else {
            now_ms_raw as u64
        };
        let now_local = now_utc.with_timezone(&Local);
        let period_due = latest_period_due_time(&policies, now_local);

        debug!("check auto run, plan-id: {}, now: {}, period_due: {:?}, now-ms: {}, last-created-ms: {:?}, plan-create-ms: {}", plan_id, now_local, period_due, now_ms, last_created, plan.create_time);

        if let Some(period_due) = period_due {
            if period_due <= now_ms
                && period_due > last_created.unwrap_or(0)
                && period_due >= plan.create_time
            {
                return Ok(true);
            }
        }

        let mut latest_update = None;
        for policy in policies.iter() {
            if let PlanPolicyRule::Event(event) = policy {
                if latest_update.is_none() {
                    latest_update = self.latest_monitored_update(plan_id).await;
                }
                if is_event_policy_due(
                    last_completed,
                    now_ms,
                    event,
                    plan.create_time,
                    latest_update,
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn sync_event_watch_for_plan(
        &self,
        plan_id: &str,
        plan: &BackupPlanConfig,
        policies: &[PlanPolicyRule],
    ) {
        if !policies_contain_event(policies) {
            self.clear_event_watch_for_plan(plan_id).await;
            return;
        }

        let monitor = match &self.fs_monitor {
            Some(monitor) => monitor.clone(),
            None => return,
        };

        let Some(path) = resolve_plan_source_path(plan_id, plan) else {
            self.clear_event_watch_for_plan(plan_id).await;
            return;
        };

        let mut registry = self.event_watch_registry.lock().await;
        if registry
            .plan_path(plan_id)
            .as_ref()
            .map(|current| current == &path)
            .unwrap_or(false)
        {
            return;
        }

        let need_add_new = !registry.has_path(&path);
        if need_add_new {
            if let Err(err) = monitor.add_path(&path) {
                warn!(
                    "plan {} failed to add filesystem watch for {}: {}",
                    plan_id,
                    path.display(),
                    err
                );
                return;
            }
        }

        let previous = registry.detach_plan(plan_id);
        registry.attach_plan(plan_id, path.clone());
        drop(registry);

        if let Some((old_path, should_remove)) = previous {
            if should_remove {
                if let Err(err) = monitor.remove_path(&old_path) {
                    warn!(
                        "plan {} failed to remove filesystem watch for {}: {}",
                        plan_id,
                        old_path.display(),
                        err
                    );
                }
            }
        }

        debug!(
            "plan {} event policy now watches {}",
            plan_id,
            path.display()
        );
    }

    async fn clear_event_watch_for_plan(&self, plan_id: &str) {
        let monitor = self.fs_monitor.as_ref().cloned();
        let mut registry = self.event_watch_registry.lock().await;
        let removed = registry.detach_plan(plan_id);
        drop(registry);

        if let Some((path, should_remove)) = removed {
            if should_remove {
                if let Some(monitor) = monitor {
                    if let Err(err) = monitor.remove_path(&path) {
                        warn!(
                            "plan {} failed to remove filesystem watch for {}: {}",
                            plan_id,
                            path.display(),
                            err
                        );
                    }
                }
            }
        }
    }

    async fn latest_monitored_update(&self, plan_id: &str) -> Option<u64> {
        let monitor = self.fs_monitor.as_ref()?.clone();
        let path = {
            let registry = self.event_watch_registry.lock().await;
            registry.plan_path(plan_id)
        }?;
        monitor.get_last_update_time(&path)
    }

    pub async fn register_backup_chunk_source_provider(
        &self,
        desc: BackupSourceProviderDesc,
        create_func: BackupChunkSourceCreateFunc,
    ) -> BackupResult<()> {
        let mut all_chunk_source_providers = self.all_chunk_source_providers.lock().await;
        if all_chunk_source_providers.contains_key(&desc.type_id) {
            return Err(BuckyBackupError::Failed(format!(
                "chunk source provider already registered"
            )));
        }
        all_chunk_source_providers.insert(
            desc.type_id.clone(),
            (desc, Arc::new(Mutex::new(create_func))),
        );
        Ok(())
    }

    pub async fn register_backup_chunk_target_provider(
        &self,
        desc: BackupTargetProviderDesc,
        create_func: BackupChunkTargetCreateFunc,
    ) -> BackupResult<()> {
        let mut all_chunk_target_providers = self.all_chunk_target_providers.lock().await;
        if all_chunk_target_providers.contains_key(&desc.type_id) {
            return Err(BuckyBackupError::Failed(format!(
                "chunk target provider already registered"
            )));
        }
        all_chunk_target_providers.insert(
            desc.type_id.clone(),
            (desc, Arc::new(Mutex::new(create_func))),
        );
        Ok(())
    }

    pub async fn is_plan_have_running_backup_task(&self, plan_id: &str) -> bool {
        let all_tasks = self.all_tasks.lock().await;
        for (task_id, task) in all_tasks.iter() {
            let real_task = task.lock().await;
            if real_task.owner_plan_id == plan_id
                && real_task.task_type == TaskType::Backup
                && (real_task.state == TaskState::Running || real_task.state == TaskState::Pending)
            {
                return true;
            }
        }
        false
    }

    pub async fn is_plan_have_runable_backup_task(&self, plan_id: &str) -> bool {
        {
            let all_tasks = self.all_tasks.lock().await;
            for task in all_tasks.values() {
                let real_task = task.lock().await;
                if real_task.owner_plan_id == plan_id
                    && real_task.task_type == TaskType::Backup
                    && real_task.state != TaskState::Done
                    && real_task.state != TaskState::Remove
                {
                    return true;
                }
            }
        }

        match self.task_db.plan_has_runable_backup_task(plan_id) {
            Ok(result) => result,
            Err(err) => {
                warn!(
                    "failed to check runable backup tasks for plan {}: {}",
                    plan_id, err
                );
                false
            }
        }
    }

    pub async fn is_restoring_task_dup(
        &self,
        plan_id: &str,
        check_point_id: &str,
        cfg: &RestoreConfig,
    ) -> bool {
        let target_url = translate_local_path_from_url(&cfg.restore_location_url)
            .unwrap_or("".to_string())
            .to_lowercase();
        let all_tasks = self.all_tasks.lock().await;
        for (task_id, task) in all_tasks.iter() {
            let real_task = task.lock().await;
            if real_task.owner_plan_id == plan_id
                && real_task.task_type == TaskType::Restore
                && real_task.state != TaskState::Done
                && real_task.checkpoint_id == check_point_id
                && real_task.restore_config.as_ref().map_or(true, |old_cfg| {
                    translate_local_path_from_url(&old_cfg.restore_location_url)
                        .unwrap_or("".to_string())
                        .to_lowercase()
                        == target_url
                })
            {
                return true;
            }
        }
        false
    }

    //return planid
    pub async fn create_backup_plan(
        &self,
        mut plan_config: BackupPlanConfig,
    ) -> BackupResult<String> {
        self.get_target_record(plan_config.target.as_str()).await?;
        let plan_key = plan_config.get_plan_key();
        let mut all_plans = self.all_plans.lock().await;
        if all_plans.contains_key(&plan_key) {
            return Err(BuckyBackupError::Failed(format!("plan already exists")));
        }

        if plan_config.create_time == 0 {
            let now = Utc::now().timestamp_millis() as u64;
            plan_config.create_time = now;
            plan_config.update_time = now;
        } else if plan_config.update_time == 0 {
            plan_config.update_time = plan_config.create_time;
        }

        self.task_db.create_backup_plan(&plan_config)?;
        info!("create backup plan: [{}] {:?}", plan_key, plan_config);
        let plan_snapshot = plan_config.clone();
        all_plans.insert(plan_key.clone(), Arc::new(Mutex::new(plan_config)));
        drop(all_plans);
        let policies = parse_plan_policies(&plan_key, &plan_snapshot.policy);
        self.sync_event_watch_for_plan(&plan_key, &plan_snapshot, &policies)
            .await;
        Ok(plan_key)
    }

    pub async fn get_backup_plan(&self, plan_id: &str) -> BackupResult<BackupPlanConfig> {
        let maybe_plan = {
            let mut all_plans = self.all_plans.lock().await;
            if let Some(plan) = all_plans.get(plan_id) {
                Some(plan.clone())
            } else {
                match self.task_db.get_backup_plan_by_id(plan_id) {
                    Ok(plan_config) => {
                        let plan_arc = Arc::new(Mutex::new(plan_config));
                        all_plans.insert(plan_id.to_string(), plan_arc.clone());
                        Some(plan_arc)
                    }
                    Err(err) => {
                        error!(
                            "plan {} not found in memory or db: {}",
                            plan_id,
                            err.to_string()
                        );
                        None
                    }
                }
            }
        };

        if let Some(plan_arc) = maybe_plan {
            let plan = plan_arc.lock().await;
            Ok(plan.clone())
        } else {
            Err(BuckyBackupError::NotFound(format!(
                "plan {} not found",
                plan_id
            )))
        }
    }

    pub async fn delete_backup_plan(&self, plan_id: &str) -> BackupResult<()> {
        // Ensure the plan exists (and cached) before attempting removal
        let _ = self.get_backup_plan(plan_id).await?;

        if self.task_db.plan_has_non_removed_tasks(plan_id)? {
            return Err(BuckyBackupError::Failed(format!(
                "plan {} has backup or restore tasks that are not deleted",
                plan_id
            )));
        }

        self.task_db.delete_backup_plan(plan_id)?;

        {
            let mut all_plans = self.all_plans.lock().await;
            all_plans.remove(plan_id);
        }
        self.clear_event_watch_for_plan(plan_id).await;
        Ok(())
    }

    pub async fn update_backup_plan(
        &self,
        plan_id: &str,
        params: BackupPlanUpdateParams,
    ) -> BackupResult<BackupPlanConfig> {
        // Ensure plan exists and loaded into cache
        let _ = self.get_backup_plan(plan_id).await?;

        let plan_arc = {
            let all_plans = self.all_plans.lock().await;
            all_plans.get(plan_id).cloned()
        };

        let plan_arc = plan_arc.ok_or_else(|| {
            BuckyBackupError::NotFound(format!("plan {} not found in cache", plan_id))
        })?;

        let mut plan = plan_arc.lock().await;
        if let Some(title) = params.title {
            plan.title = title;
        }
        if let Some(description) = params.description {
            plan.description = description;
        }
        if let Some(policy) = params.policy {
            plan.policy = policy;
        }
        if let Some(priority) = params.priority {
            plan.priority = priority;
        }
        if let Some(policy_disabled) = params.policy_disabled {
            plan.policy_disabled = policy_disabled;
        }
        plan.update_time = Utc::now().timestamp_millis() as u64;

        self.task_db.update_backup_plan(&plan)?;
        let plan_snapshot = plan.clone();
        drop(plan);
        drop(plan_arc);
        let policies = parse_plan_policies(plan_id, &plan_snapshot.policy);
        self.sync_event_watch_for_plan(plan_id, &plan_snapshot, &policies)
            .await;
        Ok(plan_snapshot)
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
        let create_1_task_in_same_time = self.lock_create_task.lock().await;

        if self.is_plan_have_runable_backup_task(plan_id).await {
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
        plan.update_time = Utc::now().timestamp_millis() as u64;
        let last_checkpoint_index = plan.last_checkpoint_index;
        self.task_db.update_backup_plan(&plan)?;
        let checkpoint_type = plan.get_checkpiont_type();
        drop(plan);
        drop(all_plans);

        let new_checkpoint_id = uuid::Uuid::new_v4().to_string();
        let new_checkpoint = BackupCheckpoint::new(
            checkpoint_type,
            "test_checkpoint".to_string(),
            parent_checkpoint_id.map(|id| id.to_string()),
            None,
        );
        let local_checkpoint = LocalBackupCheckpoint::new(
            new_checkpoint,
            new_checkpoint_id.clone(),
            plan_id.to_string(),
        );
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

        drop(create_1_task_in_same_time);
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

    async fn update_backup_checkpoint(
        &self,
        checkpoint_id: &str,
        state: CheckPointState,
        owner_task: Arc<Mutex<WorkTask>>,
    ) -> BackupResult<()> {
        {
            let all_checkpoints = self.all_checkpoints.lock().await;
            let checkpoint = all_checkpoints.get(checkpoint_id);
            if checkpoint.is_some() {
                let checkpoint = checkpoint.unwrap();
                let mut real_checkpoint = checkpoint.lock().await;
                real_checkpoint.state = state.clone();
            }
            self.task_db
                .update_checkpoint_state(checkpoint_id, state.clone())?;
        }

        // {
        //     let mut real_task = owner_task.lock().await;
        //     let mut new_task_state = None;
        //     match state {
        //         CheckPointState::Done => match real_task.state {
        //             TaskState::Done => {}
        //             _ => new_task_state = Some(TaskState::Done),
        //         },
        //         CheckPointState::Failed(msg) => new_task_state = Some(TaskState::Failed(msg)),
        //         _ => {}
        //     }
        //     if let Some(new_state) = new_task_state {
        //         real_task.state = new_state;
        //         self.task_db.update_task(&real_task)?;
        //     }
        // }
        Ok(())
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
        self.task_db
            .update_task(&real_task)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
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
        let local_checkpoint = self.task_db.load_checkpoint_by_id(checkpoint_id.as_str())?;
        if !local_checkpoint.state.need_working() {
            info!(
                "checkpoint {} is not need working, exit backup thread",
                checkpoint_id
            );
            return Ok(());
        }

        // 保存 state 以便后续使用
        let checkpoint_state = local_checkpoint.state.clone();

        // 获取或创建 checkpoint Arc
        let mut all_checkpoints = self.all_checkpoints.lock().await;
        let checkpoint = all_checkpoints.get(&checkpoint_id).cloned();
        let checkpoint = if checkpoint.is_some() {
            checkpoint.unwrap()
        } else {
            let checkpoint_arc = Arc::new(Mutex::new(local_checkpoint));
            all_checkpoints.insert(checkpoint_id.clone(), checkpoint_arc.clone());
            checkpoint_arc
        };
        drop(all_checkpoints);

        // 准备需要的变量
        let engine = self.clone();
        let source_url = source.get_source_url();
        let target_url = target.get_target_url();
        let task_id = {
            let task = backup_task.lock().await;
            task.taskid.clone()
        };
        let task_session = Arc::new(Mutex::new(BackupTaskSession::new(task_id)));
        let backup_task_prepare = backup_task.clone();
        let backup_task_work = backup_task.clone();
        let checkpoint_clone = checkpoint.clone();
        let task_session_prepare = task_session.clone();
        let task_session_work = task_session.clone();

        // 重新创建 source provider
        let source_prepare = engine
            .get_chunk_source_provider(&source_url)
            .await
            .map_err(|err| {
                error!("prepare thread: failed to create source provider: {}", err);
                err
            })?;

        // 重新创建 source 和 target providers
        let source_work = engine
            .get_chunk_source_provider(&source_url)
            .await
            .map_err(|err| {
                error!("work thread: failed to create source provider: {}", err);
                err
            })?;

        let target_work = engine
            .get_chunk_target_provider(&target_url)
            .await
            .map_err(|err| {
                error!("work thread: failed to create target provider: {}", err);
                err
            })?;

        let running_thread_count = Arc::new(AtomicU32::new(2));

        if checkpoint_state == CheckPointState::New {
            //start prepare thread
            let engine_prepare = engine.clone();
            let source_url_prepare = source_url.clone();
            let running_thread_count_prepare = running_thread_count.clone();

            let prepare_thread = tokio::spawn(async move {
                let result = BackupEngine::backup_chunk_source_prepare_thread(
                    engine_prepare.clone(),
                    source_prepare,
                    backup_task_prepare.clone(),
                    task_session_prepare,
                    checkpoint_clone,
                )
                .await;
                let old_thread_count =
                    running_thread_count_prepare.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if let Err(err) = result {
                    error!("prepare thread error: {}", err);
                    // task failed
                    let mut real_task = backup_task_prepare.lock().await;
                    let mut new_task_state = None;
                    if real_task.state == TaskState::Running {
                        new_task_state =
                            Some(TaskState::Failed(format!("Prepare source failed: {}", err)));
                    } else if real_task.state == TaskState::Pausing {
                        if old_thread_count == 1 {
                            // 2 thread all exit
                            new_task_state = Some(TaskState::Paused);
                        } else {
                            // wait work thread exit
                        }
                    }

                    if let Some(new_task_state) = new_task_state {
                        real_task.state = new_task_state;
                        engine_prepare.task_db.update_task(&real_task);
                    }
                }
            });
        }

        //start working thread
        let engine_work = engine.clone();
        let source_url_work = source_url.clone();
        let target_url_work = target_url.clone();

        let working_thread = tokio::spawn(async move {
            let working_result = BackupEngine::backup_work_thread(
                engine_work.clone(),
                source_work,
                target_work,
                backup_task_work.clone(),
                task_session_work,
            )
            .await;
            let old_thread_count =
                running_thread_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            let mut real_task = backup_task_work.lock().await;
            let mut new_task_state = None;
            let mut cleanup_task_id = None;
            match working_result {
                Ok(checkpoint_state) => {
                    if checkpoint_state == CheckPointState::Done {
                        new_task_state = Some(TaskState::Done);
                    } else if real_task.state != TaskState::Pausing {
                        // thread abort with uncomplete, it's failed and retry
                        new_task_state = Some(TaskState::Failed(
                            "Run failed and will retry later.".to_string(),
                        ));
                    } else if old_thread_count == 1 {
                        new_task_state = Some(TaskState::Paused);
                    }
                }
                Err(err) => {
                    error!("working thread error: {}", err);
                    if real_task.state != TaskState::Pausing {
                        new_task_state =
                            Some(TaskState::Failed(format!("Work failed with {}", err)));
                    } else if old_thread_count == 1 {
                        new_task_state = Some(TaskState::Paused);
                    }
                }
            }

            if let Some(new_task_state) = new_task_state {
                if matches!(new_task_state, TaskState::Done) {
                    cleanup_task_id = Some(real_task.taskid.clone());
                }
                real_task.state = new_task_state;
                engine_work.task_db.update_task(&real_task);
            }
            drop(real_task);

            if let Some(task_id) = cleanup_task_id {
                if let Err(err) = engine_work.cleanup_task_snapshot(&task_id).await {
                    warn!(
                        "cleanup snapshot for completed task {} failed: {}",
                        task_id, err
                    );
                }
            }
        });

        Ok(())
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
        let mut total_size = 0;
        let mut this_size = 0;
        let mut item_count: u64 = 0;
        loop {
            let (mut this_item_list, this_size, is_done) =
                source.prepare_items(checkpoint_id.as_str(), None).await?;
            engine
                .task_db
                .save_itemlist_to_checkpoint(checkpoint_id.as_str(), &this_item_list)?;
            total_size += this_size;
            item_count += this_item_list.len() as u64;
            if is_done {
                let mut real_checkpoint = checkpoint.lock().await;
                real_checkpoint.state = CheckPointState::Prepared;
                real_checkpoint.total_size = total_size;
                real_checkpoint.item_count = item_count;
                engine.task_db.set_checkpoint_ready(
                    checkpoint_id.as_str(),
                    total_size,
                    item_count,
                )?;
                info!(
                    "checkpoint {} set to ready, total_size: {}, item_count: {}",
                    checkpoint_id, total_size, item_count
                );
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
        match checkpoint_items_state {
            RemoteBackupCheckPointItemStatus::NotSupport => {
                return Ok(());
            }
            _ => {
                unimplemented!()
            }
        }
    }

    async fn pop_wait_backup_item(
        &self,
        checkpoint_id: &str,
    ) -> BackupResult<Option<BackupChunkItem>> {
        self.task_db
            .pop_wait_backup_item(checkpoint_id)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))
    }

    pub async fn backup_work_thread(
        engine: BackupEngine,
        source: BackupChunkSourceProvider,
        target: BackupChunkTargetProvider,
        backup_task: Arc<Mutex<WorkTask>>,
        task_session: Arc<Mutex<BackupTaskSession>>,
        //checkpoint: Arc<Mutex<BackupCheckPoint>>,
    ) -> BackupResult<CheckPointState> {
        let real_task = backup_task.lock().await;
        let checkpoint_id = real_task.checkpoint_id.clone();
        let task_id = real_task.taskid.clone();
        drop(real_task);

        let mut checkpoint_state = CheckPointState::New;
        info!("task {} transfer thread start", task_id);
        loop {
            let local_checkpoint = engine
                .task_db
                .load_checkpoint_by_id(checkpoint_id.as_str())?;
            checkpoint_state = local_checkpoint.state.clone();
            if !local_checkpoint.state.need_working() {
                info!(
                    "checkpoint {} is not need working, exit transfer thread",
                    checkpoint_id
                );
                break;
            }

            let real_task = backup_task.lock().await;
            if real_task.state != TaskState::Running {
                info!(
                    "backup task {} is not running, exit transfer thread,task_state: {:?}",
                    real_task.taskid, real_task.state
                );
                break;
            }
            drop(real_task);

            if local_checkpoint.state == CheckPointState::New {
                tokio::time::sleep(Duration::from_secs(1)).await;
                info!("checkpoint {} is new, sleep 1 second", checkpoint_id);
                continue;
            }

            let mut remote_checkpoint_state = CheckPointState::New;
            let (remote_checkpoint, checkpoint_items_state) = target
                .query_check_point_state(checkpoint_id.as_str())
                .await?;
            remote_checkpoint_state = remote_checkpoint.state.clone();
            engine
                .update_backup_item_state_by_remote_checkpoint_state(&checkpoint_items_state)
                .await?;
            match remote_checkpoint_state {
                CheckPointState::New => {
                    warn!(
                        "checkpoint {} remote state is new, need allocate checkpoint at remote",
                        checkpoint_id
                    );
                    let checkpoint = engine
                        .task_db
                        .load_checkpoint_by_id(checkpoint_id.as_str())?;
                    let checkpoint_items = engine
                        .task_db
                        .load_backup_chunk_items_by_checkpoint(
                            checkpoint_id.as_str(),
                            None,
                            None,
                            None,
                        )?;
                    let mut chunk_list = SimpleChunkList::new();
                    for item in checkpoint_items.iter() {
                        chunk_list
                            .append_chunk(item.chunk_id.clone())
                            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
                    }
                    //let target check there is enough free space to allocate checkpoint
                    let alloc_result = target.alloc_checkpoint(checkpoint_id.as_str(), &checkpoint, chunk_list).await;
                    if alloc_result.is_err() {
                        let err_string = alloc_result.err().unwrap().to_string();
                        warn!(
                            "allocate checkpoint {} at backup target error: {}",
                            checkpoint_id,
                            err_string.as_str()
                        );
                        checkpoint_state = CheckPointState::Failed(err_string);
                        engine
                            .update_backup_checkpoint(
                                checkpoint_id.as_str(),
                                checkpoint_state.clone(),
                                backup_task.clone(),
                            )
                            .await?;
                        break;
                    }

                    warn!("checkpoint {} allocated at backup target.", checkpoint_id);
                    continue;
                }
                CheckPointState::Prepared => {
                    error!("checkpoint {} remote state is prepared,but remote checkpint state NEVER be prepared. something wrong,exit working thread", checkpoint_id);
                    break;
                }
                CheckPointState::Done => {
                    warn!(
                        "checkpoint {} remote state is done, exit working thread",
                        checkpoint_id
                    );
                    checkpoint_state = CheckPointState::Done;
                    engine
                        .update_backup_checkpoint(
                            checkpoint_id.as_str(),
                            CheckPointState::Done,
                            backup_task.clone(),
                        )
                        .await?;
                    break;
                }
                CheckPointState::Failed(msg) => {
                    warn!(
                        "checkpoint {} remote state is failed: {}, exit working thread",
                        checkpoint_id,
                        msg.as_str()
                    );
                    checkpoint_state = CheckPointState::Failed(msg);
                    engine
                        .update_backup_checkpoint(
                            checkpoint_id.as_str(),
                            checkpoint_state.clone(),
                            backup_task.clone(),
                        )
                        .await?;
                    break;
                }
                CheckPointState::WaitTrans => {
                    //try put item list to target
                    warn!(
                        "checkpoint {} remote state is wait trans, wait remote 5 seconds",
                        checkpoint_id
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                CheckPointState::Working => {
                    let item = engine.pop_wait_backup_item(checkpoint_id.as_str()).await?;

                    if item.is_some() {
                        let item = item.unwrap();
                        let mut is_item_done = false;
                        let mut writer = target
                            .open_chunk_writer(checkpoint_id.as_str(), &item.chunk_id, item.size)
                            .await;
                        match writer {
                            Ok((mut writer, init_offset)) => {
                                let mut reader = source
                                    .open_item_chunk_reader(
                                        checkpoint_id.as_str(),
                                        &item,
                                        init_offset,
                                    )
                                    .await?;
                                // TODO: 并发执行?
                                let trans_bytes =
                                    tokio::io::copy(&mut reader, &mut writer)
                                        .await
                                        .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
                                debug!(
                                    "backup chunk {} bytes: {}",
                                    item.chunk_id.to_string(),
                                    trans_bytes
                                );
                                target
                                    .complete_chunk_writer(checkpoint_id.as_str(), &item.chunk_id)
                                    .await?;
                                is_item_done = true;
                            }
                            Err(e) => match e {
                                BuckyBackupError::TryLater(msg) => {
                                    warn!("open chunk writer error: {}, try later", msg);
                                    continue;
                                }
                                BuckyBackupError::AlreadyDone(msg) => {
                                    warn!(
                                        "chunk {} already exist, skip upload",
                                        item.chunk_id.to_string()
                                    );
                                    is_item_done = true;
                                }
                                _ => {
                                    warn!("open chunk writer error: {}", e.to_string());
                                    return Err(e);
                                }
                            },
                        }
                        if is_item_done {
                            engine
                                .complete_backup_item(
                                    checkpoint_id.as_str(),
                                    &item,
                                    backup_task.clone(),
                                )
                                .await?;
                        }
                    } else {
                        //no item to backup, check point completed
                        if checkpoint_items_state == RemoteBackupCheckPointItemStatus::NotSupport {
                            warn!("checkpoint {} remote state is not support checkpoint level check, complete backup checkpoint by all local items done.", checkpoint_id);
                            checkpoint_state = CheckPointState::Done;
                            engine
                                .update_backup_checkpoint(
                                    checkpoint_id.as_str(),
                                    CheckPointState::Done,
                                    backup_task.clone(),
                                )
                                .await?;
                            break;
                        }
                    }
                }
            }
        }

        Ok(checkpoint_state)
    }

    //return taskid
    pub async fn create_restore_task(
        &self,
        plan_id: &str,
        check_point_id: &str,
        restore_config: RestoreConfig,
    ) -> BackupResult<String> {
        let create_1_task_in_same_time = self.lock_create_task.lock().await;
        if self
            .is_restoring_task_dup(plan_id, check_point_id, &restore_config)
            .await
        {
            return Err(BuckyBackupError::Failed(format!(
                "plan {} already has a running backup task",
                plan_id
            )));
        }

        if restore_config.is_clean_restore {
            let dir_path = PathBuf::from(translate_local_path_from_url(
                &restore_config.restore_location_url,
            )?);
            fs::remove_dir_all(dir_path.as_path())
                .await
                .map_err(|err| {
                    BuckyBackupError::Failed(format!("Cliean target directory failed: {:?}", err))
                })?;
        }

        let checkpoint = self.task_db.load_checkpoint_by_id(check_point_id)?;
        let mut new_task = WorkTask::new(plan_id, check_point_id, TaskType::Restore);
        new_task.set_restore_config(restore_config);
        let new_task_id = new_task.taskid.clone();
        self.task_db.create_task(&new_task)?;
        info!("create new restore task: {:?}", new_task);
        let mut all_tasks = self.all_tasks.lock().await;
        all_tasks.insert(new_task_id.clone(), Arc::new(Mutex::new(new_task)));
        drop(create_1_task_in_same_time);
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

    // return Ok(is_done)
    async fn run_chunk2chunk_restore_task(
        &self,
        restore_task: Arc<Mutex<WorkTask>>,
        checkpoint_id: String,
        source: BackupChunkSourceProvider,
        target: BackupChunkTargetProvider,
    ) -> BackupResult<bool> {
        debug!("run_chunk2chunk_restore_task enter");
        let checkpoint = self
            .all_checkpoints
            .lock()
            .await
            .get(checkpoint_id.as_str())
            .cloned();
        let checkpoint = if let Some(checkpoint) = checkpoint {
            checkpoint
        } else {
            let checkpoint = self.task_db.load_checkpoint_by_id(checkpoint_id.as_str())?;
            let checkpoint = Arc::new(Mutex::new(checkpoint));
            self.all_checkpoints
                .lock()
                .await
                .entry(checkpoint_id.clone())
                .or_insert(checkpoint.clone());
            checkpoint
        };

        struct WaitFlushBuffer {
            item: BackupChunkItem,
            buf: Vec<u8>,
        }
        struct WaitFlushFileInfo {
            max_len: u64,
            wait_buffers: Vec<WaitFlushBuffer>,
        }

        let todo_what_is_target_id = "";
        let mut restore_cfg = {
            restore_task
                .lock()
                .await
                .restore_config
                .clone()
                .expect("no restore-config for restore task.")
        };

        if !restore_cfg.restore_location_url.starts_with("file://") {
            restore_cfg.restore_location_url =
                url::Url::from_file_path(restore_cfg.restore_location_url.as_str())
                    .unwrap()
                    .to_string();
        }
        let taskid = restore_task.lock().await.taskid.clone();
        let source = Arc::new(source);
        let target = Arc::new(target);
        let mut pending_tasks = vec![];
        let mut wait_flush_buffers: Arc<Mutex<HashMap<String, WaitFlushFileInfo>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let mut process_item_pos = 0;
        let mut is_all_items_load = false;
        let mut load_result = Ok(());
        let mut run_result = Ok(false);
        loop {
            loop {
                let load_limit = 16 - pending_tasks.len() as u64;
                {
                    let state = restore_task.lock().await.state.clone();
                    if is_all_items_load || load_limit < 4 || state != TaskState::Running {
                        debug!("run_chunk2chunk_restore_task break load for is_all_items_load = {}, load_limit = {}, state = {:?}", is_all_items_load, load_limit, state);
                        break;
                    }
                }
                load_result = Ok(());
                let standby_items = self.task_db.load_backup_chunk_items_by_checkpoint(
                    checkpoint_id.as_str(),
                    None,
                    Some(process_item_pos),
                    Some(load_limit),
                );
                let standby_items = match standby_items {
                    Err(err) => {
                        debug!(
                            "run_chunk2chunk_restore_task break load from {} limit {} failed for {:?}", process_item_pos, load_limit,
                            err
                        );
                        load_result = Err(err);
                        break;
                    }
                    Ok(t) => {
                        debug!("run_chunk2chunk_restore_task load count: {}", t.len());
                        if t.len() == 0 {
                            is_all_items_load = true;
                        } else {
                            process_item_pos = process_item_pos + t.len() as u64;
                        }
                        t
                    }
                };

                for standby_item in standby_items {
                    let file_path = ChunkInnerPathHelper::strip_chunk_suffix(&standby_item.item_id);
                    let source_clone = source.clone();
                    let target_clone = target.clone();
                    let restore_task_clone = restore_task.clone();
                    let engine_clone = self.clone();
                    let file_path_clone = file_path.clone();
                    let wait_flush_buffers_clone = wait_flush_buffers.clone();
                    let standby_item_clone = standby_item.clone();
                    let restore_cfg_clone = restore_cfg.clone();
                    debug!(
                        "run_chunk2chunk_restore_task chunk {} will transfer",
                        standby_item.item_id
                    );
                    let new_task = tokio::spawn(async move {
                        debug!(
                            "run_chunk2chunk_restore_task chunk {} tokio::spawn",
                            standby_item_clone.item_id
                        );
                        let mut buffer = vec![];
                        let is_buffer = {
                            let file_infos = wait_flush_buffers_clone.lock().await;
                            debug!(
                                "run_chunk2chunk_restore_task chunk {} locked for is_buffer",
                                standby_item_clone.item_id
                            );
                            match file_infos.get(&file_path_clone) {
                                Some(file_info) => file_info.max_len < standby_item_clone.offset,
                                None => false,
                            }
                        };
                        let file_writer = {
                            if is_buffer {
                                debug!("run_chunk2chunk_restore_task chunk {} will write to buffer for is_buffer", standby_item_clone.item_id);
                                None
                            } else {
                                // try file

                                debug!(
                                    "run_chunk2chunk_restore_task chunk {} will open writer",
                                    standby_item_clone.item_id
                                );
                                let source_writer = source_clone
                                    .open_writer_for_restore(
                                        todo_what_is_target_id,
                                        &standby_item_clone,
                                        &restore_cfg_clone,
                                        0,
                                    )
                                    .await;

                                debug!(
                                    "run_chunk2chunk_restore_task chunk {} open writer finish",
                                    standby_item_clone.item_id
                                );
                                match source_writer {
                                    Err(err) => match err {
                                        BuckyBackupError::AlreadyDone(_) => {
                                            debug!("run_chunk2chunk_restore_task chunk {} done for AlreadyDone", standby_item_clone.item_id);
                                            return Ok(None);
                                        }
                                        BuckyBackupError::TryLater(_) => {
                                            debug!("run_chunk2chunk_restore_task chunk {} will write to buffer", standby_item_clone.item_id);
                                            None
                                        }
                                        _ => {
                                            debug!("run_chunk2chunk_restore_task chunk {} open writer failed: {:?}", standby_item_clone.item_id, err);
                                            return Err(err);
                                        }
                                    },
                                    Ok((writer, _offset)) => Some(writer),
                                }
                            }
                        };

                        if file_writer.is_none() {
                            let mut file_infos = wait_flush_buffers_clone.lock().await;
                            let mut file_info =
                                file_infos.entry(file_path_clone).or_insert_with(|| {
                                    WaitFlushFileInfo {
                                        max_len: standby_item_clone.offset,
                                        wait_buffers: vec![],
                                    }
                                });
                            let pos = file_info
                                .wait_buffers
                                .binary_search_by(|buf| {
                                    buf.item.offset.cmp(&standby_item_clone.offset)
                                })
                                .expect_err("should not found");
                            file_info.wait_buffers.insert(
                                pos,
                                WaitFlushBuffer {
                                    item: standby_item_clone.clone(),
                                    buf: vec![],
                                },
                            );
                            if standby_item_clone.offset < file_info.max_len {
                                file_info.max_len = standby_item_clone.offset;
                            }
                        }

                        debug!(
                            "run_chunk2chunk_restore_task chunk {} will open reader",
                            standby_item_clone.item_id
                        );
                        let mut target_reader = target_clone
                            .open_chunk_reader_for_restore(&standby_item_clone.chunk_id, 0)
                            .await.map_err(|err| {
                                debug!("run_chunk2chunk_restore_task chunk {} open reader failed: {:?}", standby_item_clone.item_id, err);
                                err
                            })?;

                        debug!(
                            "run_chunk2chunk_restore_task chunk {} will copy chunk",
                            standby_item_clone.item_id
                        );

                        if let Some(writer) = file_writer {
                            debug!(
                                "run_chunk2chunk_restore_task chunk {} will copy chunk to file",
                                standby_item_clone.item_id
                            );
                            let copy_len = copy_chunk(
                                standby_item_clone.chunk_id,
                                target_reader,
                                writer,
                                None,
                                None,
                            )
                            .await
                            .map_err(|err| {
                                debug!("run_chunk2chunk_restore_task chunk {} copy chunk to file failed: {:?}", standby_item_clone.item_id, err);
                                BuckyBackupError::Failed(format!(
                                    "Copy chunk failed: {:?}",
                                    err
                                ))
                            })?;

                            {
                                debug!("run_chunk2chunk_restore_task chunk {} write to file success, {}/{}", standby_item_clone.item_id, copy_len, standby_item_clone.size);
                                // assert_eq!(copy_len, standby_item_clone.size);
                                let mut real_restore_task = restore_task_clone.lock().await;
                                real_restore_task.completed_size =
                                    real_restore_task.completed_size + standby_item_clone.size;
                                let _ignore_err =
                                    engine_clone.task_db.update_task(&real_restore_task);
                            }
                            Ok(None)
                        } else {
                            let buffer_writer = BufWriter::new(std::io::Cursor::new(&mut buffer));

                            debug!(
                                "run_chunk2chunk_restore_task chunk {} will copy chunk to buffer",
                                standby_item_clone.item_id
                            );
                            let copy_len = copy_chunk(
                                standby_item_clone.chunk_id,
                                target_reader,
                                buffer_writer,
                                None,
                                None,
                            )
                            .await
                            .map_err(|err| {
                                debug!("run_chunk2chunk_restore_task chunk {} copy chunk to buffer failed: {:?}", standby_item_clone.item_id, err);
                                BuckyBackupError::Failed(format!(
                                    "Copy chunk failed: {:?}",
                                    err
                                ))
                            })?;
                            debug!("run_chunk2chunk_restore_task chunk {} write to buffer success, {}/{}", standby_item_clone.item_id, copy_len, standby_item_clone.size);
                            assert_eq!(copy_len, standby_item_clone.size);
                            Ok(Some(buffer))
                        }
                    });

                    pending_tasks.push((new_task, file_path, standby_item));
                }

                debug!(
                    "run_chunk2chunk_restore_task load will break pending_tasks.len: {}",
                    pending_tasks.len()
                );
                break;
            }

            debug!(
                "run_chunk2chunk_restore_task pending_tasks.len: {}",
                pending_tasks.len()
            );

            if pending_tasks.len() > 0 {
                let (result, index, remain) =
                    select_all(pending_tasks.iter_mut().map(|t| &mut t.0)).await;
                debug!("run_chunk2chunk_restore_task select_all waked: {}", index);
                let finish_result =
                    result.expect(format!("select tasks[{}] failed", index).as_str());
                let (_finish_task, finish_path, finish_item) = pending_tasks.remove(index);
                debug!(
                    "run_chunk2chunk_restore_task waked task removed chunk {}",
                    finish_item.item_id
                );

                match finish_result {
                    Ok(buffer) => {
                        match buffer {
                            Some(buffer) => {
                                debug!(
                                    "run_chunk2chunk_restore_task chunk {} will find the buffer.",
                                    finish_item.item_id
                                );
                                let mut wait_flush_buffers_guard = wait_flush_buffers.lock().await;

                                debug!("run_chunk2chunk_restore_task chunk {} will find the buffer locked.", finish_item.item_id);
                                let wait_buffers = &mut wait_flush_buffers_guard
                                    .get_mut(&finish_path)
                                    .unwrap()
                                    .wait_buffers;

                                debug!("run_chunk2chunk_restore_task chunk {} will find the buffer find the file. target-offset: {}, all offset: {:?}", finish_item.item_id, finish_item.offset, wait_buffers.iter().map(|buf| buf.item.offset).collect::<Vec<_>>());
                                let pos = wait_buffers
                                    .binary_search_by(|buf: &WaitFlushBuffer| {
                                        buf.item.offset.cmp(&finish_item.offset)
                                    })
                                    .expect("should found");

                                debug!("run_chunk2chunk_restore_task chunk {} will find the buffer find the buffer.", finish_item.item_id);
                                let wait_buffer = wait_buffers.get_mut(pos).unwrap();
                                assert_eq!(wait_buffer.buf.len(), 0);
                                wait_buffer.buf = buffer;
                                debug!(
                                    "run_chunk2chunk_restore_task chunk {} finish to buffer",
                                    wait_buffer.item.item_id
                                );
                            }
                            None => {
                                let mut wait_flush_buffers_guard = wait_flush_buffers.lock().await;
                                let will_flush_buffers =
                                    wait_flush_buffers_guard.get_mut(&finish_path);
                                if let Some(will_flush_buffers) = will_flush_buffers {
                                    if finish_item.offset + finish_item.size
                                        > will_flush_buffers.max_len
                                    {
                                        will_flush_buffers.max_len =
                                            finish_item.offset + finish_item.size;
                                    }
                                }
                            }
                        }

                        // try flush buffers to file
                        loop {
                            let will_flush = {
                                let mut wait_flush_buffers_guard = wait_flush_buffers.lock().await;
                                match wait_flush_buffers_guard.get_mut(&finish_path) {
                                    Some(will_flush_file) => {
                                        will_flush_file.wait_buffers.get_mut(0).map(|buf| {
                                            let mut data = vec![];
                                            std::mem::swap(&mut buf.buf, &mut data);
                                            (buf.item.clone(), data)
                                        })
                                    }
                                    None => None,
                                }
                            };

                            match will_flush {
                                Some((will_flush_item, mut will_flush_buf)) => {
                                    debug!("run_chunk2chunk_restore_task chunk {} will flush buffer to file.", will_flush_item.item_id);
                                    if will_flush_item.size > 0 && will_flush_buf.len() == 0 {
                                        break;
                                    }

                                    assert_eq!(will_flush_buf.len(), will_flush_item.size as usize);
                                    let writer = source
                                        .open_writer_for_restore(
                                            todo_what_is_target_id,
                                            &will_flush_item,
                                            &restore_cfg,
                                            0,
                                        )
                                        .await;
                                    let (result, is_continue) = match writer {
                                        Ok((mut writer, _offset)) => {
                                            let result = 
                                            writer
                                                .write_all(will_flush_buf.as_slice())
                                                .await
                                                .map_err(|err| {
                                                    debug!("run_chunk2chunk_restore_task chunk {} flush to file failed: {:?}", will_flush_item.item_id, err);
                                                    BuckyBackupError::Failed(format!(
                                                        "Flush to file failed: {:?}",
                                                        err
                                                    ))
                                                });
                                            if result.is_ok() {
                                                debug!("run_chunk2chunk_restore_task chunk {} flush to file success", will_flush_item.item_id);
                                                let mut real_restore_task =
                                                    restore_task.lock().await;
                                                real_restore_task.completed_size = real_restore_task
                                                    .completed_size
                                                    + will_flush_item.size;
                                                let _ignore_err =
                                                    self.task_db.update_task(&real_restore_task);
                                            }
                                            (result, true)
                                        }
                                        Err(err) => match err {
                                            BuckyBackupError::AlreadyDone(_) => {
                                                debug!("run_chunk2chunk_restore_task chunk {} flush to file done for AlreadyDone.", will_flush_item.item_id);
                                                (Ok(()), true)
                                            }
                                            BuckyBackupError::TryLater(_) => {
                                                debug!("run_chunk2chunk_restore_task chunk {} flush to file later.", will_flush_item.item_id);
                                                (Ok(()), false)
                                            }
                                            _ => {
                                                debug!("run_chunk2chunk_restore_task {} flush to file failed. {:?}", will_flush_item.item_id, err);
                                                (Err(err), false)
                                            }
                                        },
                                    };

                                    match result {
                                        Ok(_) => {
                                            let mut wait_flush_buffers_guard =
                                                wait_flush_buffers.lock().await;

                                            let mut file_buffers =
                                                wait_flush_buffers_guard.get_mut(&finish_path);
                                            file_buffers.as_mut().and_then(|file_buffers| {
                                                if will_flush_item.offset + will_flush_item.size
                                                    > file_buffers.max_len
                                                {
                                                    file_buffers.max_len = will_flush_item.offset
                                                        + will_flush_item.size;
                                                }
                                                Some(0)
                                            });
                                            if !is_continue {
                                                // retry next
                                                file_buffers.and_then(|f| {
                                                    f.wait_buffers
                                                        .iter_mut()
                                                        .find(|buf| {
                                                            buf.item.offset
                                                                == will_flush_item.offset
                                                        })
                                                        .and_then(|buf| {
                                                            std::mem::swap(
                                                                &mut buf.buf,
                                                                &mut will_flush_buf,
                                                            );
                                                            Some(0)
                                                        })
                                                });
                                                break;
                                            } else {
                                                // remove it
                                                let is_empty =
                                                    file_buffers.and_then(|file_buffers| {
                                                        let pos = file_buffers
                                                            .wait_buffers
                                                            .iter()
                                                            .position(|buf| {
                                                                buf.item.offset
                                                                    == will_flush_item.offset
                                                            });

                                                        if let Some(pos) = pos {
                                                            file_buffers.wait_buffers.remove(pos);
                                                            if file_buffers.wait_buffers.len() == 0
                                                            {
                                                                return Some(0);
                                                            }
                                                        }
                                                        None
                                                    });
                                                if is_empty.is_some() {
                                                    wait_flush_buffers_guard.remove(&finish_path);
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            debug!(
                                                "run_chunk2chunk_restore_task failed: {:?}",
                                                err
                                            );
                                            run_result = run_result.and(Err(err));
                                            break;
                                        }
                                    }
                                }
                                None => break,
                            }
                        }
                    }
                    Err(err) => {
                        debug!("run_chunk2chunk_restore_task failed: {:?}", err);
                        run_result = run_result.and(Err(err));
                    }
                }
            }

            if pending_tasks.len() == 0 {
                if is_all_items_load {
                    debug!("run_chunk2chunk_restore_task task {} done", taskid);
                    return run_result.and(Ok(true));
                } else {
                    run_result = run_result
                        .and(load_result.map_err(|err| {
                            BuckyBackupError::Failed(format!("Read items failed {}", err))
                        }))
                        .and(Ok(false));
                    debug!(
                        "run_chunk2chunk_restore_task task {} end with {:?}",
                        taskid, run_result
                    );
                    return run_result;
                }
            } else {
                debug!(
                    "run_chunk2chunk_restore_task {} will try load more chunks",
                    restore_task.lock().await.taskid
                );
            }
        }
    }

    async fn run_dir2chunk_restore_task(
        &self,
        plan_id: &str,
        check_point_id: &str,
    ) -> BackupResult<()> {
        unimplemented!()
    }

    async fn run_dir2dir_restore_task(
        &self,
        plan_id: &str,
        check_point_id: &str,
    ) -> BackupResult<()> {
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
            return Err(BuckyBackupError::NotFound(format!(
                "create chunk backup source failed, unsupported source url scheme: {}",
                url.scheme()
            )));
        }
        let (_desc, create_func) = create_func.unwrap();
        let mut local_path = url.path();
        let mut create_func = create_func.lock().await;
        let result = create_func(local_path.to_string()).await?;
        Ok(result)
    }

    async fn get_chunk_target_provider(
        &self,
        target_url: &str,
    ) -> BackupResult<BackupChunkTargetProvider> {
        let url = Url::parse(target_url).map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        let mut all_chunk_target_providers = self.all_chunk_target_providers.lock().await;
        let create_func = all_chunk_target_providers.get(url.scheme());
        if create_func.is_none() {
            return Err(BuckyBackupError::NotFound(format!(
                "create chunk backup target failed, unsupported target url scheme: {}",
                url.scheme()
            )));
        }
        let (_desc, create_func) = create_func.unwrap();
        let mut local_path = url.path();
        let mut create_func = create_func.lock().await;
        let result = create_func(local_path.to_string()).await?;
        Ok(result)
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

    pub async fn get_target_record(&self, target_id: &str) -> BackupResult<BackupTargetRecord> {
        if let Some(cached) = {
            let all_targets = self.all_targets.lock().await;
            all_targets.get(target_id).cloned()
        } {
            let record = cached.lock().await.clone();
            return Ok(record);
        }

        match self.task_db.get_backup_target(target_id) {
            Ok(record) => {
                let mut all_targets = self.all_targets.lock().await;
                all_targets.insert(target_id.to_string(), Arc::new(Mutex::new(record.clone())));
                Ok(record)
            }
            Err(BackupDbError::NotFound(_)) => {
                if let Ok(parsed) = Url::parse(target_id) {
                    warn!(
                        "target {} not found in database, using fallback from url",
                        target_id
                    );
                    let fallback = BackupTargetRecord::new(
                        target_id.to_string(),
                        parsed.scheme(),
                        target_id,
                        target_id,
                        Some("legacy target"),
                        None,
                    );
                    let mut all_targets = self.all_targets.lock().await;
                    all_targets.insert(
                        fallback.target_id.clone(),
                        Arc::new(Mutex::new(fallback.clone())),
                    );
                    Ok(fallback)
                } else {
                    Err(BuckyBackupError::NotFound(format!(
                        "target {} not found",
                        target_id
                    )))
                }
            }
            Err(err) => Err(BuckyBackupError::Failed(err.to_string())),
        }
    }

    async fn get_chunk_target_provider_by_id(
        &self,
        target_id: &str,
    ) -> BackupResult<BackupChunkTargetProvider> {
        let record = self.get_target_record(target_id).await?;
        let url = if record.target_type == "file" {
            if record.url.starts_with("file://") {
                record.url
            } else {
                let file_url = url::Url::from_file_path(record.url.as_str()).unwrap();
                file_url.to_string()
            }
        } else {
            record.url
        };
        self.get_chunk_target_provider(url.as_str()).await
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

        let real_restore_task = restore_task.lock().await;
        if real_restore_task.task_type != TaskType::Restore {
            error!("try resume a BackupTask as Restore.");
            return Err(BuckyBackupError::Failed(
                "try resume a BackupTask as Restore".to_string(),
            ));
        }
        if !real_restore_task.state.is_resumable() {
            warn!("restore task is running, ignore resume");
            return Err(BuckyBackupError::Failed(
                "restore task is running".to_string(),
            ));
        }
        let task_id = real_restore_task.taskid.clone();
        let checkpoint_id = real_restore_task.checkpoint_id.clone();
        let owner_plan_id = real_restore_task.owner_plan_id.clone();
        drop(real_restore_task);

        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(&owner_plan_id);
        if plan.is_none() {
            error!(
                "task plan not found: {} plan_id: {}",
                taskid,
                owner_plan_id.as_str()
            );
            return Err(BuckyBackupError::NotFound(
                "task plan not found".to_string(),
            ));
        }
        let plan = plan.unwrap().lock().await;
        let task_type = plan.type_str.clone();
        let source_provider = self
            .get_chunk_source_provider(plan.source.get_source_url())
            .await?;
        let target_provider = self
            .get_chunk_target_provider_by_id(plan.target.as_str())
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

        let mut real_restore_task = restore_task.lock().await;
        real_restore_task.state = TaskState::Running;
        let _ignore_err = self.task_db.update_task(&real_restore_task);
        drop(real_restore_task);

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
                _ => Err(BuckyBackupError::Failed(format!(
                    "unknown plan type: {}",
                    task_type
                ))),
            };

            let mut real_restore_task = restore_task.lock().await;
            let new_task_state = match task_result {
                Ok(is_done) => {
                    if is_done {
                        info!("restore task done: {} ", taskid.as_str());
                        TaskState::Done
                    } else {
                        info!("restore task paused: {} ", taskid.as_str());
                        TaskState::Paused
                    }
                }
                Err(err) => {
                    if real_restore_task.state == TaskState::Pausing {
                        info!("restore task paused: {} ", taskid.as_str());
                        TaskState::Paused
                    } else {
                        info!("restore task failed: {} {}", taskid.as_str(), err);
                        if let BuckyBackupError::Failed(msg) = err {
                            TaskState::Failed(msg)
                        } else {
                            TaskState::Failed(format!("Task failed: {}", err))
                        }
                    }
                }
            };
            real_restore_task.state = new_task_state;
            let _ignore_err = engine.task_db.update_task(&real_restore_task);
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
            return Err(BuckyBackupError::Failed(
                "try resume a RestoreTask as Backup".to_string(),
            ));
        }
        let task_id = real_backup_task.taskid.clone();
        let checkpoint_id = real_backup_task.checkpoint_id.clone();
        let owner_plan_id = real_backup_task.owner_plan_id.clone();
        drop(real_backup_task);

        let all_plans = self.all_plans.lock().await;
        let plan = all_plans.get(&owner_plan_id);
        if plan.is_none() {
            error!(
                "task plan not found: {} plan_id: {}",
                taskid,
                owner_plan_id.as_str()
            );
            return Err(BuckyBackupError::NotFound(
                "task plan not found".to_string(),
            ));
        }
        let plan_arc = plan.unwrap();
        let plan_snapshot = plan_arc.lock().await.clone();
        drop(all_plans);

        let snapshot_source_url = self
            .prepare_local_snapshot_if_needed(&owner_plan_id, &task_id, &plan_snapshot)
            .await?;
        let effective_source_url = snapshot_source_url
            .as_deref()
            .unwrap_or(plan_snapshot.source.get_source_url());
        let task_type = plan_snapshot.type_str.clone();
        let source_provider = self
            .get_chunk_source_provider(effective_source_url)
            .await?;
        let target_provider = self
            .get_chunk_target_provider_by_id(plan_snapshot.target.as_str())
            .await?;

        let mut real_backup_task = backup_task.lock().await;
        if !real_backup_task.state.is_resumable() {
            warn!("task is not paused, ignore resume");
            return Err(BuckyBackupError::Failed("task is not paused".to_string()));
        }
        real_backup_task.state = TaskState::Running;
        self.task_db.update_task(&real_backup_task);
        drop(real_backup_task);

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
                _ => Err(BuckyBackupError::Failed(format!(
                    "unknown plan type: {}",
                    task_type
                ))),
            };

            if let Err(err) = task_result {
                error!("start backup task failed: {}", err);
                // task failed
                let mut real_task = backup_task.lock().await;
                let mut new_task_state = None;
                if real_task.state == TaskState::Running {
                    new_task_state =
                        Some(TaskState::Failed(format!("Prepare source failed: {}", err)));
                } else if real_task.state == TaskState::Pausing {
                    new_task_state = Some(TaskState::Paused);
                }

                if let Some(new_task_state) = new_task_state {
                    real_task.state = new_task_state;
                    engine.task_db.update_task(&real_task);
                }
            }
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

        let concurrency_limit = self.task_concurrency_limit();
        if concurrency_limit > 0 {
            let running_snapshots: Vec<_> = {
                let all_tasks = self.all_tasks.lock().await;
                all_tasks.values().cloned().collect()
            };

            let mut running_count = 0usize;
            for task_arc in running_snapshots {
                let task = task_arc.lock().await;
                if task.state == TaskState::Running {
                    running_count += 1;
                }
            }

            if running_count >= concurrency_limit {
                let mut real_backup_task = backup_task.lock().await;
                if matches!(
                    real_backup_task.state,
                    TaskState::Paused | TaskState::Failed(_)
                ) {
                    real_backup_task.state = TaskState::Pending;
                }
                drop(real_backup_task);
                self.task_db
                    .update_task_state(taskid, &TaskState::Pending)?;
                return Ok(());
            }
        }

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
        if !backup_task.state.is_puasable() {
            warn!("task is not running, ignore pause");
            return Err(BuckyBackupError::Failed("task is not running".to_string()));
        }
        backup_task.state = TaskState::Pausing;
        self.task_db.update_task(&backup_task)?;
        Ok(())
    }

    pub async fn remove_work_task(&self, taskid: &str) -> BackupResult<()> {
        let mut task_info = self.get_task_info(taskid).await?;
        match task_info.state {
            TaskState::Running | TaskState::Pending => {
                self.pause_work_task(taskid).await?;
            }
            _ => {}
        }

        if matches!(
            task_info.state,
            TaskState::Running | TaskState::Pending | TaskState::Pausing
        ) {
            task_info = self.wait_task_to_stop(taskid).await?;
        }

        if task_info.state == TaskState::Remove {
            return Ok(());
        }

        self.set_task_state_persisted(taskid, TaskState::Remove)
            .await?;

        if let Err(err) = self.cleanup_removed_tasks().await {
            warn!("cleanup removed tasks failed: {}", err);
        }

        Ok(())
    }

    async fn wait_task_to_stop(&self, taskid: &str) -> BackupResult<WorkTask> {
        for _ in 0..REMOVE_WAIT_ATTEMPTS {
            let task = self.get_task_info(taskid).await?;
            match task.state {
                TaskState::Running | TaskState::Pending | TaskState::Pausing => {
                    tokio::time::sleep(Duration::from_millis(REMOVE_WAIT_INTERVAL_MS)).await;
                }
                _ => return Ok(task),
            }
        }
        Err(BuckyBackupError::Failed(format!(
            "timeout waiting task {} to pause",
            taskid
        )))
    }

    async fn set_task_state_persisted(
        &self,
        taskid: &str,
        new_state: TaskState,
    ) -> BackupResult<()> {
        let task_arc = {
            let all_tasks = self.all_tasks.lock().await;
            all_tasks.get(taskid).cloned()
        };

        if let Some(task_arc) = task_arc {
            let mut task = task_arc.lock().await;
            task.state = new_state.clone();
        }

        self.task_db.update_task_state(taskid, &new_state)?;
        Ok(())
    }

    async fn cleanup_removed_tasks(&self) -> BackupResult<()> {
        let _cleanup_guard = self.cleanup_removed_task_lock.lock().await;
        let tasks = self
            .task_db
            .list_tasks_by_state(&TaskState::Remove)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;

        for task in tasks {
            match task.task_type {
                TaskType::Backup => {
                    if let Err(err) = self.cleanup_backup_task_resources(&task).await {
                        warn!("cleanup backup task {} failed: {}", task.taskid, err);
                    }
                }
                TaskType::Restore => {
                    self.remove_task_from_memory(&task.taskid, &task.checkpoint_id)
                        .await;
                }
            }
        }

        Ok(())
    }

    async fn cleanup_backup_task_resources(&self, task: &WorkTask) -> BackupResult<()> {
        let plan = self.get_backup_plan(task.owner_plan_id.as_str()).await?;
        let target_provider = self
            .get_chunk_target_provider_by_id(plan.target.as_str())
            .await?;

        target_provider
            .remove_checkpoint(task.checkpoint_id.as_str())
            .await?;

        self.task_db
            .delete_backup_items_by_checkpoint(task.checkpoint_id.as_str())?;
        self.task_db
            .delete_checkpoint(task.checkpoint_id.as_str())?;
        self.task_db.delete_worktask_logs(task.taskid.as_str())?;
        self.task_db.delete_work_task(task.taskid.as_str())?;
        self.remove_task_from_memory(&task.taskid, &task.checkpoint_id)
            .await;
        if let Err(err) = self.cleanup_task_snapshot(task.taskid.as_str()).await {
            warn!("cleanup snapshot for task {} failed: {}", task.taskid, err);
        }
        Ok(())
    }

    async fn cleanup_task_snapshot(&self, task_id: &str) -> BackupResult<()> {
        let maybe_snapshot = self
            .task_db
            .get_task_snapshot(task_id)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        let Some(record) = maybe_snapshot else {
            return Ok(());
        };

        let snapshot_path = PathBuf::from(&record.snapshot_path);
        match remove_snapshot_dir(&snapshot_path).await {
            Ok(_) => {}
            Err(err) => {
                warn!(
                    "failed to remove snapshot directory {}: {}",
                    snapshot_path.display(),
                    err
                );
            }
        }

        self.task_db
            .delete_task_snapshot(task_id)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        Ok(())
    }

    async fn prepare_local_snapshot_if_needed(
        &self,
        plan_id: &str,
        task_id: &str,
        plan: &BackupPlanConfig,
    ) -> BackupResult<Option<String>> {
        // if !matches!(plan.source, BackupSource::Directory(_)) {
        //     return Ok(None);
        // }

        let source_path = resolve_plan_source_path(plan_id, plan).ok_or_else(|| {
            BuckyBackupError::Failed(format!(
                "plan {} uses unsupported source path for snapshot",
                plan_id
            ))
        })?;

        if let Some(snapshot) = self
            .task_db
            .get_task_snapshot(task_id)
            .map_err(|e| BuckyBackupError::Failed(e.to_string()))?
        {
            let existing_path = PathBuf::from(&snapshot.snapshot_path);
            let file_url = Url::from_file_path(&existing_path).map_err(|_| {
                BuckyBackupError::Failed(format!(
                    "failed to convert snapshot path {} to url",
                    existing_path.display()
                ))
            })?;
            return Ok(Some(file_url.to_string()));
        }

        let snapshot_info = snapshot::create_snapshot(plan_id, task_id, &source_path).await?;
        let file_url = Url::from_file_path(&snapshot_info.final_path).map_err(|_| {
            BuckyBackupError::Failed(format!(
                "failed to convert snapshot path {} to url",
                snapshot_info.final_path.display()
            ))
        })?;

        let record = TaskSnapshotRecord {
            task_id: task_id.to_string(),
            plan_id: plan_id.to_string(),
            source_url: plan.source.get_source_url().to_string(),
            snapshot_path: snapshot_info
                .final_path
                .to_string_lossy()
                .to_string(),
            create_time: 0,
            update_time: 0,
        };

        if let Err(err) = self.task_db.upsert_task_snapshot(&record) {
            warn!(
                "failed to persist snapshot metadata for task {}, cleaning up snapshot: {}",
                task_id, err
            );
            let _ = snapshot::remove_snapshot_dir(&snapshot_info.final_path).await;
            return Err(BuckyBackupError::Failed(err.to_string()));
        }

        Ok(Some(file_url.to_string()))
    }

    async fn remove_task_from_memory(&self, taskid: &str, checkpoint_id: &str) {
        {
            let mut all_tasks = self.all_tasks.lock().await;
            all_tasks.remove(taskid);
        }
        {
            let mut all_checkpoints = self.all_checkpoints.lock().await;
            all_checkpoints.remove(checkpoint_id);
        }
    }

    pub async fn cancel_backup_task(&self, taskid: &str) -> BackupResult<()> {
        unimplemented!()
    }
}

fn parse_plan_policies(plan_id: &str, policy_value: &Value) -> Vec<PlanPolicyRule> {
    let raw = match policy_value {
        Value::Array(_) => policy_value.clone(),
        _ => {
            debug!(
                "plan {} has policy in unexpected format (expect array): {}",
                plan_id, policy_value
            );
            return vec![];
        }
    };

    let parsed: Vec<PlanPolicyRule> = match serde_json::from_value(raw) {
        Ok(policies) => policies,
        Err(err) => {
            warn!("failed to parse policy for plan {}: {}", plan_id, err);
            vec![]
        }
    };

    parsed
        .into_iter()
        .filter(|rule| match rule {
            PlanPolicyRule::Period(period) => {
                if period.minutes >= 24 * 60 {
                    debug!(
                        "ignore invalid period policy (minutes >= 1440) for plan {}",
                        plan_id
                    );
                    false
                } else {
                    true
                }
            }
            PlanPolicyRule::Event(event) => {
                if event.update_delay == 0 {
                    debug!(
                        "ignore event policy without positive update_delay for plan {}",
                        plan_id
                    );
                    false
                } else {
                    true
                }
            }
        })
        .collect()
}

fn policies_contain_event(policies: &[PlanPolicyRule]) -> bool {
    policies
        .iter()
        .any(|policy| matches!(policy, PlanPolicyRule::Event(_)))
}

fn latest_period_due_time(policies: &[PlanPolicyRule], now_local: DateTime<Local>) -> Option<u64> {
    policies
        .iter()
        .filter_map(|policy| match policy {
            PlanPolicyRule::Period(period) => latest_period_occurrence(period, now_local),
            _ => None,
        })
        .max()
}

fn latest_period_occurrence(period: &PlanPolicyPeriod, now_local: DateTime<Local>) -> Option<u64> {
    if period.minutes >= 24 * 60 {
        return None;
    }
    let hour = (period.minutes / 60) as u32;
    let minute = (period.minutes % 60) as u32;

    if let Some(date) = period.date {
        return latest_monthly_occurrence(date, hour, minute, now_local);
    }

    if let Some(week) = period.week {
        return latest_weekly_occurrence(week, hour, minute, now_local);
    }

    latest_daily_occurrence(hour, minute, now_local)
}

fn latest_daily_occurrence(hour: u32, minute: u32, now_local: DateTime<Local>) -> Option<u64> {
    let today = now_local.date_naive();
    let mut candidate = combine_date_time(today, hour, minute)?;
    if candidate > now_local {
        let yesterday = today - ChronoDuration::days(1);
        candidate = combine_date_time(yesterday, hour, minute)?;
    }
    Some(datetime_to_timestamp(candidate))
}

fn latest_weekly_occurrence(
    week: u32,
    hour: u32,
    minute: u32,
    now_local: DateTime<Local>,
) -> Option<u64> {
    let target_weekday = policy_weekday(week)?;
    let today = now_local.date_naive();
    let current_weekday = now_local.weekday();
    let mut diff = current_weekday.num_days_from_monday() as i32
        - target_weekday.num_days_from_monday() as i32;
    if diff < 0 {
        diff += 7;
    }
    let mut target_date = today - ChronoDuration::days(diff as i64);
    let mut candidate = combine_date_time(target_date, hour, minute)?;
    if candidate > now_local {
        target_date = target_date - ChronoDuration::days(7);
        candidate = combine_date_time(target_date, hour, minute)?;
    }
    Some(datetime_to_timestamp(candidate))
}

fn latest_monthly_occurrence(
    date: u32,
    hour: u32,
    minute: u32,
    now_local: DateTime<Local>,
) -> Option<u64> {
    if date == 0 {
        return None;
    }
    let today = now_local.date_naive();
    let mut year = today.year();
    let mut month = today.month();
    let mut candidate = build_monthly_datetime(year, month, date, hour, minute)?;
    if candidate > now_local {
        if month == 1 {
            year -= 1;
            month = 12;
        } else {
            month -= 1;
        }
        candidate = build_monthly_datetime(year, month, date, hour, minute)?;
    }
    Some(datetime_to_timestamp(candidate))
}

fn build_monthly_datetime(
    year: i32,
    month: u32,
    date: u32,
    hour: u32,
    minute: u32,
) -> Option<DateTime<Local>> {
    let clamped_day = clamp_day(year, month, date);
    let naive = NaiveDate::from_ymd_opt(year, month, clamped_day)?;
    combine_date_time(naive, hour, minute)
}

fn combine_date_time(date: NaiveDate, hour: u32, minute: u32) -> Option<DateTime<Local>> {
    match Local.with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0) {
        LocalResult::Single(dt) => Some(dt),
        _ => None,
    }
}

fn clamp_day(year: i32, month: u32, desired_day: u32) -> u32 {
    if desired_day == 0 {
        return 1;
    }
    let last_day = last_day_of_month(year, month);
    desired_day.min(last_day)
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("invalid date while computing last day of month");
    let last = first_next - ChronoDuration::days(1);
    last.day()
}

fn datetime_to_timestamp(dt: DateTime<Local>) -> u64 {
    let utc_time = dt.with_timezone(&Utc).timestamp_millis();
    if utc_time <= 0 {
        0
    } else {
        utc_time as u64
    }
}

fn policy_weekday(week: u32) -> Option<Weekday> {
    match week {
        1 => Some(Weekday::Mon),
        2 => Some(Weekday::Tue),
        3 => Some(Weekday::Wed),
        4 => Some(Weekday::Thu),
        5 => Some(Weekday::Fri),
        6 => Some(Weekday::Sat),
        7 => Some(Weekday::Sun),
        0 => Some(Weekday::Sun),
        _ => None,
    }
}

fn is_event_policy_due(
    last_completed: Option<u64>,
    now_ms: u64,
    event: &PlanPolicyEvent,
    baseline_ms: u64,
    source_update: Option<u64>,
) -> bool {
    let latest_update = match source_update {
        Some(ts) => ts,
        None => return false,
    };

    let last_backup = last_completed.unwrap_or(baseline_ms);
    if latest_update <= last_backup {
        return false;
    }

    let delay_ms = event.update_delay.saturating_mul(1000);
    now_ms >= latest_update.saturating_add(delay_ms)
}

fn resolve_plan_source_path(plan_id: &str, plan: &BackupPlanConfig) -> Option<PathBuf> {
    let source_url = plan.source.get_source_url();
    let raw_path = match translate_local_path_from_url(source_url) {
        Ok(path) => path,
        Err(err) => {
            warn!(
                "plan {} failed to translate source url {}: {}",
                plan_id, source_url, err
            );
            return None;
        }
    };

    let mut path = PathBuf::from(raw_path);
    if !path.is_absolute() {
        if let Ok(cwd) = env::current_dir() {
            path = cwd.join(path);
        }
    }
    Some(path)
}

//impl kRPC for BackupEngine

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_c2c_backup_task() {
        //std::env::set_var("BUCKY_LOG", "debug");
        buckyos_kit::init_logging("bucky_backup_test", false);
        let tempdb = "/opt/buckyos/data/backup_suite/bucky_backup.db";
        //delete db file if exists
        if std::path::Path::new(tempdb).exists() {
            std::fs::remove_file(tempdb).unwrap();
        }

        let engine = BackupEngine::new();
        engine.start().await.unwrap();
        let target_id = "test_target";
        let target_record = BackupTargetRecord::new(
            target_id.to_string(),
            "file",
            "/tmp/bucky_backup_result",
            "test target",
            Some("test target for unit"),
            None,
        );
        engine.task_db.create_backup_target(&target_record).unwrap();
        let new_plan = BackupPlanConfig::chunk2chunk(
            "file:///Users/liuzhicong/Downloads",
            target_id,
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
        buckyos_kit::init_logging("bucky_backup_test", false);

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
