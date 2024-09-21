use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
    u64,
};

use sha2::digest::typenum::Length;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};

use crate::{
    checkpoint::{
        CheckPoint, CheckPointInfo, CheckPointObserver, CheckPointStatus, ChunkTransferInfo,
        DirChildType, LinkInfo, PrepareTransferChunkResult, StorageReader,
    },
    engine::{FindTaskBy, TaskUuid},
    engine_impl::Engine,
    error::{BackupError, BackupResult},
    meta::{CheckPointMetaEngine, CheckPointVersion, StorageItemAttributes},
    storage::{QueryTransferMapFilter, QueryTransferMapFilterItem},
};

struct FileTransferMap {
    chunk_maps: HashMap<Vec<u8>, Vec<ChunkTransferInfo>>, // <target-address, ChunkTransferInfo>
    speed: u64,                                           // bytes/s
    transferred: u64,                                     // bytes
}

enum ItemTransferInfo {
    File(FileTransferMap),
    Dir(DirTransferInfo),
}

struct DirTransferInfo {
    name: PathBuf,
    children: HashMap<PathBuf, ItemTransferInfo>,
}

impl DirTransferInfo {
    fn find_by_full_path_mut(
        &mut self,
        full_path: &Path,
        is_insert_default: bool,
    ) -> Option<&mut ItemTransferInfo> {
        // split full_path with '/'
        let mut path_names = vec![];
        for part in full_path.iter() {
            path_names.push(PathBuf::from(part));
        }

        // remove the first dir if it is empty
        match path_names.first() {
            Some(path) => {
                if path.as_os_str().is_empty() {
                    path_names.remove(0);
                }
            }
            None => {
                return None;
            }
        }

        let file_name = match path_names.pop() {
            Some(path) => path,
            None => return None,
        };

        // find the transfer-map follow the path-names in the transfer-map-cache
        let mut current_dir_transfer_info: &mut DirTransferInfo = self;
        for path_name in path_names {
            let entry = current_dir_transfer_info.children.entry(path_name.clone());
            let found = match entry {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    if is_insert_default {
                        entry.insert(ItemTransferInfo::Dir(DirTransferInfo {
                            name: path_name.clone(),
                            children: HashMap::new(),
                        }))
                    } else {
                        return None;
                    }
                }
            };

            match found {
                ItemTransferInfo::Dir(dir) => dir,
                ItemTransferInfo::File(_) => {
                    unreachable!("{} should be a dir", path_name.display())
                }
            };
        }

        let item_transfer_info = match current_dir_transfer_info.children.entry(file_name.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                if is_insert_default {
                    entry.insert(ItemTransferInfo::File(FileTransferMap {
                        chunk_maps: HashMap::new(),
                        speed: 0,
                        transferred: 0,
                    }))
                } else {
                    return None;
                }
            }
        };

        Some(item_transfer_info)
    }
}

pub(crate) struct CheckPointImpl {
    task_uuid: TaskUuid,
    version: CheckPointVersion,
    info: Arc<RwLock<CheckPointInfo<CheckPointMetaEngine>>>,
    full_meta: Arc<RwLock<Option<CheckPointMetaEngine>>>,
    transfer_map: Arc<Mutex<DirTransferInfo>>,
    engine: Engine,
}

impl CheckPointImpl {
    pub(crate) fn new(info: CheckPointInfo<CheckPointMetaEngine>, engine: Engine) -> Self {
        let task_uuid = info.meta.task_uuid;
        let version = info.meta.version;
        Self {
            info: Arc::new(RwLock::new(info)),
            full_meta: Arc::new(RwLock::new(None)),
            engine,
            task_uuid,
            version,
            transfer_map: Arc::new(Mutex::new(DirTransferInfo {
                name: PathBuf::new(),
                children: HashMap::new(),
            })),
        }
    }

    pub(crate) fn info(&self) -> CheckPointInfo<CheckPointMetaEngine> {
        let info = self.info.clone();
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move { info.read().await.clone() })
        })
    }

    pub(crate) async fn transfer_impl(&self) -> BackupResult<()> {
        let task = self
            .engine
            .get_task_impl(&FindTaskBy::Uuid(self.task_uuid))
            .await?;

        let task = match task {
            Some(task) => task,
            None => {
                return Err(BackupError::NotFound(format!(
                    "task({}) has been removed.",
                    self.task_uuid
                )));
            }
        };

        let mut info = self.info.read().await.clone();
        if info.target_meta.is_none() {
            // `service-meta` has not been filled by the `target`.
            // so we need to fill it by ourselves.
            let target_task = self
                .engine
                .get_target_task_impl(task.info().target_id, &self.task_uuid)
                .await?;
            let target_meta = target_task.fill_target_meta(&mut info.meta).await?;

            self.engine
                .save_checkpoint_target_meta(
                    &self.task_uuid,
                    self.version,
                    target_meta
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .await?;

            self.info.write().await.meta = info.meta.clone();
        }

        let target_checkpoint = self
            .engine
            .get_target_checkpoint_impl(task.info().target_id, &self.task_uuid, self.version)
            .await?;

        target_checkpoint.transfer().await
    }

    async fn load_transfer_map(
        &self,
        filter: QueryTransferMapFilter<'_>,
    ) -> BackupResult<HashMap<PathBuf, HashMap<Vec<u8>, Vec<ChunkTransferInfo>>>> {
        let transfer_map = self
            .engine
            .query_transfer_map(&self.task_uuid, self.version, filter)
            .await?;

        let mut transfer_map_cache = self.transfer_map.lock().await;

        let mut load_transfer_map = HashMap::new();

        for (full_path, target_address_map) in transfer_map {
            let load_transfer_map = load_transfer_map
                .entry(full_path.clone())
                .or_insert_with(HashMap::new);

            // find the transfer-map follow the path-names in the transfer-map-cache
            let item_transfer_info =
                match transfer_map_cache.find_by_full_path_mut(full_path.as_path(), true) {
                    Some(item_transfer_info) => item_transfer_info,
                    None => continue,
                };

            match item_transfer_info {
                ItemTransferInfo::File(file) => {
                    for (target_address, mut chunk_transfer_infos) in target_address_map {
                        let load_transfer_map_items = load_transfer_map
                            .entry(target_address.clone())
                            .or_insert_with(Vec::new);

                        let chunk_infos_cache =
                            file.chunk_maps.entry(target_address).or_insert(vec![]);
                        // insert the chunk_transfer_info into the chunk_infos_cache ascendingly by the chunk_transfer_info.offset if the prepared_chunk_id is not exists in the chunk_infos_cache
                        // 1. sort the chunk_transfer_infos by the offset
                        chunk_transfer_infos.sort_by_key(|k| k.offset);

                        // 2. insert the chunk_transfer_info into the chunk_infos_cache
                        let mut find_pos = 0;
                        for chunk_transfer_info in chunk_transfer_infos {
                            let pos = chunk_infos_cache.as_slice()[find_pos..]
                                .iter()
                                .position(|c| c.offset >= chunk_transfer_info.offset);

                            match pos {
                                Some(pos) => {
                                    let info_at_pos = chunk_infos_cache.get(pos).unwrap();
                                    let is_dup = info_at_pos.offset == chunk_transfer_info.offset;
                                    find_pos = pos;
                                    if !is_dup {
                                        load_transfer_map_items.push(chunk_transfer_info.clone());
                                        chunk_infos_cache.insert(pos, chunk_transfer_info);
                                    } else {
                                        load_transfer_map_items.push(info_at_pos.clone());
                                    }
                                }
                                None => {
                                    load_transfer_map_items.push(chunk_transfer_info.clone());
                                    chunk_infos_cache.push(chunk_transfer_info);
                                }
                            }
                        }
                    }
                }
                ItemTransferInfo::Dir(_) => {
                    unreachable!("{} should be a file", full_path.display())
                }
            }
        }

        Ok(load_transfer_map)
    }

    async fn full_meta_ref(
        &self,
    ) -> BackupResult<RwLockReadGuard<'_, Option<CheckPointMetaEngine>>> {
        {
            let full_meta_reader = self.full_meta.read().await;
            if full_meta_reader.is_some() {
                return Ok(full_meta_reader);
            }
        }

        self.full_meta().await?;

        let full_meta_reader = self.full_meta.read().await;
        Ok(full_meta_reader)
    }
}

#[async_trait::async_trait]
impl StorageReader for CheckPointImpl {
    async fn read_dir(&self, path: &Path) -> BackupResult<Vec<DirChildType>> {
        unimplemented!()
    }
    async fn file_size(&self, path: &Path) -> BackupResult<u64> {
        unimplemented!()
    }
    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        unimplemented!()
    }
    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo> {
        unimplemented!()
    }
    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CheckPoint<CheckPointMetaEngine> for CheckPointImpl {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }
    fn version(&self) -> CheckPointVersion {
        self.version
    }
    async fn info(&self) -> BackupResult<CheckPointInfo<CheckPointMetaEngine>> {
        Ok(self.info.read().await.clone())
    }
    async fn full_meta(&self) -> BackupResult<CheckPointMetaEngine> {
        if let Some(full_meta) = self.full_meta.read().await.clone() {
            return Ok(full_meta);
        }

        let meta = self.info.read().await.meta.clone();
        let full_meta = if meta.prev_versions.is_empty() {
            meta.clone()
        } else {
            let mut prev_checkpoint_results = futures::future::join_all(
                meta.prev_versions
                    .iter()
                    .map(|version| self.engine.get_checkpoint_impl(self.task_uuid(), *version)),
            )
            .await;

            // fail if there is any error
            let mut prev_checkpoints = Vec::with_capacity(meta.prev_versions.len());
            for i in 0..meta.prev_versions.len() {
                let checkpoint = prev_checkpoint_results.remove(0);
                match checkpoint {
                    Ok(cp) => match cp {
                        Some(cp) => prev_checkpoints.push(cp),
                        None => {
                            return Err(BackupError::NotFound(format!(
                                "prev-checkpoint(version: {:?}) has been removed.",
                                meta.prev_versions[i],
                            )));
                        }
                    },
                    Err(err) => return Err(err),
                }
            }

            // merge all prev checkpoints
            let mut prev_checkpoint_metas = Vec::with_capacity(meta.prev_versions.len() + 1);
            for cp in prev_checkpoints {
                let meta = cp.info.read().await.meta.clone();
                prev_checkpoint_metas.push(meta);
            }
            prev_checkpoint_metas.push(meta);

            CheckPointMetaEngine::combine_previous_versions(
                prev_checkpoint_metas.iter().collect::<Vec<_>>().as_slice(),
            )?
        };

        *self.full_meta.write().await = Some(full_meta.clone());
        Ok(full_meta)
    }

    async fn transfer(&self) -> BackupResult<()> {
        let info = self.info.read().await.clone();
        match info.status {
            CheckPointStatus::Success => {
                return Err(BackupError::ErrorState(format!(
                    "the checkpoint({}-{:?}) has successed.",
                    self.task_uuid, self.version
                )))
            }
            CheckPointStatus::Transfering => {
                return Err(BackupError::ErrorState(format!(
                    "the checkpoint({}-{:?}) is transferring.",
                    self.task_uuid, self.version
                )))
            }
            CheckPointStatus::Standby => {
                self.engine
                    .start_checkpoint_first(self.task_uuid(), self.version())
                    .await?;
            }
            _ => {
                self.engine
                    .update_checkpoint_status(
                        self.task_uuid(),
                        self.version(),
                        CheckPointStatus::Start,
                    )
                    .await?;
            }
        }

        self.info.write().await.status = CheckPointStatus::Transfering;

        if let Err(err) = self.transfer_impl().await {
            let new_status = CheckPointStatus::Failed(Some(err.clone()));
            self.engine
                .update_checkpoint_status(
                    self.task_uuid(),
                    self.version(),
                    CheckPointStatus::Failed(Some(err.clone())),
                )
                .await?;
            let mut info = self.info.write().await;
            match info.status {
                CheckPointStatus::Success => {}
                _ => info.status = new_status,
            }
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn stop(&self) -> BackupResult<()> {
        let info = self.info.read().await.clone();
        let task = self
            .engine
            .get_task_impl(&FindTaskBy::Uuid(info.meta.task_uuid))
            .await?
            .map_or(
                Err(BackupError::NotFound(format!(
                    "task({}) has been removed.",
                    self.task_uuid
                ))),
                |task| Ok(task),
            )?;

        match info.status {
            CheckPointStatus::Success => {
                return Err(BackupError::ErrorState(format!(
                    "the checkpoint({}-{:?}) has successed.",
                    self.task_uuid, self.version
                )))
            }
            CheckPointStatus::Transfering => {
                let target_checkpoint = self
                    .engine
                    .get_target_checkpoint_impl(
                        task.info().target_id,
                        self.task_uuid(),
                        self.version(),
                    )
                    .await?;
                target_checkpoint.stop().await?;
            }
            CheckPointStatus::Standby => {
                return Err(BackupError::ErrorState(format!(
                    "the checkpoint({}-{:?}) has not started.",
                    self.task_uuid, self.version
                )));
            }
            _ => {}
        }

        self.engine
            .update_checkpoint_status(self.task_uuid(), self.version(), CheckPointStatus::Stop)
            .await?;
        self.info.write().await.status = CheckPointStatus::Stop;
        Ok(())
    }

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>> {
        Ok(self.info.read().await.target_meta.clone())
    }

    async fn transfer_map_by_item_path(
        &self,
        item_full_paths: Option<Vec<&Path>>,
    ) -> BackupResult<HashMap<PathBuf, HashMap<Vec<u8>, Vec<ChunkTransferInfo>>>> // <item-full-path, target-address, ItemTransferInfo>
    {
        let full_meta_guard = self.full_meta_ref().await?;
        let full_meta = full_meta_guard.as_ref().unwrap();
        if let Some(item_full_paths) = item_full_paths.as_ref() {
            for item_full_path in item_full_paths {
                if full_meta.root.find_by_full_path(item_full_path).is_none() {
                    return Err(BackupError::NotFound(format!(
                        "item({}) not found",
                        item_full_path.display()
                    )));
                }
            }
        }

        self.load_transfer_map(QueryTransferMapFilter {
            items: item_full_paths.map(|pv| {
                pv.into_iter()
                    .map(|p| QueryTransferMapFilterItem {
                        path: p,
                        offset: 0,
                        length: u64::MAX,
                    })
                    .collect()
            }),
            target_addresses: None,
        })
        .await
    }

    async fn transfer_map_to_target_address(
        &self,
        target_addresses: Option<Vec<&[u8]>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<PathBuf, Vec<ChunkTransferInfo>>>> // <target-address, <item-full-path, ItemTransferInfo>>
    {
        let transfer_map = self
            .load_transfer_map(QueryTransferMapFilter {
                items: None,
                target_addresses,
            })
            .await?;

        let mut transfer_map_by_target_address = HashMap::new();
        for (item_full_path, target_address_map) in transfer_map {
            for (target_address, chunk_transfer_infos) in target_address_map {
                transfer_map_by_target_address
                    .entry(target_address)
                    .or_insert_with(HashMap::new)
                    .entry(item_full_path.clone())
                    .or_insert_with(Vec::new)
                    .extend(chunk_transfer_infos);
            }
        }

        Ok(transfer_map_by_target_address)
    }

    async fn get_all_transfer_target_address(&self) -> BackupResult<Vec<Vec<u8>>> {
        let transfer_map = self
            .load_transfer_map(QueryTransferMapFilter {
                items: None,
                target_addresses: None,
            })
            .await?;

        let mut target_addresses = HashSet::new();
        for (_, target_address_map) in transfer_map {
            for (target_address, _) in target_address_map {
                target_addresses.insert(target_address);
            }
        }
        Ok(target_addresses.into_iter().collect())
    }

    async fn status(&self) -> BackupResult<CheckPointStatus> {
        Ok(self.info.read().await.status.clone())
    }
}

#[async_trait::async_trait]
impl CheckPointObserver for CheckPointImpl {
    async fn on_success(&self) -> BackupResult<()> {
        self.engine
            .update_checkpoint_status(&self.task_uuid, self.version, CheckPointStatus::Success)
            .await?;

        self.info.write().await.status = CheckPointStatus::Success;
        Ok(())
    }
    async fn on_failed(&self, err: BackupError) -> BackupResult<()> {
        let failed_status = CheckPointStatus::Failed(Some(err));
        self.engine
            .update_checkpoint_status(&self.task_uuid, self.version, failed_status.clone())
            .await?;
        self.info.write().await.status = failed_status;
        Ok(())
    }
    async fn on_prepare_transfer_chunk(
        &self,
        item_full_path: &Path,
        offset: u64,
        length: u64,
        target_address: Option<&[u8]>, // specific target address
        detail: Option<&[u8]>,
    ) -> BackupResult<PrepareTransferChunkResult> {
        let full_meta_guard = self.full_meta_ref().await?;
        let full_meta = full_meta_guard.as_ref().unwrap();
        if full_meta.root.find_by_full_path(item_full_path).is_none() {
            return Err(BackupError::NotFound(format!(
                "item({}) not found",
                item_full_path.display()
            )));
        }

        loop {
            {
                let mut guard = self.transfer_map.lock().await;
                let transfer_map = guard.find_by_full_path_mut(item_full_path, false);
                if transfer_map.is_some() {
                    break;
                }
            }

            self.load_transfer_map(QueryTransferMapFilter {
                items: Some(vec![QueryTransferMapFilterItem {
                    path: item_full_path,
                    offset: 0,
                    length: u64::MAX,
                }]),
                target_addresses: None,
            })
            .await?;
            break;
        }

        let mut guard = self.transfer_map.lock().await;
        let transfer_map = guard.find_by_full_path_mut(item_full_path, true).unwrap();

        match transfer_map {
            ItemTransferInfo::File(transfer_map) => {
                let mut is_dup = false;
                for (ta, chunks) in transfer_map.chunk_maps.iter() {
                    if let Some(pos) = chunks.iter().position(|ck| {
                        !(((ck.offset <= offset) && ((ck.offset + ck.length) <= offset))
                            || (ck.offset >= (offset + length))
                                && ((ck.offset + ck.length) >= (offset + length)))
                    }) {
                        // duplicate chunk
                        is_dup = true;
                        let chunk = chunks.get(pos).unwrap();
                        if chunk.finish_time.is_none() {
                            return Ok(PrepareTransferChunkResult {
                                prepared_chunk_id: chunk.prepared_chunk_id,
                                target_address: ta.clone(),
                                info: chunk.clone(),
                            });
                        }
                    }
                }
                if is_dup {
                    return Err(BackupError::ErrorState(format!(
                        "the chunk({}-{}-{}) is duplicated with finished chunk.",
                        item_full_path.display(),
                        offset,
                        length
                    )));
                }

                let mut new_chunk = ChunkTransferInfo {
                    prepared_chunk_id: 0,
                    begin_time: SystemTime::now(),
                    finish_time: None,
                    offset,
                    length,
                    detail: detail.map(|d| d.to_vec()), // special parse for different target.
                };

                let prepared_chunk_id = self
                    .engine
                    .add_transfer_map(
                        &self.task_uuid,
                        self.version,
                        item_full_path,
                        target_address,
                        &new_chunk,
                    )
                    .await?;

                new_chunk.prepared_chunk_id = prepared_chunk_id;

                let chunks = transfer_map
                    .chunk_maps
                    .entry(target_address.unwrap_or(&[]).to_vec())
                    .or_insert_with(Vec::new);
                let pos = chunks.iter().position(|ck| ck.offset > offset);
                match pos {
                    Some(pos) => chunks.insert(pos, new_chunk.clone()),
                    None => chunks.push(new_chunk.clone()),
                }

                Ok(PrepareTransferChunkResult {
                    prepared_chunk_id,
                    target_address: target_address.unwrap_or(&[]).to_vec(),
                    info: new_chunk,
                })
            }
            ItemTransferInfo::Dir(_) => Err(BackupError::InvalidArgument(format!(
                "the item({}) is a directory.",
                item_full_path.display()
            ))),
        }
    }

    async fn on_item_transfer_done(
        &self,
        prepared_chunk_id: u64,
        target_address: Option<&[u8]>, // specific target address defined by target
        detail: Option<&[u8]>,
    ) -> BackupResult<()> {
        unimplemented!()
    }
    async fn save_key_value(&self, key: &str, value: &[u8], is_replace: bool) -> BackupResult<()> {
        unimplemented!()
    }
    async fn get_key_value(&self, key: &str) -> BackupResult<Option<Vec<u8>>> {
        unimplemented!()
    }
    async fn delete_key_value(&self, key: &str) -> BackupResult<()> {
        unimplemented!()
    }
}

pub(crate) struct CheckPointWrapper {
    task_uuid: TaskUuid,
    version: CheckPointVersion,
    engine: Engine,
}

impl CheckPointWrapper {
    pub(crate) fn new(task_uuid: TaskUuid, version: CheckPointVersion, engine: Engine) -> Self {
        Self {
            task_uuid,
            version,
            engine,
        }
    }
}

#[async_trait::async_trait]
impl StorageReader for CheckPointWrapper {
    async fn read_dir(&self, path: &Path) -> BackupResult<Vec<DirChildType>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_dir(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn file_size(&self, path: &Path) -> BackupResult<u64> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.file_size(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn read_file(&self, path: &Path, offset: u64, length: u32) -> BackupResult<Vec<u8>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_file(path, offset, length).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn read_link(&self, path: &Path) -> BackupResult<LinkInfo> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.read_link(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn stat(&self, path: &Path) -> BackupResult<StorageItemAttributes> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.stat(path).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
}

#[async_trait::async_trait]
impl CheckPoint<CheckPointMetaEngine> for CheckPointWrapper {
    fn task_uuid(&self) -> &TaskUuid {
        &self.task_uuid
    }

    fn version(&self) -> CheckPointVersion {
        self.version
    }

    async fn info(&self) -> BackupResult<CheckPointInfo<CheckPointMetaEngine>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.info.read().await.clone()),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn full_meta(&self) -> BackupResult<CheckPointMetaEngine> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => Ok(cp.full_meta().await?),
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer(&self) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.transfer().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn stop(&self) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.stop().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn target_meta(&self) -> BackupResult<Option<Vec<String>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.target_meta().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer_map_by_item_path(
        &self,
        item_full_paths: Option<Vec<&Path>>,
    ) -> BackupResult<HashMap<PathBuf, HashMap<Vec<u8>, Vec<ChunkTransferInfo>>>> // <item-full-path, target-address, ItemTransferInfo>
    {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.transfer_map_by_item_path(item_full_paths).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn transfer_map_to_target_address(
        &self,
        target_addresses: Option<Vec<&[u8]>>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<PathBuf, Vec<ChunkTransferInfo>>>> // <target-address, <item-full-path, ItemTransferInfo>>
    {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.transfer_map_to_target_address(target_addresses).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn get_all_transfer_target_address(&self) -> BackupResult<Vec<Vec<u8>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.get_all_transfer_target_address().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn status(&self) -> BackupResult<CheckPointStatus> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.status().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
}

#[async_trait::async_trait]
impl CheckPointObserver for CheckPointWrapper {
    async fn on_success(&self) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.on_success().await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn on_failed(&self, err: BackupError) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.on_failed(err).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn on_prepare_transfer_chunk(
        &self,
        item_full_path: &Path,
        offset: u64,
        length: u64,
        target_address: Option<&[u8]>, // specific target address
        detail: Option<&[u8]>,
    ) -> BackupResult<PrepareTransferChunkResult> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => {
                cp.on_prepare_transfer_chunk(item_full_path, offset, length, target_address, detail)
                    .await
            }
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn on_item_transfer_done(
        &self,
        prepared_chunk_id: u64,
        target_address: Option<&[u8]>, // specific target address defined by target
        detail: Option<&[u8]>,
    ) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => {
                cp.on_item_transfer_done(prepared_chunk_id, target_address, detail)
                    .await
            }
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn save_key_value(&self, key: &str, value: &[u8], is_replace: bool) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.save_key_value(key, value, is_replace).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn get_key_value(&self, key: &str) -> BackupResult<Option<Vec<u8>>> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.get_key_value(key).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }

    async fn delete_key_value(&self, key: &str) -> BackupResult<()> {
        match self
            .engine
            .get_checkpoint_impl(&self.task_uuid, self.version)
            .await?
        {
            Some(cp) => cp.delete_key_value(key).await,
            None => Err(BackupError::NotFound(format!(
                "checkpoint({}-{:?}) has been removed.",
                self.task_uuid, self.version
            ))),
        }
    }
}
