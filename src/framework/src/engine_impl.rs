use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{
    engine::{
        Config, EngineConfig, FindTaskBy, ListOffset, ListSourceFilter, ListTargetFilter,
        ListTaskFilter, SourceId, SourceInfo, SourceMgr, SourceQueryBy, TargetId, TargetInfo,
        TargetMgr, TargetQueryBy, TaskMgr, TaskUuid,
    },
    error::{BackupError, BackupResult},
    handle_error,
    meta::{CheckPointVersion, PreserveStateId},
    meta_storage::{MetaStorage, MetaStorageSourceMgr, MetaStorageTargetMgr},
    source::{Source, SourceCheckPoint, SourceEngine, SourceFactory, SourceTask},
    source_wrapper::{SourceTaskWrapper, SourceWrapper},
    target::{Target, TargetCheckPoint, TargetEngine, TargetFactory, TargetTask},
    target_wrapper::TargetWrapper,
    task::{HistoryStrategy, ListPreservedSourceStateFilter, SourceState, Task, TaskInfo},
    task_impl::{TaskImpl, TaskWrapper},
};

struct SourceTaskCache {
    source_task: Arc<Box<dyn SourceTask<String, String, String, String, String>>>,
    source_checkpoints: HashMap<CheckPointVersion, Arc<Box<dyn SourceCheckPoint>>>,
}

struct SourceCache {
    source: Arc<Box<dyn Source<String, String, String, String, String>>>,
    source_tasks: HashMap<TaskUuid, SourceTaskCache>,
}

struct TargetTaskCache {
    target_task: Arc<Box<dyn TargetTask<String, String, String, String, String>>>,
    target_checkpoints: HashMap<CheckPointVersion, Arc<Box<dyn TargetCheckPoint>>>,
}

struct TargetCache {
    target: Arc<Box<dyn Target<String, String, String, String, String>>>,
    target_tasks: HashMap<TaskUuid, TargetTaskCache>,
}

#[derive(Clone)]
pub struct Engine {
    meta_storage: Arc<Box<dyn MetaStorage>>,
    source_factory: Arc<Box<dyn SourceFactory<String, String, String, String, String>>>,
    target_factory: Arc<Box<dyn TargetFactory<String, String, String, String, String>>>,
    sources: Arc<RwLock<HashMap<SourceId, SourceCache>>>,
    targets: Arc<RwLock<HashMap<TargetId, TargetCache>>>,
    config: Arc<RwLock<Option<EngineConfig>>>,
    tasks: Arc<RwLock<HashMap<TaskUuid, Arc<TaskImpl>>>>,
}

impl Engine {
    pub fn new(
        meta_storage: Box<dyn MetaStorage>,
        source_factory: Box<dyn SourceFactory<String, String, String, String, String>>,
        target_factory: Box<dyn TargetFactory<String, String, String, String, String>>,
    ) -> Self {
        Self {
            meta_storage: Arc::new(meta_storage),
            source_factory: Arc::new(source_factory),
            target_factory: Arc::new(target_factory),
            sources: Arc::new(RwLock::new(HashMap::new())),
            targets: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(None)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
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
                return Ok(Some(task.clone()));
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
                .or_insert_with(|| Arc::new(TaskImpl::new(task_info, self.clone())))
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
    ) -> BackupResult<Option<Arc<Box<dyn Source<String, String, String, String, String>>>>> {
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

        let source = MetaStorageSourceMgr::query_by(self.meta_storage.as_ref().as_ref(), by)
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
    ) -> BackupResult<Arc<Box<dyn SourceTask<String, String, String, String, String>>>> {
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
                    return Err(BackupError::ErrorState(format!(
                        "source({:?}) has been removed.",
                        source_id
                    )));
                }
            };

            let task = self.get_task_impl(&FindTaskBy::Uuid(*task_uuid)).await?;
            let task = match task {
                Some(t) => t,
                None => {
                    return Err(BackupError::ErrorState(format!(
                        "task({}) has been removed.",
                        task_uuid
                    )));
                }
            };

            let source_task =
                source
                    .source_task(task.task_info().await?)
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
                            source_checkpoints: HashMap::new(),
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
    ) -> BackupResult<Option<Arc<Box<dyn Target<String, String, String, String, String>>>>> {
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

        let target = MetaStorageTargetMgr::query_by(self.meta_storage.as_ref().as_ref(), by)
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

    pub(crate) async fn save_source_original_state(
        &self,
        task_uuid: &TaskUuid,
        original_state: Option<&str>,
    ) -> BackupResult<PreserveStateId> {
        self.meta_storage.new_state(task_uuid, original_state).await
    }

    pub(crate) async fn save_source_preserved_state(
        &self,
        preserved_state_id: PreserveStateId,
        preserved_state: Option<&str>,
    ) -> BackupResult<()> {
        self.meta_storage
            .preserved_state(preserved_state_id, preserved_state)
            .await
    }

    pub(crate) async fn delete_source_preserved_state(
        &self,
        preserved_state_id: PreserveStateId,
    ) -> BackupResult<()> {
        self.meta_storage
            .delete_source_state(preserved_state_id)
            .await
    }

    pub(crate) async fn load_source_preserved_state(
        &self,
        preserved_state_id: PreserveStateId,
    ) -> BackupResult<SourceState> {
        self.meta_storage.state(preserved_state_id).await
    }

    pub(crate) async fn list_preserved_source_states(
        &self,
        task_uuid: &TaskUuid,
        filter: ListPreservedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>> {
        self.meta_storage
            .list_preserved_source_states(task_uuid, filter, offset, limit)
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
        let source_id = MetaStorageSourceMgr::register(self.meta_storage.as_ref().as_ref(),
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
        MetaStorageSourceMgr::unregister(self.meta_storage.as_ref().as_ref(), by)
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
    ) -> BackupResult<Vec<Arc<dyn SourceEngine>>> {
        let source_infos =
            MetaStorageSourceMgr::list(self.meta_storage.as_ref().as_ref(), filter, offset, limit)
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
            sources.push(Arc::new(SourceWrapper::new(id, self.clone())) as Arc<dyn SourceEngine>);
        }

        Ok(sources)
    }

    async fn query_by(&self, by: &SourceQueryBy) -> BackupResult<Option<Arc<dyn SourceEngine>>> {
        self.get_source(by)
            .await
            .map(|s| s.map(|s| s as Arc<dyn SourceEngine>))
    }

    async fn update(
        &self,
        by: &SourceQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()> {
        MetaStorageSourceMgr::update(self
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
        let target_id = MetaStorageTargetMgr::register(self.meta_storage.as_ref().as_ref(),
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
        MetaStorageTargetMgr::unregister(self.meta_storage.as_ref().as_ref(), by)
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
            MetaStorageTargetMgr::list(self.meta_storage.as_ref().as_ref(), filter, offset, limit)
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
        MetaStorageTargetMgr::update(self
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
        target_id: String,
        target_param: String, // Any parameters(address .eg) for the target, the target can get it from engine.
        history_strategy: HistoryStrategy,
        priority: u32,
        attachment: String, // The application can save any attachment with task.
        flag: u64,          // Save any flags for the task. it will be filterd when list the tasks.
    ) -> BackupResult<Arc<dyn Task>> {
        let uuid = TaskUuid::from(uuid::Uuid::new_v4());

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
        self.tasks.write().await.insert(uuid, task);

        Ok(Arc::new(TaskWrapper::new(self.clone(), uuid)))
    }

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
    ) -> BackupResult<Vec<Arc<dyn Task>>> {
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
                    .or_insert_with(|| Arc::new(TaskImpl::new(task_info, self.clone())));
                Arc::new(TaskWrapper::new(self.clone(), uuid)) as Arc<dyn Task>
            })
            .collect())
    }

    async fn find_task(&self, by: &FindTaskBy) -> BackupResult<Option<Arc<dyn Task>>> {
        self.get_task(by)
            .await
            .map(|t| t.map(|t| t as Arc<dyn Task>))
    }
}
