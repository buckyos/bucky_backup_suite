use std::collections::HashMap;

use crate::{
    checkpoint::{CheckPointInfo, CheckPointStatus, ItemTransferMap},
    engine::{
        EngineConfig, FindTaskBy, ListOffset, ListSourceFilter, ListTargetFilter, ListTaskFilter,
        SourceId, SourceInfo, SourceQueryBy, TargetId, TargetInfo, TargetQueryBy, TaskUuid,
    },
    error::BackupResult,
    meta::{CheckPointMetaEngine, CheckPointVersion, PreserveStateId},
    task::{ListCheckPointFilter, ListPreservedSourceStateFilter, SourceState, TaskInfo},
};

pub trait Storage:
    StorageSourceMgr
    + StorageTargetMgr
    + StorageTaskMgr
    + StorageSourceStateMgr
    + StorageCheckPointMgr
    + StorageCheckPointTransferMapMgr
    + StorageCheckPointKeyValueMgr
    + StorageConfig
{
}

#[async_trait::async_trait]
pub trait StorageSourceMgr: Send + Sync {
    async fn register(
        &self,
        classify: &str,
        url: &str,
        friendly_name: &str,
        config: &str,
        description: &str,
    ) -> BackupResult<SourceId>;

    async fn unregister(&self, by: &SourceQueryBy) -> BackupResult<()>;

    async fn list(
        &self,
        filter: &ListSourceFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<SourceInfo>>;

    async fn query_by(&self, by: &SourceQueryBy) -> BackupResult<Option<SourceInfo>>;

    async fn update(
        &self,
        by: &SourceQueryBy,
        url: Option<&str>,
        friendly_name: Option<&str>,
        config: Option<&str>,
        description: Option<&str>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageTargetMgr: Send + Sync {
    async fn register(
        &self,
        classify: &str,
        url: &str,
        friendly_name: &str,
        config: &str,
        description: &str,
    ) -> BackupResult<TargetId>;

    async fn unregister(&self, by: &TargetQueryBy) -> BackupResult<()>;

    async fn list(
        &self,
        filter: &ListTargetFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TargetInfo>>;

    async fn query_by(&self, by: &TargetQueryBy) -> BackupResult<Option<TargetInfo>>;

    async fn update(
        &self,
        by: &TargetQueryBy,
        url: Option<&str>,
        friendly_name: Option<&str>,
        config: Option<&str>,
        description: Option<&str>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageTaskMgr: Send + Sync {
    async fn create_task(&self, task_info: &TaskInfo) -> BackupResult<()>;

    async fn set_delete_flag(&self, by: &FindTaskBy, is_delete_on_target: bool)
        -> BackupResult<()>;

    async fn delete_task(&self, by: &FindTaskBy) -> BackupResult<()>;

    async fn list_task(
        &self,
        filter: &ListTaskFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TaskInfo>>;

    async fn query_task(&self, by: &FindTaskBy) -> BackupResult<Option<TaskInfo>>;

    async fn update_task(&self, task_info: &TaskInfo) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageConfig: Send + Sync {
    async fn get_config(&self) -> BackupResult<Option<EngineConfig>>;

    async fn set_config(&self, config: &EngineConfig) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageSourceStateMgr: Send + Sync {
    async fn new_state(
        &self,
        task_uuid: &TaskUuid,
        original_state: Option<&str>,
    ) -> BackupResult<PreserveStateId>;

    async fn preserved_state(
        &self,
        state_id: PreserveStateId,
        preserved_state: Option<&str>,
    ) -> BackupResult<()>;

    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState>;

    async fn list_preserved_source_states(
        &self,
        task_uuid: &TaskUuid,
        filter: ListPreservedSourceStateFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<(PreserveStateId, SourceState)>>;

    async fn delete_source_state(&self, state_id: PreserveStateId) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait StorageCheckPointMgr: Send + Sync {
    async fn create_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        preserved_source_id: Option<PreserveStateId>, // It will be lost for `None`
        meta: &CheckPointMetaEngine,
    ) -> BackupResult<CheckPointVersion>;

    async fn set_delete_flag(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        is_delete_on_target: bool,
    ) -> BackupResult<()>;

    async fn delete_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn start_checkpoint_only_once_per_preserved_source(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn update_status(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        status: CheckPointStatus,
    ) -> BackupResult<()>;

    // Maybe formated by the target in special way.
    // Save in string to avoid it changed by encode/decode.
    async fn save_target_meta(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        meta: &[&str],
    ) -> BackupResult<()>;

    async fn list_checkpoints(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo<CheckPointMetaEngine>>>;

    async fn query_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Option<CheckPointInfo<CheckPointMetaEngine>>>;
}

pub struct QueryTransferMapFilterItem<'a> {
    pub path: &'a [u8],
    pub offset: u64,
    pub length: u64,
}

pub struct QueryTransferMapFilter<'a> {
    pub items: Option<Vec<QueryTransferMapFilter<'a>>>,
    pub target_addresses: Option<Vec<&'a [u8]>>,
}

#[async_trait::async_trait]
pub trait StorageCheckPointTransferMapMgr: Send + Sync {
    // target_address: Where this chunk has been transferred to. users can get it from here.
    // but it should be parsed by the `target` for specific protocol.
    // the developer should remove the conflicting scope to update the transfer map.
    async fn add_transfer_map(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        item_path: &[u8],
        target_address: Option<&[u8]>,
        info: &ItemTransferMap,
    ) -> BackupResult<()>;

    async fn query_transfer_map<'a>(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        filter: QueryTransferMapFilter<'a>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>>; // <target-address, <item-path, ItemTransferMap>>
}

#[async_trait::async_trait]
pub trait StorageCheckPointKeyValueMgr: Send + Sync {
    async fn add_value(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        key: &str,
        value: &[u8],
        is_replace: bool,
    ) -> BackupResult<()>;
    async fn get_value(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        key: &str,
    ) -> BackupResult<Option<Vec<u8>>>;
}

#[async_trait::async_trait]
pub trait StorageCheckPointMgrSql: Send + Sync {
    async fn create_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        preserved_source_id: PreserveStateId,
        meta: &str,
    ) -> BackupResult<CheckPointVersion>;

    async fn set_delete_flag(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        is_delete_on_target: bool,
    ) -> BackupResult<()>;

    async fn delete_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn start_checkpoint_only_once_per_preserved_source(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn update_status(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        status: CheckPointStatus,
    ) -> BackupResult<()>;

    // Maybe formated by the target in special way.
    // Save in string to avoid it changed by encode/decode.
    async fn save_target_meta(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        meta: &[&str],
    ) -> BackupResult<()>;

    async fn list_checkpoints(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo<CheckPointMetaEngine>>>;

    async fn query_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Option<CheckPointInfo<CheckPointMetaEngine>>>;
}

#[async_trait::async_trait]
pub trait StorageCheckPointItemMgrSql: Send + Sync {
    async fn insert_item(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        item_path: &[u8],
    ) -> BackupResult<()>;
    async fn remove_items(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<usize>;
}

#[async_trait::async_trait]
pub trait StorageTransaction: Send + Sync {
    async fn start_transaction(&self) -> BackupResult<()>;
    async fn commit_transaction(&self) -> BackupResult<()>;
}

#[async_trait::async_trait]
impl<T> StorageCheckPointMgr for T
where
    T: StorageCheckPointMgrSql + StorageCheckPointItemMgrSql + StorageTransaction,
{
    async fn create_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        preserved_source_id: Option<PreserveStateId>,
        meta: &CheckPointMetaEngine,
    ) -> BackupResult<CheckPointVersion> {
        self.start_transaction().await?;
        // TODO: insert checkpoint
        // insert items
        self.commit_transaction().await?;
        unimplemented!()
    }

    async fn set_delete_flag(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        is_delete_on_target: bool,
    ) -> BackupResult<()> {
        StorageCheckPointMgrSql::set_delete_flag(self, task_uuid, version, is_delete_on_target)
            .await
    }

    async fn delete_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()> {
        self.start_transaction().await?;
        // TODO: delete items
        // delete checkpoint
        self.commit_transaction().await?;
        unimplemented!()
    }

    async fn start_checkpoint_only_once_per_preserved_source(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<()> {
        StorageCheckPointMgrSql::start_checkpoint_only_once_per_preserved_source(
            self, task_uuid, version,
        )
        .await
    }

    async fn update_status(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        status: CheckPointStatus,
    ) -> BackupResult<()> {
        StorageCheckPointMgrSql::update_status(self, task_uuid, version, status).await
    }

    // Maybe formated by the target in special way.
    // Save in string to avoid it changed by encode/decode.
    async fn save_target_meta(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
        meta: &[&str],
    ) -> BackupResult<()> {
        StorageCheckPointMgrSql::save_target_meta(self, task_uuid, version, meta).await
    }

    async fn list_checkpoints(
        &self,
        task_uuid: &TaskUuid,
        filter: &ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo<CheckPointMetaEngine>>> {
        StorageCheckPointMgrSql::list_checkpoints(self, task_uuid, filter, offset, limit).await
    }

    async fn query_checkpoint(
        &self,
        task_uuid: &TaskUuid,
        version: CheckPointVersion,
    ) -> BackupResult<Option<CheckPointInfo<CheckPointMetaEngine>>> {
        StorageCheckPointMgrSql::query_checkpoint(self, task_uuid, version).await
    }
}
