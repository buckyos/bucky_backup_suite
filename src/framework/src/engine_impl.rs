use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{
    engine::{
        Config, EngineConfig, FindTaskBy, ListOffset, ListSourceFilter, ListTargetFilter,
        ListTaskFilter, SourceId, SourceInfo, SourceMgr, SourceQueryBy, TargetId, TargetInfo,
        TargetMgr, TargetQueryBy, TaskMgr, TaskUuid,
    },
    error::BackupResult,
    handle_error,
    meta_storage::{MetaStorage, MetaStorageSourceMgr, MetaStorageTargetMgr},
    task::{HistoryStrategy, Task, TaskInfo},
    task_impl::{TaskImpl, TaskWrapper},
};

#[derive(Clone)]
pub struct Engine {
    meta_storage: Arc<Box<dyn MetaStorage>>,
    sources: Arc<RwLock<HashMap<SourceId, SourceInfo>>>,
    targets: Arc<RwLock<HashMap<TargetId, TargetInfo>>>,
    config: Arc<RwLock<Option<EngineConfig>>>,
    tasks: Arc<RwLock<HashMap<TaskUuid, Arc<TaskImpl>>>>,
}

impl Engine {
    pub fn new(meta_storage: Box<dyn MetaStorage>) -> Self {
        Self {
            meta_storage: Arc::new(meta_storage),
            sources: Arc::new(RwLock::new(HashMap::new())),
            targets: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(None)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
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

        self.sources.write().await.insert(source_id, source_info);
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
                let source_id = cache.iter().find(|(_, s)| s.url == *url).map(|(id, _)| *id);
                if let Some(source_id) = source_id {
                    cache.remove(&source_id);
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
    ) -> BackupResult<Vec<SourceInfo>> {
        let sources =
            MetaStorageSourceMgr::list(self.meta_storage.as_ref().as_ref(), filter, offset, limit)
                .await
                .map_err(handle_error!(
                    "list sources failed, filter: {:?}, offset: {:?}, limit: {}",
                    filter,
                    offset,
                    limit
                ))?;

        let mut cache_sources = self.sources.write().await;
        sources.iter().for_each(|s| {
            cache_sources.entry(s.id).or_insert(s.clone());
        });

        Ok(sources)
    }

    async fn query_by(&self, by: &SourceQueryBy) -> BackupResult<Option<SourceInfo>> {
        {
            let cache = self.sources.read().await;
            let source = match by {
                SourceQueryBy::Id(id) => cache.get(id),
                SourceQueryBy::Url(url) => {
                    cache.iter().find(|(_, s)| s.url == *url).map(|(_, s)| s)
                }
            };

            if let Some(source) = source {
                return Ok(Some(source.clone()));
            }
        }

        let source = MetaStorageSourceMgr::query_by(self.meta_storage.as_ref().as_ref(), by)
            .await
            .map_err(handle_error!("query source failed, by: {:?}", by))?;

        if let Some(source) = &source {
            self.sources
                .write()
                .await
                .entry(source.id)
                .or_insert(source.clone());
        }

        Ok(source)
    }

    async fn update(
        &self,
        by: &SourceQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()> {
        {
            // try find it in cache first.
            let cache = self.sources.read().await;
            let source = match &by {
                SourceQueryBy::Id(id) => cache.get(id),
                SourceQueryBy::Url(url) => cache.values().find(|s| s.url == *url),
            };

            // check which fields are being updated, return Ok() if no fields are updated.
            if let Some(source) = source {
                if (url.is_none() || &source.url == url.as_ref().unwrap())
                    && (friendly_name.is_none()
                        || &source.friendly_name == friendly_name.as_ref().unwrap())
                    && (config.is_none() || &source.config == config.as_ref().unwrap())
                    && (description.is_none()
                        || &source.description == description.as_ref().unwrap())
                {
                    return Ok(());
                }
            }
        }

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
            // update fields in cache.
            let mut cache = self.sources.write().await;
            let source = match &by {
                SourceQueryBy::Id(id) => cache.get_mut(id),
                SourceQueryBy::Url(url) => cache.values_mut().find(|s| s.url == *url),
            };

            // update fields in cache if it's updated.
            if let Some(source) = source {
                if let Some(url) = url {
                    source.url = url;
                }
                if let Some(friendly_name) = friendly_name {
                    source.friendly_name = friendly_name;
                }
                if let Some(config) = config {
                    source.config = config;
                }
                if let Some(description) = description {
                    source.description = description;
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

        self.targets.write().await.insert(target_id, target_info);
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
                let target_id = cache.iter().find(|(_, t)| t.url == *url).map(|(id, _)| *id);
                if let Some(target_id) = target_id {
                    cache.remove(&target_id);
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
    ) -> BackupResult<Vec<TargetInfo>> {
        let targets =
            MetaStorageTargetMgr::list(self.meta_storage.as_ref().as_ref(), filter, offset, limit)
                .await
                .map_err(handle_error!(
                    "list targets failed, filter: {:?}, offset: {:?}, limit: {}",
                    filter,
                    offset,
                    limit
                ))?;

        let mut cache_targets = self.targets.write().await;
        targets.iter().for_each(|t| {
            cache_targets.entry(t.id).or_insert(t.clone());
        });

        Ok(targets)
    }

    async fn query_by(&self, by: &TargetQueryBy) -> BackupResult<Option<TargetInfo>> {
        {
            let cache = self.targets.read().await;
            let target = match by {
                TargetQueryBy::Id(id) => cache.get(id),
                TargetQueryBy::Url(url) => {
                    cache.iter().find(|(_, t)| t.url == *url).map(|(_, t)| t)
                }
            };

            if let Some(target) = target {
                return Ok(Some(target.clone()));
            }
        }

        let target = MetaStorageTargetMgr::query_by(self.meta_storage.as_ref().as_ref(), by)
            .await
            .map_err(handle_error!("query target failed, by: {:?}", by))?;

        if let Some(target) = &target {
            self.targets
                .write()
                .await
                .entry(target.id)
                .or_insert(target.clone());
        }

        Ok(target)
    }

    async fn update(
        &self,
        by: &TargetQueryBy,
        url: Option<String>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<String>,
    ) -> BackupResult<()> {
        {
            // try find it in cache first.
            let cache = self.targets.read().await;
            let target = match &by {
                TargetQueryBy::Id(id) => cache.get(id),
                TargetQueryBy::Url(url) => cache.values().find(|t| t.url == *url),
            };

            // check which fields are being updated, return Ok() if no fields are updated.
            if let Some(target) = target {
                if (url.is_none() || &target.url == url.as_ref().unwrap())
                    && (friendly_name.is_none()
                        || &target.friendly_name == friendly_name.as_ref().unwrap())
                    && (config.is_none() || &target.config == config.as_ref().unwrap())
                    && (description.is_none()
                        || &target.description == description.as_ref().unwrap())
                {
                    return Ok(());
                }
            }
        }

        MetaStorageTargetMgr::update(self.meta_storage.as_ref().as_ref(), by, url.as_deref(), friendly_name.as_deref(), config.as_deref(), description.as_deref())
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
            // update fields in cache.
            let mut cache = self.targets.write().await;
            let target = match &by {
                TargetQueryBy::Id(id) => cache.get_mut(id),
                TargetQueryBy::Url(url) => cache.values_mut().find(|t| t.url == *url),
            };

            // update fields in cache if it's updated.
            if let Some(target) = target {
                if let Some(url) = url {
                    target.url = url;
                }
                if let Some(friendly_name) = friendly_name {
                    target.friendly_name = friendly_name;
                }
                if let Some(config) = config {
                    target.config = config;
                }
                if let Some(description) = description {
                    target.description = description;
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

        let task = Arc::new(TaskImpl::new(task_info));
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
                    .or_insert_with(|| Arc::new(TaskImpl::new(task_info)));
                Arc::new(TaskWrapper::new(self.clone(), uuid)) as Arc<dyn Task>
            })
            .collect())
    }

    async fn find_task(&self, by: &FindTaskBy) -> BackupResult<Option<Arc<dyn Task>>> {
        self.get_task(by).await.map(|t| {
            t.map(|t| Arc::new(TaskWrapper::new(self.clone(), *t.uuid())) as Arc<dyn Task>)
        })
    }
}

impl Engine {
    pub(crate) async fn get_task(&self, by: &FindTaskBy) -> BackupResult<Option<Arc<TaskImpl>>> {
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
                .or_insert_with(|| Arc::new(TaskImpl::new(task_info)))
                .clone();
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }
}
