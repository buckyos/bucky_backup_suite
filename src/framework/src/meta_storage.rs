use std::collections::HashMap;

use crate::{
    checkpoint::{self, CheckPointInfo, CheckPointStatus, ItemTransferMap},
    engine::{
        FindTaskBy, ListOffset, ListTaskFilter, SourceId, SourceInfo, SourceQueryBy, TargetId,
        TargetInfo, TargetQueryBy,
    },
    error::BackupResult,
    meta::{CheckPointMetaEngine, CheckPointVersion, PreserveStateId},
    task::{ListCheckPointFilter, SourceState, TaskInfo},
};

pub trait MetaStorage:
    MetaStorageSourceMgr
    + MetaStorageTargetMgr
    + MetaStorageTaskMgr
    + MetaStorageSourceStateMgr
    + MetaStorageCheckPointMgr
    + MetaStorageCheckPointTransferMapMgr
    + MetaStorageCheckPointKeyValueMgr
{
}

#[async_trait::async_trait]
pub trait MetaStorageSourceMgr {
    async fn register(
        &self,
        classify: &str,
        url: &str,
        friendly_name: String,
        config: String,
        description: &str,
    ) -> BackupResult<SourceId>;

    async fn unregister(&self, by: SourceId) -> BackupResult<()>;

    async fn list(
        &self,
        classify: Option<&str>,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<SourceInfo>>;

    async fn query_by(&self, by: SourceQueryBy) -> BackupResult<Option<SourceInfo>>;

    async fn update(
        &self,
        by: SourceQueryBy,
        url: Option<&str>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<&str>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait MetaStorageTargetMgr {
    async fn register(
        &self,
        classify: &str,
        url: &str,
        friendly_name: String,
        config: String,
        description: &str,
    ) -> BackupResult<TargetId>;

    async fn unregister(&self, by: TargetQueryBy) -> BackupResult<()>;

    async fn list(
        &self,
        classify: Option<&str>,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TargetInfo>>;

    async fn query_by(&self, by: TargetQueryBy) -> BackupResult<Option<TargetInfo>>;

    async fn update(
        &self,
        by: TargetQueryBy,
        url: Option<&str>,
        friendly_name: Option<String>,
        config: Option<String>,
        description: Option<&str>,
    ) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait MetaStorageTaskMgr {
    async fn create_task(&self, task_info: &TaskInfo) -> BackupResult<()>;

    async fn delete_task(&self, by: FindTaskBy) -> BackupResult<()>;

    async fn list_task(
        &self,
        filter: ListTaskFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<TaskInfo>>;

    async fn query_task(&self, by: FindTaskBy) -> BackupResult<TaskInfo>;

    async fn update_task(&self, task_info: &TaskInfo) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait MetaStorageSourceStateMgr {
    async fn new_state(
        &self,
        task_uuid: &str,
        original_state: Option<&str>,
    ) -> BackupResult<PreserveStateId>;

    async fn preserved_state(
        &self,
        state_id: PreserveStateId,
        preserved_state: &str,
    ) -> BackupResult<()>;

    async fn state(&self, state_id: PreserveStateId) -> BackupResult<SourceState>;

    async fn list_preserved_source_states(&self, task_uuid: &str)
        -> BackupResult<Vec<SourceState>>;

    async fn delete_source_state(&self, state_id: PreserveStateId) -> BackupResult<()>;
}

#[async_trait::async_trait]
pub trait MetaStorageCheckPointMgr {
    async fn create_checkpoint(
        &self,
        task_uuid: &str,
        preserved_source_id: Option<PreserveStateId>, // It will be lost for `None`
        meta: &CheckPointMetaEngine,
    ) -> BackupResult<CheckPointVersion>;

    async fn delete_checkpoint(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn start_checkpoint_only_once_per_preserved_source(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn update_status(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        status: CheckPointStatus,
    ) -> BackupResult<()>;

    // Maybe formated by the target in special way.
    // Save in string to avoid it changed by encode/decode.
    async fn save_target_meta(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        meta: &[&str],
    ) -> BackupResult<()>;

    async fn list_checkpoints(
        &self,
        task_uuid: &str,
        filter: ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo<CheckPointMetaEngine>>>;

    async fn query_checkpoint(
        &self,
        task_uuid: &str,
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
pub trait MetaStorageCheckPointTransferMapMgr {
    // target_address: Where this chunk has been transferred to. users can get it from here.
    // but it should be parsed by the `target` for specific protocol.
    // the developer should remove the conflicting scope to update the transfer map.
    async fn add_transfer_map(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        item_path: &[u8],
        target_address: Option<&[u8]>,
        info: &ItemTransferMap,
    ) -> BackupResult<()>;

    async fn query_transfer_map<'a>(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        filter: QueryTransferMapFilter<'a>,
    ) -> BackupResult<HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<ItemTransferMap>>>>; // <target-address, <item-path, ItemTransferMap>>
}

#[async_trait::async_trait]
pub trait MetaStorageCheckPointKeyValueMgr {
    async fn add_value(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        key: &str,
        value: &[u8],
        is_replace: bool,
    ) -> BackupResult<()>;
    async fn get_value(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        key: &str,
    ) -> BackupResult<Option<Vec<u8>>>;
}

#[async_trait::async_trait]
pub trait MetaStorageCheckPointMgrSql: Send + Sync {
    async fn create_checkpoint(
        &self,
        task_uuid: &str,
        preserved_source_id: PreserveStateId,
        meta: &str,
    ) -> BackupResult<CheckPointVersion>;

    async fn delete_checkpoint(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<()>;
    async fn start_checkpoint_only_once_per_preserved_source(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<()>;

    async fn update_status(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        status: CheckPointStatus,
    ) -> BackupResult<()>;

    // Maybe formated by the target in special way.
    // Save in string to avoid it changed by encode/decode.
    async fn save_target_meta(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        meta: &[&str],
    ) -> BackupResult<()>;

    async fn list_checkpoints(
        &self,
        task_uuid: &str,
        filter: ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo<CheckPointMetaEngine>>>;

    async fn query_checkpoint(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<Option<CheckPointInfo<CheckPointMetaEngine>>>;
}

#[async_trait::async_trait]
pub trait MetaStorageCheckPointItemMgrSql: Send + Sync {
    async fn insert_item(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        item_path: &[u8],
    ) -> BackupResult<()>;
    async fn remove_items(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<usize>;
}

#[async_trait::async_trait]
pub trait MetaStorageTransaction: Send + Sync {
    async fn start_transaction(&self) -> BackupResult<()>;
    async fn commit_transaction(&self) -> BackupResult<()>;
}

#[async_trait::async_trait]
impl<T> MetaStorageCheckPointMgr for T
where
    T: MetaStorageCheckPointMgrSql + MetaStorageCheckPointItemMgrSql + MetaStorageTransaction,
{
    async fn create_checkpoint(
        &self,
        task_uuid: &str,
        preserved_source_id: Option<PreserveStateId>,
        meta: &CheckPointMetaEngine,
    ) -> BackupResult<CheckPointVersion> {
        self.start_transaction().await?;
        // TODO: insert checkpoint
        // insert items
        self.commit_transaction().await?;
        unimplemented!()
    }

    async fn delete_checkpoint(
        &self,
        task_uuid: &str,
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
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<()> {
        MetaStorageCheckPointMgrSql::start_checkpoint_only_once_per_preserved_source(
            self, task_uuid, version,
        )
        .await
    }

    async fn update_status(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        status: CheckPointStatus,
    ) -> BackupResult<()> {
        MetaStorageCheckPointMgrSql::update_status(self, task_uuid, version, status).await
    }

    // Maybe formated by the target in special way.
    // Save in string to avoid it changed by encode/decode.
    async fn save_target_meta(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
        meta: &[&str],
    ) -> BackupResult<()> {
        MetaStorageCheckPointMgrSql::save_target_meta(self, task_uuid, version, meta).await
    }

    async fn list_checkpoints(
        &self,
        task_uuid: &str,
        filter: ListCheckPointFilter,
        offset: ListOffset,
        limit: u32,
    ) -> BackupResult<Vec<CheckPointInfo<CheckPointMetaEngine>>> {
        MetaStorageCheckPointMgrSql::list_checkpoints(self, task_uuid, filter, offset, limit).await
    }

    async fn query_checkpoint(
        &self,
        task_uuid: &str,
        version: CheckPointVersion,
    ) -> BackupResult<Option<CheckPointInfo<CheckPointMetaEngine>>> {
        MetaStorageCheckPointMgrSql::query_checkpoint(self, task_uuid, version).await
    }
}
