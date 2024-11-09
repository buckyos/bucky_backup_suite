use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use base58::ToBase58;
use tokio::sync::RwLock;

use crate::{
    checkpoint::{CheckPoint, CheckPointInfo, CheckPointStatus, PendingStatus},
    checkpoint_impl::{CheckPointImpl, CheckPointWrapper},
    engine::{
        Config, EngineConfig, FindTaskBy, ListOffset, ListSourceFilter, ListTargetFilter,
        ListTaskFilter, SourceId, SourceInfo, SourceMgr, SourceQueryBy, TargetId, TargetInfo,
        TargetMgr, TargetQueryBy, TaskMgr, TaskUuid,
    },
    error::{BackupError, BackupResult},
    handle_error,
    meta::{CheckPointVersion, LockedSourceStateId},
    source::{LockedSource, Source, SourceFactory, SourceTask},
    source_wrapper::{LockedSourceWrapper, SourceTaskWrapper, SourceWrapper},
    storage::{ListLockedSourceStateFilter, Storage, StorageSourceMgr, StorageTargetMgr},
    target::{Target, TargetCheckPoint, TargetFactory, TargetTask},
    target_wrapper::{TargetCheckPointWrapper, TargetTaskWrapper, TargetWrapper},
    task::{HistoryStrategy, ListCheckPointFilter, SourceState, SourceStateInfo, Task, TaskInfo},
    task_impl::{TaskImpl, TaskWrapper},
};

struct SourceTaskCache {
    source_task: Arc<Box<dyn SourceTask>>,
    source_lockeds: HashMap<LockedSourceStateId, Arc<Box<dyn LockedSource>>>,
}

struct SourceCache {
    source: Arc<Box<dyn Source>>,
    source_tasks: HashMap<TaskUuid, SourceTaskCache>,
}

struct TargetTaskCache {
    target_task: Arc<Box<dyn TargetTask>>,
    target_checkpoints: HashMap<CheckPointVersion, Arc<Box<dyn TargetCheckPoint>>>,
}

struct TargetCache {
    target: Arc<Box<dyn Target>>,
    target_tasks: HashMap<TaskUuid, TargetTaskCache>,
}

struct TaskCache {
    task: Arc<TaskImpl>,
    checkpoints: HashMap<CheckPointVersion, Arc<CheckPointImpl>>,
}

#[derive(Clone)]
pub struct Engine {
    meta_storage: Arc<Box<dyn Storage>>,
    source_factory: Arc<Box<dyn SourceFactory>>,
    target_factory: Arc<Box<dyn TargetFactory>>,
    sources: Arc<RwLock<HashMap<SourceId, SourceCache>>>,
    targets: Arc<RwLock<HashMap<TargetId, TargetCache>>>,
    config: Arc<RwLock<Option<EngineConfig>>>,
    tasks: Arc<RwLock<HashMap<TaskUuid, TaskCache>>>,
    process_magic: u64,
}

impl Engine {
    pub fn new(
        meta_storage: Box<dyn Storage>,
        source_factory: Box<dyn SourceFactory>,
        target_factory: Box<dyn TargetFactory>,
    ) -> Self {
        let magic: u32 = rand::random();
        Self {
            meta_storage: Arc::new(meta_storage),
            source_factory: Arc::new(source_factory),
            target_factory: Arc::new(target_factory),
            sources: Arc::new(RwLock::new(HashMap::new())),
            targets: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(None)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            process_magic: ((std::process::id() as u64) << 32) + magic,
        }
    }

    pub(crate) fn process_magic(&self) -> u64 {
        self.process_magic
    }

    // should never call this function directly except `TaskWrapper`, use `Self::get_task` instead.
    pub(crate) async fn get_task_impl(
        &self,
        by: &FindTaskBy,
    ) -> BackupResult<Option<Arc<TaskImpl>>> {
        {
            let cache = self.tasks.read().await;
            let task = match by {
                FindTaskBy::Uuid(uuid) => cache.get(uuid),
            };

            if let Some(task) = task {
                return Ok(Some(task.task.clone()));
            }
        }

        let task_info = self
            .meta_storage
            .query_task(by)
            .await
            .map_err(handle_error!("find task failed, by: {:?}", by))?;

        if let Some(task_info) = task_info {
            let task = self
                .tasks
                .write()
                .await
                .entry(task_info.uuid)
                .or_insert_with(|| TaskCache {
                    task: Arc::new(TaskImpl::new(task_info, self.clone())),
                    checkpoints: HashMap::new(),
                })
                .task
                .clone();
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn get_task(&self, by: &FindTaskBy) -> BackupResult<Option<Arc<TaskWrapper>>> {
        self.get_task_impl(by)
            .await
            .map(|t| t.map(|t| Arc::new(TaskWrapper::new(self.clone(), *t.uuid()))))
    }

    // should never call this function directly except `SourceWrapper`, use `Self::get_source` instead.
    pub(crate) async fn get_source_impl(
        &self,
        by: &SourceQueryBy,
    ) -> BackupResult<Option<Arc<Box<dyn Source>>>> {
        {
            let cache = self.sources.read().await;
            match by {
                SourceQueryBy::Id(id) => {
                    if let Some(s) = cache.get(id) {
                        return Ok(Some(s.source.clone()));
                    }
                }
                SourceQueryBy::Url(url) => {
                    for (id, s) in cache.iter() {
                        if let Ok(u) = s.source.source_info().await {
                            if u.url == *url {
                                return Ok(Some(s.source.clone()));
                            }
                        }
                    }
                }
            }
        }

        let source = StorageSourceMgr::query_by(self.meta_storage.as_ref().as_ref(), by)
            .await
            .map_err(handle_error!("query source failed, by: {:?}", by))?;

        if let Some(source) = source {
            if let std::collections::hash_map::Entry::Vacant(v) =
                self.sources.write().await.entry(source.id)
            {
                let source = Arc::new(self.source_factory.from_source_info(source).await?);
                v.insert(SourceCache {
                    source: source.clone(),
                    source_tasks: HashMap::new(),
                });
                return Ok(Some(source));
            }
        }

        Ok(None)
    }

    pub(crate) async fn get_source(
        &self,
        by: &SourceQueryBy,
    ) -> BackupResult<Option<Arc<SourceWrapper>>> {
        self.get_source_impl(by)
            .await
            .map(|s| s.map(|s| Arc::new(SourceWrapper::new(s.source_id(), self.clone()))))
    }

    // should never call this function directly except `SourceWrapper`, use `Self::get_source` instead.
    pub(crate) async fn get_source_task_impl(
        &self,
        source_id: SourceId,
        task_uuid: &TaskUuid,
    ) -> BackupResult<Arc<Box<dyn SourceTask>>> {
        loop {
            {
                // read from cache
                let cache = self.sources.read().await;
                if let Some(source) = cache.get(&source_id) {
                    if let Some(source_task) = source.source_tasks.get(task_uuid) {
                        return Ok(source_task.source_task.clone());
                    }
                }
            }

            let source = self.get_source_impl(&SourceQueryBy::Id(source_id)).await?;
            let source = match source {
                Some(s) => s,
                None => {
                    return Err(BackupError::NotFound(format!(
                        "source({:?}) has been removed.",
                        source_id
                    )));
                }
            };

            let task = self.get_task_impl(&FindTaskBy::Uuid(*task_uuid)).await?;
            let task = match task {
                Some(t) => t,
                None => {
                    return Err(BackupError::NotFound(format!(
                        "task({}) has been removed.",
                        task_uuid
                    )));
                }
            };

            let source_task = source
                .source_task(task.uuid(), task.task_info().await?.source_entitiy.as_str())
                .await
                .map_err(handle_error!(
                    "get source task failed, source_id: {:?}, task_uuid: {}",
                    source_id,
                    task_uuid
                ))?;

            // insert into cache
            {
                let source_task = Arc::new(source_task);
                let mut cache = self.sources.write().await;
                if let Some(source) = cache.get_mut(&source_id) {
                    source.source_tasks.insert(
                        *task_uuid,
                        SourceTaskCache {
                            source_task: source_task.clone(),
                            source_lockeds: HashMap::new(),
                        },
                    );
                    return Ok(source_task);
                } else {
                    // source maybe remove from the cache by other thread, retry it.
                }
            }
        }
    }

    pub(crate) async fn get_source_task(
        &self,
        source_id: SourceId,
        task_uuid: &TaskUuid,
    ) -> BackupResult<Arc<SourceTaskWrapper>> {
        self.get_source_task_impl(source_id, task_uuid)
            .await
            .map(|_| Arc::new(SourceTaskWrapper::new(source_id, *task_uuid, self.clone())))
    }

    // should never call this function directly except `TargetWrapper`, use `Self::get_target` instead.
    pub(crate) async fn get_target_impl(
        &self,
        by: &TargetQueryBy,
    ) -> BackupResult<Option<Arc<Box<dyn Target>>>> {
        {
            let cache = self.targets.read().await;
            match by {
                TargetQueryBy::Id(id) => {
                    if let Some(t) = cache.get(id) {
                        return Ok(Some(t.target.clone()));
                    }
                }
                TargetQueryBy::Url(url) => {
                    for (id, t) in cache.iter() {
                        if let Ok(u) = t.target.target_info().await {
                            if u.url == *url {
                                return Ok(Some(t.target.clone()));
                            }
                        }
                    }
                }
            }
        }

        let target = StorageTargetMgr::query_by(self.meta_storage.as_ref().as_ref(), by)
            .await
            .map_err(handle_error!("query target failed, by: {:?}", by))?;

        if let Some(target) = target {
            if let std::collections::hash_map::Entry::Vacant(v) =
                self.targets.write().await.entry(target.id)
            {
                let target = Arc::new(self.target_factory.from_target_info(target).await?);
                v.insert(TargetCache {
                    target: target.clone(),
                    target_tasks: HashMap::new(),
                });
                return Ok(Some(target));
            }
        }

        Ok(None)
    }

    pub(crate) async fn get_target(
        &self,
        by: &TargetQueryBy,
    ) -> BackupResult<Option<Arc<TargetWrapper>>> {
        self.get_target_impl(by)
            .await
            .map(|t| t.map(|t| Arc::new(TargetWrapper::new(t.target_id(), self.clone()))))
    }

    pub(crate) async fn get_target_task_impl(
        &self,
        target_id: TargetId,
        task_uuid: &TaskUuid,
    ) -> BackupResult<Arc<Box<dyn TargetTask>>> {
        loop {
            {
                // read from cache
                let cache = self.targets.read().await;
                if let Some(target) = cache.get(&target_id) {
                    if let Some(target_task) = target.target_tasks.get(task_uuid) {
                        return Ok(target_task.target_task.clone());
                    }
                }
            }

            let target = self.get_target_impl(&TargetQueryBy::Id(target_id)).await?;
            let target = match target {
                Some(t) => t,
                None => {
                    return Err(BackupError::NotFound(format!(
                        "target({:?}) has been removed.",
                        target_id
                    )));
                }
            };

            let task = self.get_task_impl(&FindTaskBy::Uuid(*task_uuid)).await?;
            let task = match task {
                Some(t) => t,
                None => {
                    return Err(BackupError::NotFound(format!(
                        "task({}) has been removed.",
                        task_uuid
                    )));
                }
            };

            let target_task = target
                .target_task(task.uuid(), task.task_info().await?.target_entitiy.as_str())
                .await
                .map_err(handle_error!(
                    "get target task failed, target_id: {:?}, task_uuid: {}",
                    target_id,
                    task_uuid
                ))?;

            // insert into cache
            {
                let target_task = Arc::new(target_task);
                let mut cache = self.targets.write().await;
                if let Some(target) = cache.get_mut(&target_id) {
                    target.target_tasks.insert(
                        *task_uuid,
                        TargetTaskCache {
                            target_task: target_task.clone(),
                            target_checkpoints: HashMap::new(),
                        },
                    );
                    return Ok(target_task);
                } else {
                    // target maybe remove from the cache by other thread, retry it.
                }
            }
        }
    }

    pub(crate) async fn get_target_task(
        &self,
        target_id: TargetId,
        task_uuid: &TaskUuid,
    ) -> BackupResult<Arc<TargetTaskWrapper>> {
        self.get_target_task_impl(target_id, task_uuid)
            .await
            .map(|_| Arc::new(TargetTaskWrapper::new(target_id, *task_uuid, self.clone())))
    }

    pub(crate) async fn new_source_state(
        &self,
        task_uuid: &TaskUuid,
    ) -> BackupResult<LockedSourceStateId> {
        self.meta_storage
            .new_state(task_uuid, self.process_magic)
            .await
    }

    pub(crate) async fn save_source_original_state(
        &self,
        locked_state_id: LockedSourceStateId,
        original_state: Option<&str>,
    ) -> BackupResult<()> {
        self.meta_storage
            .original_state(locked_state_id, original_state)
            .await
    }

    pub(crate) async fn save_source_locked_state(
        &self,
        locked_state_id: LockedSourceStateId,
        locked_state: Option<&str>,
    ) -> BackupResult<()> {
        self.meta_storage
            .locked_state(locked_state_id, locked_state)
            .await
    }

    pub(crate) async fn delete_source_locked_state(
        &self,
        filter: &ListLockedSourceStateFilter,
    ) -> BackupResult<()> {
        self.meta_storage.delete_source_state(filter).await
    }

    pub(crate) async fn unlock_source_locked_state(
        &self,
        locked_state_id: LockedSourceStateId,
    ) -> BackupResult<()> {
        self.meta_storage.unlock_source_state(locked_state_id).await
    }

    pub(crate) async fn load_source_locked_state(
        &self,
        locked_state_id: LockedSourceStateId,
    ) -> BackupResult<SourceStateInfo> {
        self.meta_storage.state(locked_state_id).await
    }

    pub(crate) async fn list_locked_source_states(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListLockedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<SourceStateInfo>> {
        self.meta_storage
            .list_locked_source_states(task_uuid, filter, offset, limit)
            .await
    }

    pub(crate) async fn update_task_info(&self, task_info: &TaskInfo) -> BackupResult<()> {
        self.meta_storage.update_task(task_info).await?;
        // remove from cache, it will be reloaded when it's queried next time.
        self.tasks.write().await.remove(&task_info.uuid);
        self.sources.write().await.iter_mut().for_each(|(_, s)| {
            s.source_tasks.remove(&task_info.uuid);
        });
        self.targets.write().await.iter_mut().for_each(|(_, t)| {
            t.target_tasks.remove(&task_info.uuid);
        });
        Ok(())
    }

    pub(crate) async fn create_checkpoint_impl(
        &self,
        task_uuid: &TaskUuid,
        locked_source_id: Option<LockedSourceStateId>, // It will be lost for `None`
        prev_checkpoint_version: Option<CheckPointVersion>,
    ) -> BackupResult<Arc<CheckPointImpl>> {
        let version = self
            .meta_storage
            .create_checkpoint(task_uuid, locked_source_id, prev_checkpoint_version)
            .await?;

        // insert into cache
        loop {
            self.get_task_impl(&FindTaskBy::Uuid(*task_uuid))
                .await?
                .map_or(
                    Err(BackupError::NotFound(format!(
                        "task({}) has been removed",
                        task_uuid
                    ))),
                    |_| Ok(()),
                )?;

            let mut cache = self.tasks.write().await;
            if let Some(task_cache) = cache.get_mut(task_uuid) {
                let task_friendly_name = task_cache.task.info().friendly_name.clone();
                let checkpoint = task_cache
                    .checkpoints
                    .entry(version)
                    .or_insert_with(|| {
                        let checkpoint_info = CheckPointInfo {
                            task_uuid: task_uuid.clone(),
                            task_friendly_name,
                            version,
                            prev_version: prev_checkpoint_version,
                            complete_time: None,
                            locked_source_state_id: locked_source_id,
                            status: CheckPointStatus::Standby,
                            last_status_changed_time: SystemTime::now(),
                        };
                        Arc::new(CheckPointImpl::new(checkpoint_info, self.clone()))
                    })
                    .clone();
                return Ok(checkpoint);
            } else {
                // task maybe remove from the cache by other thread, retry it.
            }
        }
    }

    pub(crate) async fn create_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        locked_source_id: Option<LockedSourceStateId>, // It will be lost for `None`
        prev_checkpoint_version: Option<CheckPointVersion>,
    ) -> BackupResult<Arc<CheckPointWrapper>> {
        let checkpoint = self
            .create_checkpoint_impl(task_uuid, locked_source_id, prev_checkpoint_version)
            .await?;
        Ok(Arc::new(CheckPointWrapper::new(
            *task_uuid,
            checkpoint.version(),
            self.clone(),
        )))
    }

    // should never call this function directly except `CheckPointWrapper`, use `Self::get_checkpoint` instead.
    pub(crate) async fn list_checkpoints_impl(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<CheckPointImpl>>> {
        loop {
            self.get_task_impl(&FindTaskBy::Uuid(*task_uuid))
                .await?
                .map_or(
                    Err(BackupError::NotFound(format!(
                        "task({}) has been removed",
                        task_uuid
                    ))),
                    |_| Ok(()),
                )?;

            let checkpoint_infos = self
                .meta_storage
                .list_checkpoints(task_uuid, filter, offset, limit)
                .await?;

            let mut load_new_checkpoints = vec![];
            let mut cache = self.tasks.write().await;
            if let Some(task_cache) = cache.get_mut(task_uuid) {
                let checkpoints = checkpoint_infos
                    .into_iter()
                    .map(|info| {
                        let version = info.version;
                        task_cache
                            .checkpoints
                            .entry(version)
                            .or_insert_with(|| {
                                let old_status = info.status.clone();
                                let checkpoint = Arc::new(CheckPointImpl::new(info, self.clone()));
                                load_new_checkpoints.push((checkpoint.clone(), old_status));
                                checkpoint
                            })
                            .clone()
                    })
                    .collect();

                if load_new_checkpoints.len() > 0 {
                    let engine = self.clone();
                    tokio::task::spawn(async move {
                        futures::future::join_all(load_new_checkpoints.into_iter().map(
                            |(checkpoint, old_status)| {
                                engine.auto_resume_checkpoint_from_storage(checkpoint, old_status)
                            },
                        ))
                        .await;
                    });
                }

                return Ok(checkpoints);
            } else {
                // task maybe remove from the cache by other thread, retry it.
            }
        }
    }

    async fn auto_resume_checkpoint_from_storage(
        &self,
        checkpoint: Arc<CheckPointImpl>,
        old_status: CheckPointStatus,
    ) -> BackupResult<()> {
        match old_status {
            CheckPointStatus::Standby
            | CheckPointStatus::StopPrepare(_)
            | CheckPointStatus::Stop(_, _)
            | CheckPointStatus::Success
            | CheckPointStatus::Failed(_) => Ok(()),
            CheckPointStatus::Prepare(pending_status) => match pending_status {
                PendingStatus::Pending | PendingStatus::Started => checkpoint.prepare().await,
                PendingStatus::Done | PendingStatus::Failed(_) => Ok(()),
            },
            CheckPointStatus::Start(_, _) => checkpoint.transfer().await,
            CheckPointStatus::Delete(_, _, delete_from_target) => {
                let task = self
                    .get_task(&FindTaskBy::Uuid(checkpoint.task_uuid().clone()))
                    .await?
                    .ok_or_else(|| {
                        log::warn!(
                            "owner task({}) of checkpoint({:?}) has been removed",
                            checkpoint.task_uuid(),
                            checkpoint.version()
                        );
                        BackupError::NotFound("task has been removed".to_owned())
                    })?;
                task.remove_checkpoint(checkpoint.version(), delete_from_target)
                    .await
            }
        }
    }

    pub(crate) async fn list_checkpoints(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<CheckPointWrapper>>> {
        let checkpoints = self
            .list_checkpoints_impl(task_uuid, filter, offset, limit)
            .await?;
        let checkpoints = checkpoints
            .iter()
            .map(|cp| {
                Arc::new(CheckPointWrapper::new(
                    *cp.task_uuid(),
                    cp.version(),
                    self.clone(),
                ))
            })
            .collect();
        Ok(checkpoints)
    }

    // shold never call this function directly except `CheckPointWrapper`, use `Self::get_checkpoint` instead.
    pub(crate) async fn get_checkpoint_impl(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Arc<CheckPointImpl>>> {
        loop {
            self.get_task_impl(&FindTaskBy::Uuid(*task_uuid))
                .await?
                .map_or(
                    Err(BackupError::NotFound(format!(
                        "task({}) has been removed",
                        task_uuid
                    ))),
                    |_| Ok(()),
                )?;

            let checkpoint_info = self
                .meta_storage
                .query_checkpoint(task_uuid, version)
                .await?;

            match checkpoint_info {
                None => return Ok(None),
                Some(checkpoint_info) => {
                    let mut load_new_checkpoint = None;
                    let mut cache = self.tasks.write().await;
                    if let Some(task_cache) = cache.get_mut(task_uuid) {
                        let version = checkpoint_info.version;
                        let checkpoint = task_cache
                            .checkpoints
                            .entry(version)
                            .or_insert_with(|| {
                                let old_status: CheckPointStatus = checkpoint_info.status.clone();
                                let checkpoint =
                                    Arc::new(CheckPointImpl::new(checkpoint_info, self.clone()));
                                load_new_checkpoint = Some((checkpoint.clone(), old_status));
                                checkpoint
                            })
                            .clone();

                        if let Some((load_new_checkpoint, old_status)) = load_new_checkpoint {
                            let engine = self.clone();
                            tokio::task::spawn(async move {
                                engine
                                    .auto_resume_checkpoint_from_storage(
                                        load_new_checkpoint,
                                        old_status,
                                    )
                                    .await;
                            });
                        }

                        return Ok(Some(checkpoint));
                    } else {
                        // task maybe remove from the cache by other thread, retry it.
                    }
                }
            }
        }
    }

    pub(crate) async fn get_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Option<Arc<CheckPointWrapper>>> {
        self.get_checkpoint_impl(task_uuid, version)
            .await
            .map(|cp| {
                cp.map(|_| Arc::new(CheckPointWrapper::new(*task_uuid, version, self.clone())))
            })
    }

    // load `LockedSource` from cache or `meta_storage`, is similar to `get_source_task_impl`.
    pub(crate) async fn get_source_locked_impl(
        &self,
        source_id: SourceId,
        task_uuid: &TaskUuid,
        locked_state_id: LockedSourceStateId,
    ) -> BackupResult<Arc<Box<dyn LockedSource>>> {
        loop {
            {
                // read from cache
                let cache = self.sources.read().await;
                if let Some(source) = cache.get(&source_id) {
                    if let Some(source_task) = source.source_tasks.get(task_uuid) {
                        if let Some(source_locked) =
                            source_task.source_lockeds.get(&locked_state_id)
                        {
                            return Ok(source_locked.clone());
                        }
                    }
                }
            }

            let source = self.get_source_impl(&SourceQueryBy::Id(source_id)).await?;
            if source.is_none() {
                return Err(BackupError::NotFound(format!(
                    "source({:?}) has been removed.",
                    source_id
                )));
            };

            let task = self.get_task_impl(&FindTaskBy::Uuid(*task_uuid)).await?;
            if task.is_none() {
                return Err(BackupError::NotFound(format!(
                    "task({}) has been removed.",
                    task_uuid
                )));
            };

            let locked_state = self.load_source_locked_state(locked_state_id).await?;
            if let SourceState::Locked(_, locked_state) = locked_state {
                let source_task = self.get_source_task_impl(source_id, task_uuid).await?;
                let source_locked = source_task
                    .source_locked(locked_state_id, locked_state.as_deref())
                    .await
                    .map_err(handle_error!(
                        "create source locked failed, source_id: {:?}, task_uuid: {}, state_id: {:?}",
                        source_id,
                        task_uuid,
                        locked_state_id
                    ))?;

                // insert into cache
                {
                    let mut cache = self.sources.write().await;
                    if let Some(source) = cache.get_mut(&source_id) {
                        if let Some(source_task) = source.source_tasks.get_mut(task_uuid) {
                            let source_locked = source_task
                                .source_lockeds
                                .entry(locked_state_id)
                                .or_insert_with(|| Arc::new(source_locked))
                                .clone();
                            return Ok(source_locked);
                        } else {
                            // source task maybe remove from the cache by other thread, retry it.
                        }
                    } else {
                        // source maybe remove from the cache by other thread, retry it.
                    }
                }
            } else {
                return Err(BackupError::ErrorState(format!(
                    "source locked state({:?}) is not locked.",
                    locked_state_id
                )));
            }
        }
    }

    // get_source_locked
    pub(crate) async fn get_source_locked(
        &self,
        source_id: SourceId,
        task_uuid: &TaskUuid,
        locked_state_id: LockedSourceStateId,
    ) -> BackupResult<Arc<LockedSourceWrapper>> {
        self.get_source_locked_impl(source_id, task_uuid, locked_state_id)
            .await
            .map(|_| {
                Arc::new(LockedSourceWrapper::new(
                    source_id,
                    *task_uuid,
                    locked_state_id,
                    self.clone(),
                ))
            })
    }

    // get_target_checkpoint_impl, similar to `get_source_locked_impl`.
    pub(crate) async fn get_target_checkpoint_impl(
        &self,
        target_id: TargetId,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Arc<Box<dyn TargetCheckPoint>>> {
        loop {
            {
                // read from cache
                let cache = self.targets.read().await;
                if let Some(target) = cache.get(&target_id) {
                    if let Some(target_task) = target.target_tasks.get(task_uuid) {
                        if let Some(target_checkpoint) =
                            target_task.target_checkpoints.get(&version)
                        {
                            return Ok(target_checkpoint.clone());
                        }
                    }
                }
            }

            let target = self.get_target_impl(&TargetQueryBy::Id(target_id)).await?;
            if target.is_none() {
                return Err(BackupError::NotFound(format!(
                    "target({:?}) has been removed.",
                    target_id
                )));
            };

            let task = self.get_task_impl(&FindTaskBy::Uuid(*task_uuid)).await?;
            if task.is_none() {
                return Err(BackupError::NotFound(format!(
                    "task({}) has been removed.",
                    task_uuid
                )));
            };

            let checkpoint = self.get_checkpoint_impl(task_uuid, version).await?;
            let checkpoint = match checkpoint {
                Some(cp) => cp,
                None => {
                    return Err(BackupError::NotFound(format!(
                        "checkpoint({:?}) has been removed.",
                        version
                    )));
                }
            };

            let target_fill_meta = match checkpoint.info().target_meta {
                Some(tm) => tm,
                None => {
                    return Err(BackupError::ErrorState(format!(
                        "checkpoint({:?}) has no target meta. should start transfer first.",
                        version
                    )));
                }
            };

            let target_task = self.get_target_task_impl(target_id, task_uuid).await?;
            let target_checkpoint = target_task
                .target_checkpoint_from_filled_meta(
                    &checkpoint.info().meta,
                    target_fill_meta
                        .iter()
                        .map(|m| m.as_str())
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .await
                .map_err(handle_error!(
                    "create target checkpoint failed, target_id: {:?}, task_uuid: {}, version: {:?}",
                    target_id,
                    task_uuid,
                    version
                ))?;

            // insert into cache
            {
                let mut cache = self.targets.write().await;
                if let Some(target) = cache.get_mut(&target_id) {
                    if let Some(target_task) = target.target_tasks.get_mut(task_uuid) {
                        let target_checkpoint = target_task
                            .target_checkpoints
                            .entry(version)
                            .or_insert_with(|| Arc::new(target_checkpoint))
                            .clone();
                        return Ok(target_checkpoint);
                    } else {
                        // target task maybe remove from the cache by other thread, retry it.
                    }
                } else {
                    // target maybe remove from the cache by other thread, retry it.
                }
            }
        }
    }

    // get_target_checkpoint
    pub(crate) async fn get_target_checkpoint(
        &self,
        target_id: TargetId,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Arc<TargetCheckPointWrapper>> {
        self.get_target_checkpoint_impl(target_id, task_uuid, version)
            .await
            .map(|_| {
                Arc::new(TargetCheckPointWrapper::new(
                    target_id,
                    *task_uuid,
                    version,
                    self.clone(),
                ))
            })
    }

    pub(crate) async fn save_checkpoint_target_meta(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        target_meta: &[&str],
    ) -> BackupResult<()> {
        self.meta_storage
            .save_target_meta(task_uuid, version, target_meta)
            .await
    }

    pub(crate) async fn start_checkpoint_first(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()> {
        self.meta_storage
            .start_checkpoint_only_once_per_locked_source(task_uuid, version)
            .await
    }

    pub(crate) async fn update_checkpoint_status(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        new_status: CheckPointStatus,
    ) -> BackupResult<()> {
        self.meta_storage
            .update_status(task_uuid, version, new_status)
            .await
    }

    pub(crate) async fn add_transfer_map(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        item_path: &Path,
        target_address: Option<&[u8]>,
        info: &ChunkTransferInfo,
    ) -> BackupResult<u64> {
        self.meta_storage
            .add_transfer_map(task_uuid, version, item_path, target_address, info)
            .await
    }

    pub(crate) async fn query_transfer_map(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        filter: QueryTransferMapFilter<'_>,
    ) -> BackupResult<HashMap<PathBuf, HashMap<Vec<u8>, Vec<ChunkTransferInfo>>>> {
        self.meta_storage
            .query_transfer_map(task_uuid, version, filter)
            .await
    }
}

#[async_trait::async_trait]
impl SourceMgr for Engine {
    async fn register(
        &self,
        classify: String,
        url: String,
        friendly_name: String,
        config: String,
        description: String,
    ) -> BackupResult<SourceId> {
        let source_id = StorageSourceMgr::register(self.meta_storage.as_ref().as_ref(),
                classify.as_str(),
                url.as_str(),
                friendly_name.as_str(),
                config.as_str(),
                description.as_str(),
            )
            .await
            .map_err(handle_error!("insert new source failed, classify: {}, url: {}, friendly_name: {}, config: {}, description: {}", classify, url, friendly_name, config, description))?;

        let source_info = SourceInfo {
            id: source_id,
            classify,
            url,
            friendly_name,
            config,
            description,
        };

        let source = self.source_factory.from_source_info(source_info).await?;
        self.sources.write().await.insert(
            source_id,
            SourceCache {
                source: Arc::new(source),
                source_tasks: HashMap::new(),
            },
        );
        Ok(source_id)
    }

    async fn unregister(&self, by: &SourceQueryBy) -> BackupResult<()> {
        StorageSourceMgr::unregister(self.meta_storage.as_ref().as_ref(), by)
            .await
            .map_err(handle_error!(
                "unregister source failed, source_id: {:?}",
                by
            ))?;

        let mut cache = self.sources.write().await;
        match by {
            SourceQueryBy::Id(id) => {
                cache.remove(id);
            }
            SourceQueryBy::Url(url) => {
                let mut found_id = None;
                for (id, s) in cache.iter() {
                    if let Ok(u) = s.source.source_info().await {
                        if u.url == *url {
                            found_id = Some(*id);
                            break;
                        }
                    }
                }

                if let Some(id) = found_id {
                    cache.remove(&id);
                }
            }
        }
        Ok(())
    }

    async fn list(
        &self,
        filter: &ListSourceFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn Source>>> {
        let source_infos =
            StorageSourceMgr::list(self.meta_storage.as_ref().as_ref(), filter, offset, limit)
                .await
                .map_err(handle_error!(
                    "list sources failed, filter: {:?}, offset: {:?}, limit: {}",
                    filter,
                    offset,
                    limit
                ))?;

        let mut cache_sources = self.sources.write().await;
        let mut sources = vec![];
        for source_info in source_infos {
            let id = source_info.id;
            if let std::collections::hash_map::Entry::Vacant(v) =
                cache_sources.entry(source_info.id)
            {
                v.insert(SourceCache {
                    source: Arc::new(
                        self.source_factory
                            .from_source_info(source_info.clone())
                            .await?,
                    ),
                    source_tasks: HashMap::new(),
                });
            }
            sources.push(Arc::new(SourceWrapper::new(id, self.clone())) as Arc<dyn Source>);
        }

        Ok(sources)
    }

    async fn query_by(&self, by: &SourceQueryBy) -> BackupResult<Option<Arc<dyn Source>>> {
        self.get_source(by)
            .await
            .map(|s| s.map(|s| s as Arc<dyn Source>))
    }

    async fn update(
        &self,
        by: &SourceQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()> {
        StorageSourceMgr::update(self
                .meta_storage.as_ref().as_ref(), by, url.as_deref(), friendly_name.as_deref(), config.as_deref(), description.as_deref())
            .await
            .map_err(handle_error!(
                "update source failed, by: {:?}, url: {:?}, friendly_name: {:?}, config: {:?}, description: {:?}",
                by,
                url,
                friendly_name,
                config,
                description
            ))?;

        {
            // remove it in cache. it will be reloaded when it's queried next time.
            let mut cache = self.sources.write().await;
            match &by {
                SourceQueryBy::Id(id) => {
                    cache.remove(id);
                }
                SourceQueryBy::Url(url) => {
                    let mut found_id = None;
                    for (id, s) in cache.iter() {
                        if let Ok(u) = s.source.source_info().await {
                            if u.url == *url {
                                found_id = Some(*id);
                                break;
                            }
                        }
                    }

                    if let Some(id) = found_id {
                        cache.remove(&id);
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TargetMgr for Engine {
    // it's similar to `SourceMgr` currently
    async fn register(
        &self,
        classify: String,
        url: String,
        friendly_name: String,
        config: String,
        description: String,
    ) -> BackupResult<TargetId> {
        let target_id = StorageTargetMgr::register(self.meta_storage.as_ref().as_ref(),
                classify.as_str(),
                url.as_str(),
                friendly_name.as_str(),
                config.as_str(),
                description.as_str(),
            )
            .await
            .map_err(handle_error!("insert new target failed, classify: {}, url: {}, friendly_name: {}, config: {}, description: {}", classify, url, friendly_name, config, description))?;

        let target_info = TargetInfo {
            id: target_id,
            classify,
            url,
            friendly_name,
            config,
            description,
        };

        let target = self.target_factory.from_target_info(target_info).await?;
        self.targets.write().await.insert(
            target_id,
            TargetCache {
                target: Arc::new(target),
                target_tasks: HashMap::new(),
            },
        );
        Ok(target_id)
    }

    async fn unregister(&self, by: &TargetQueryBy) -> BackupResult<()> {
        StorageTargetMgr::unregister(self.meta_storage.as_ref().as_ref(), by)
            .await
            .map_err(handle_error!(
                "unregister target failed, target_id: {:?}",
                by
            ))?;

        let mut cache = self.targets.write().await;
        match by {
            TargetQueryBy::Id(id) => {
                cache.remove(id);
            }
            TargetQueryBy::Url(url) => {
                let mut found_id = None;
                for (id, t) in cache.iter() {
                    if let Ok(u) = t.target.target_info().await {
                        if u.url == *url {
                            found_id = Some(*id);
                            break;
                        }
                    }
                }

                if let Some(id) = found_id {
                    cache.remove(&id);
                }
            }
        }
        Ok(())
    }

    async fn list(
        &self,
        filter: &ListTargetFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn TargetEngine>>> {
        let target_infos =
            StorageTargetMgr::list(self.meta_storage.as_ref().as_ref(), filter, offset, limit)
                .await
                .map_err(handle_error!(
                    "list targets failed, filter: {:?}, offset: {:?}, limit: {}",
                    filter,
                    offset,
                    limit
                ))?;

        let mut cache_targets = self.targets.write().await;
        let mut targets = vec![];
        for target_info in target_infos {
            let id = target_info.id;
            if let std::collections::hash_map::Entry::Vacant(v) =
                cache_targets.entry(target_info.id)
            {
                v.insert(TargetCache {
                    target: Arc::new(
                        self.target_factory
                            .from_target_info(target_info.clone())
                            .await?,
                    ),
                    target_tasks: HashMap::new(),
                });
            }
            targets.push(Arc::new(TargetWrapper::new(id, self.clone())) as Arc<dyn TargetEngine>);
        }

        Ok(targets)
    }

    async fn query_by(&self, by: &TargetQueryBy) -> BackupResult<Option<Arc<dyn TargetEngine>>> {
        self.get_target(by)
            .await
            .map(|t| t.map(|t| t as Arc<dyn TargetEngine>))
    }

    async fn update(
        &self,
        by: &TargetQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()> {
        StorageTargetMgr::update(self
                .meta_storage.as_ref().as_ref(), by, url.as_deref(), friendly_name.as_deref(), config.as_deref(), description.as_deref())
            .await
            .map_err(handle_error!(
                "update target failed, by: {:?}, url: {:?}, friendly_name: {:?}, config: {:?}, description: {:?}",
                by,
                url,
                friendly_name,
                config,
                description
            ))?;

        {
            // remove it in cache. it will be reloaded when it's queried next time.
            let mut cache = self.targets.write().await;
            match &by {
                TargetQueryBy::Id(id) => {
                    cache.remove(id);
                }
                TargetQueryBy::Url(url) => {
                    let mut found_id = None;
                    for (id, t) in cache.iter() {
                        if let Ok(u) = t.target.target_info().await {
                            if u.url == *url {
                                found_id = Some(*id);
                                break;
                            }
                        }
                    }

                    if let Some(id) = found_id {
                        cache.remove(&id);
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Config for Engine {
    // 1. the field `config` is cache, so we need load it from `meta_storage` if it's not loaded yet.
    // 2. if there is no `config` set in `meta_storage`, we should return the default `EngineConfig`.
    // 3. we should update it when it's updated.
    async fn get_config(&self) -> BackupResult<EngineConfig> {
        {
            let config = self.config.read().await;
            if let Some(config) = &*config {
                return Ok(config.clone());
            }
        }

        let config = self
            .meta_storage
            .get_config()
            .await
            .map_err(handle_error!("get engine config failed"))?;

        let config = config.unwrap_or_default();
        *self.config.write().await = Some(config.clone());
        Ok(config)
    }

    async fn set_config(&self, config: EngineConfig) -> BackupResult<()> {
        self.meta_storage
            .set_config(&config)
            .await
            .map_err(handle_error!(
                "set engine config failed, config: {:?}",
                config
            ))?;

        *self.config.write().await = Some(config);
        Ok(())
    }
}

#[async_trait::async_trait]
impl TaskMgr for Engine {
    async fn create_task(
        &self,
        friendly_name: String,
        description: String,
        source_id: SourceId,
        source_param: String, // Any parameters(address .eg) for the source, the source can get it from engine.
        target_id: TargetId,
        target_param: String, // Any parameters(address .eg) for the target, the target can get it from engine.
        history_strategy: HistoryStrategy,
        priority: u32,
        attachment: String, // The application can save any attachment with task.
        flag: u64,          // Save any flags for the task. it will be filterd when list the tasks.
    ) -> BackupResult<Arc<dyn Task<CheckPointMetaEngine>>> {
        let uuid = TaskUuid::from(uuid::Uuid::new_v4().as_bytes().to_base58());

        let task_info = TaskInfo {
            uuid,
            friendly_name,
            description,
            source_id,
            source_param,
            target_id,
            target_param,
            priority,
            history_strategy,
            attachment,
            flag,
        };

        self.meta_storage
            .create_task(&task_info)
            .await
            .map_err(handle_error!(
                "create task failed, task_info: {:?}",
                task_info
            ))?;

        let task = Arc::new(TaskImpl::new(task_info, self.clone()));
        self.tasks.write().await.insert(
            uuid,
            TaskCache {
                task,
                checkpoints: HashMap::new(),
            },
        );

        Ok(Arc::new(TaskWrapper::new(self.clone(), uuid)))
    }

    // all transfering checkpoint should be stop first.
    async fn remove_task(&self, by: &FindTaskBy, is_remove_on_target: bool) -> BackupResult<()> {
        // 1. remove all checkpoints of the task.
        //      1.1 set remove flag on `meta_storage`.
        //      1.2 remove all storage on the target.
        //      1.3 remove all checkpoints from `meta_storage`.
        // 2. remove the task from `meta_storage`.
        todo!()
    }

    async fn list_task(
        &self,
        filter: &ListTaskFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<Arc<dyn Task<CheckPointMetaEngine>>>> {
        let task_infos = self
            .meta_storage
            .list_task(filter, offset, limit)
            .await
            .map_err(handle_error!(
                "list task failed, filter: {:?}, offset: {:?}, limit: {}",
                filter,
                offset,
                limit
            ))?;

        let mut task_cache = self.tasks.write().await;
        Ok(task_infos
            .into_iter()
            .map(|task_info| {
                let uuid = task_info.uuid;
                task_cache
                    .entry(task_info.uuid)
                    .or_insert_with(|| TaskCache {
                        task: Arc::new(TaskImpl::new(task_info, self.clone())),
                        checkpoints: HashMap::new(),
                    });
                Arc::new(TaskWrapper::new(self.clone(), uuid))
                    as Arc<dyn Task<CheckPointMetaEngine>>
            })
            .collect())
    }

    async fn find_task(
        &self,
        by: &FindTaskBy,
    ) -> BackupResult<Option<Arc<dyn Task<CheckPointMetaEngine>>>> {
        self.get_task(by)
            .await
            .map(|t| t.map(|t| t as Arc<dyn Task<CheckPointMetaEngine>>))
    }
}
