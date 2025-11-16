#![allow(unused)]
use crate::engine::*;
use crate::task_db::{
    BackupDbError, BackupPlanConfig, BackupTargetRecord, BackupTaskDb, SortOrder, TaskListQuery,
    TaskOrderField, TaskState, TaskType,
};
use ::kRPC::*;
use async_trait::async_trait;
use buckyos_backup_lib::{ChunkInnerPathHelper, RestoreConfig};
use buckyos_kit::{get_buckyos_service_data_dir, get_buckyos_system_bin_dir};
use chrono::{Local, TimeZone};
use cyfs_gateway_lib::*;
use cyfs_warp::*;
use log::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::result::Result;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct DirectoryChild {
    name: String,
    #[serde(rename = "isDirectory")]
    is_directory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectoryEntryType {
    Directory,
    File,
    Link,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedTargetType {
    Directory,
    File,
}

#[derive(Debug)]
struct DirectoryEntry {
    name: String,
    entry_type: DirectoryEntryType,
    target_type: Option<ResolvedTargetType>,
}

impl DirectoryEntry {
    fn is_directory_like(&self) -> bool {
        match self.entry_type {
            DirectoryEntryType::Directory => true,
            DirectoryEntryType::Link => {
                matches!(self.target_type, Some(ResolvedTargetType::Directory))
            }
            _ => false,
        }
    }

    fn is_file_like(&self) -> bool {
        match self.entry_type {
            DirectoryEntryType::File => true,
            DirectoryEntryType::Link => matches!(self.target_type, Some(ResolvedTargetType::File)),
            _ => false,
        }
    }

    fn into_child(self) -> DirectoryChild {
        let is_dir = self.is_directory_like();
        DirectoryChild {
            name: self.name,
            is_directory: is_dir,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ListDirectoryOptions {
    only_dirs: bool,
    only_files: bool,
}

#[derive(Clone)]
struct WebControlServer {
    task_db: BackupTaskDb,
}

impl WebControlServer {
    fn new() -> Self {
        let data_dir = get_buckyos_service_data_dir("backup_suite");
        if let Err(e) = fs::create_dir_all(&data_dir) {
            warn!("failed to ensure backup_suite data directory exists: {}", e);
        }
        let storage_path = data_dir.join("backup_targets.json");
        let task_db_path = data_dir.join("bucky_backup.db");
        let task_db = BackupTaskDb::new(task_db_path.to_str().unwrap());
        if let Err(err) = Self::migrate_legacy_targets(&task_db, &storage_path) {
            warn!(
                "failed to migrate legacy backup targets from {}: {}",
                storage_path.display(),
                err
            );
        }
        Self { task_db }
    }

    fn migrate_legacy_targets(
        task_db: &BackupTaskDb,
        legacy_path: &Path,
    ) -> std::result::Result<(), String> {
        if !legacy_path.exists() {
            return Ok(());
        }

        let existing = task_db
            .list_backup_target_ids()
            .map_err(|e| e.to_string())?;
        if !existing.is_empty() {
            return Ok(());
        }

        let content = fs::read_to_string(legacy_path).map_err(|e| e.to_string())?;
        if content.trim().is_empty() {
            Self::mark_legacy_file_migrated(legacy_path)?;
            return Ok(());
        }

        let records: Vec<BackupTargetRecord> =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;

        for record in records {
            match task_db.get_backup_target(&record.target_id) {
                Ok(_) => task_db
                    .update_backup_target(&record)
                    .map_err(|e| e.to_string())?,
                Err(BackupDbError::NotFound(_)) => task_db
                    .create_backup_target(&record)
                    .map_err(|e| e.to_string())?,
                Err(err) => return Err(err.to_string()),
            }
        }

        Self::mark_legacy_file_migrated(legacy_path)?;
        Ok(())
    }

    fn mark_legacy_file_migrated(path: &Path) -> std::result::Result<(), String> {
        let mut backup_path = path.to_path_buf();
        backup_path.set_extension("json.bak");
        if backup_path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())
        } else {
            fs::rename(path, &backup_path).map_err(|e| e.to_string())
        }
    }

    async fn create_backup_plan(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let params = &req.params;
        let extract_string = |key: &str| -> Result<String, RPCErrors> {
            params
                .get(key)
                .ok_or_else(|| RPCErrors::ParseRequestError(format!("{} is required", key)))?
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| RPCErrors::ParseRequestError(format!("{} must be a string", key)))
        };

        let type_str = extract_string("type_str")?;
        let source_type = extract_string("source_type")?;
        let source_raw = extract_string("source")?;
        let requested_target_type = extract_string("target_type")?;
        let target_id = extract_string("target")?;
        let title = extract_string("title")?;
        let description = extract_string("description")?;

        // Resolve source into a file URL for the engine while keeping a display-friendly path
        let (source_url_for_engine, display_source) = if source_raw.starts_with("file://") {
            let parsed = url::Url::parse(&source_raw).map_err(|_| {
                RPCErrors::ParseRequestError("source must be a valid file URL".to_string())
            })?;
            let source_path = parsed.to_file_path().map_err(|_| {
                RPCErrors::ParseRequestError(
                    "source must point to a local filesystem path".to_string(),
                )
            })?;
            (
                source_raw.clone(),
                source_path.to_string_lossy().to_string(),
            )
        } else {
            let resolved_path = resolve_requested_path(&source_raw);
            let file_url = url::Url::from_file_path(&resolved_path).map_err(|_| {
                RPCErrors::ParseRequestError(format!("invalid source path: {}", source_raw))
            })?;
            (
                file_url.to_string(),
                resolved_path.to_string_lossy().to_string(),
            )
        };

        // Lookup target details by target_id
        let target_record =
            self.task_db
                .get_backup_target(&target_id)
                .map_err(|err| match err {
                    BackupDbError::NotFound(_) => {
                        RPCErrors::ReasonError(format!("backup target {} not found", target_id))
                    }
                    _ => RPCErrors::ReasonError(err.to_string()),
                })?;

        if target_record.target_type != requested_target_type {
            warn!(
                "create_backup_plan: target type mismatch for target {} (request: {}, actual: {})",
                target_id, requested_target_type, target_record.target_type
            );
        }

        let policy_value = params
            .get("policy")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));
        let priority_value = params.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
        let reserved_versions_value = params
            .get("reserved_versions")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let policy_disabled = params
            .get("policy_disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Build plan config according to type
        let mut plan_config = match type_str.as_str() {
            "c2c" => {
                let mut config = BackupPlanConfig::chunk2chunk(
                    &source_url_for_engine,
                    target_record.target_id.as_str(),
                    &title,
                    &description,
                );
                config.last_checkpoint_index = 0;
                config
            }
            _ => {
                return Err(RPCErrors::ParseRequestError(format!(
                    "unknown type_str: {}",
                    type_str
                )));
            }
        };
        plan_config.policy = policy_value.clone();
        plan_config.priority = priority_value;
        let now = chrono::Utc::now().timestamp_millis();
        plan_config.create_time = now as u64;
        plan_config.update_time = now as u64;

        // Create plan via engine
        let engine = DEFAULT_ENGINE.lock().await;
        let last_checkpoint_index = plan_config.last_checkpoint_index;
        let plan_id = engine
            .create_backup_plan(plan_config)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        drop(engine);

        let result = json!({
            "plan_id": plan_id,
            "type_str": type_str,
            "source_type": source_type,
            "source": display_source,
            "target_type": target_record.target_type,
            "target_name": target_record.name,
            "target_url": target_record.url,
            "target": target_id,
            "title": title,
            "description": description,
            "policy": policy_value,
            "policy_disabled": policy_disabled,
            "priority": priority_value,
            "reserved_versions": reserved_versions_value,
            "last_checkpoint_index": last_checkpoint_index,
            "last_run_time": Value::Null,
            "create_time": now,
            "update_time": now,
            "total_backup": 0,
            "total_size": 0,
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn remove_backup_target(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let target_id_value = req.params.get("target_id");
        if target_id_value.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "target_id is required".to_string(),
            ));
        }
        let target_id = target_id_value.unwrap().as_str().ok_or_else(|| {
            RPCErrors::ParseRequestError("target_id must be a string".to_string())
        })?;

        self.task_db
            .remove_backup_target(target_id)
            .map_err(|err| match err {
                BackupDbError::NotFound(_) => {
                    RPCErrors::ReasonError(format!("backup target {} not found", target_id))
                }
                _ => RPCErrors::ReasonError(err.to_string()),
            })?;

        let result = json!({ "result": "success" });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn list_backup_plan(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let engine = DEFAULT_ENGINE.lock().await;
        let plans = engine
            .list_backup_plans()
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = json!({
            "backup_plans": plans
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn get_backup_plan(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "plan_id is required".to_string(),
            ));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        let plan = engine
            .get_backup_plan(plan_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let target_record = engine
            .get_target_record(plan.target.as_str())
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let mut result = plan.to_json_value();
        let is_running = engine.is_plan_have_running_backup_task(plan_id).await;

        let completed_backup_count = self
            .task_db
            .count_completed_backup_tasks(&plan_id)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let completed_backup_size = self
            .task_db
            .sum_completed_backup_items_size(&plan_id)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        result["is_running"] = json!(is_running);
        result["target_type"] = json!(target_record.target_type);
        result["target_url"] = json!(target_record.url);
        result["target_name"] = json!(target_record.name);
        result["total_size"] = json!(completed_backup_size);
        result["total_backup"] = json!(completed_backup_count);
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn create_backup_target(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let target_type_value = req.params.get("target_type");
        let target_url_value = req.params.get("url");
        let name_value = req.params.get("name");

        if target_type_value.is_none() || target_url_value.is_none() || name_value.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "target_type, url, name are required".to_string(),
            ));
        }

        let target_type = target_type_value.unwrap().as_str().ok_or_else(|| {
            RPCErrors::ParseRequestError("target_type must be a string".to_string())
        })?;
        let target_url = {
            let mut target_url = target_url_value
                .unwrap()
                .as_str()
                .ok_or_else(|| RPCErrors::ParseRequestError("url must be a string".to_string()))?
                .to_string();

            #[cfg(not(windows))]
            {
                if (target_type == "file") {
                    if (!target_url.starts_with("/")) {
                        target_url = "/".to_string() + target_url.as_str();
                    }
                }
            }

            // Check target_url
            if target_url.starts_with("file://") {
                let parsed = url::Url::parse(target_url.as_str()).map_err(|_| {
                    RPCErrors::ParseRequestError("source must be a valid file URL".to_string())
                })?;
                parsed.to_file_path().map_err(|_| {
                    RPCErrors::ParseRequestError(
                        "source must point to a local filesystem path".to_string(),
                    )
                })?;
            } else {
                let resolved_path = resolve_requested_path(target_url.as_str());
                url::Url::from_file_path(&resolved_path).map_err(|_| {
                    RPCErrors::ParseRequestError(format!("invalid source path: {}", target_url))
                })?;
            };

            target_url
        };
        let target_name = name_value
            .unwrap()
            .as_str()
            .ok_or_else(|| RPCErrors::ParseRequestError("name must be a string".to_string()))?;

        let description = req.params.get("description").and_then(|v| v.as_str());
        let config_value = req.params.get("config").cloned();
        let config = match config_value {
            Some(Value::Null) | None => None,
            Some(value) => Some(value),
        };

        let target_id = Uuid::new_v4().to_string();
        let record = BackupTargetRecord::new(
            target_id.clone(),
            target_type,
            target_url.as_str(),
            target_name,
            description,
            config,
        );

        self.task_db
            .create_backup_target(&record)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = record.to_json_value();
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn list_backup_target(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let target_ids = self
            .task_db
            .list_backup_target_ids()
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = json!({ "targets": target_ids });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn get_backup_target(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let target_id_value = req.params.get("target_id");
        if target_id_value.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "target_id is required".to_string(),
            ));
        }
        let target_id = target_id_value.unwrap().as_str().ok_or_else(|| {
            RPCErrors::ParseRequestError("target_id must be a string".to_string())
        })?;

        let record = self
            .task_db
            .get_backup_target(target_id)
            .map_err(|err| match err {
                BackupDbError::NotFound(_) => {
                    RPCErrors::ReasonError(format!("backup target {} not found", target_id))
                }
                _ => RPCErrors::ReasonError(err.to_string()),
            })?;
        let result = record.to_json_value();

        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn update_backup_target(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let target_id_value = req.params.get("target_id");
        if target_id_value.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "target_id is required".to_string(),
            ));
        }
        let target_id = target_id_value.unwrap().as_str().ok_or_else(|| {
            RPCErrors::ParseRequestError("target_id must be a string".to_string())
        })?;

        let mut record = self
            .task_db
            .get_backup_target(target_id)
            .map_err(|err| match err {
                BackupDbError::NotFound(_) => {
                    RPCErrors::ReasonError(format!("backup target {} not found", target_id))
                }
                _ => RPCErrors::ReasonError(err.to_string()),
            })?;
        if let Some(value) = req.params.get("target_type") {
            if !value.is_null() {
                let target_type = value.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError("target_type must be a string".to_string())
                })?;
                record.target_type = target_type.to_string();
            }
        }

        if let Some(value) = req.params.get("url") {
            if !value.is_null() {
                let target_url = value.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError("url must be a string".to_string())
                })?;
                record.url = target_url.to_string();
            }
        }

        if let Some(value) = req.params.get("name") {
            if !value.is_null() {
                let target_name = value.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError("name must be a string".to_string())
                })?;
                record.name = target_name.to_string();
            }
        }

        if let Some(value) = req.params.get("description") {
            if !value.is_null() {
                let description = value.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError("description must be a string".to_string())
                })?;
                record.description = description.to_string();
            }
        }

        if let Some(value) = req.params.get("state") {
            if !value.is_null() {
                let state = value.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError("state must be a string".to_string())
                })?;
                record.state = state.to_string();
            }
        }

        if let Some(value) = req.params.get("used") {
            if !value.is_null() {
                let used = match value {
                    Value::Number(num) => num.as_u64().ok_or_else(|| {
                        RPCErrors::ParseRequestError(
                            "used must be a non-negative integer".to_string(),
                        )
                    })?,
                    Value::String(s) => s.parse::<u64>().map_err(|_| {
                        RPCErrors::ParseRequestError(
                            "used must be a non-negative integer".to_string(),
                        )
                    })?,
                    _ => {
                        return Err(RPCErrors::ParseRequestError(
                            "used must be a non-negative integer".to_string(),
                        ))
                    }
                };
                record.used = used;
            }
        }

        if let Some(value) = req.params.get("total") {
            if !value.is_null() {
                let total = match value {
                    Value::Number(num) => num.as_u64().ok_or_else(|| {
                        RPCErrors::ParseRequestError(
                            "total must be a non-negative integer".to_string(),
                        )
                    })?,
                    Value::String(s) => s.parse::<u64>().map_err(|_| {
                        RPCErrors::ParseRequestError(
                            "total must be a non-negative integer".to_string(),
                        )
                    })?,
                    _ => {
                        return Err(RPCErrors::ParseRequestError(
                            "total must be a non-negative integer".to_string(),
                        ))
                    }
                };
                record.total = total;
            }
        }

        if let Some(value) = req.params.get("last_error") {
            if !value.is_null() {
                let last_error = value.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError("last_error must be a string".to_string())
                })?;
                record.last_error = last_error.to_string();
            }
        }

        if let Some(value) = req.params.get("config") {
            if value.is_null() {
                record.config = None;
            } else {
                record.config = Some(value.clone());
            }
        }

        let updated_snapshot = record.to_json_value();
        self.task_db
            .update_backup_target(&record)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = json!({
            "result": "success",
            "target": updated_snapshot,
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    //return the new task info
    async fn create_backup_task(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "plan_id is required".to_string(),
            ));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let parent_checkpoint_id = req.params.get("parent_checkpoint_id");
        let real_parent_checkpoint_id = if parent_checkpoint_id.is_some() {
            Some(parent_checkpoint_id.unwrap().as_str().unwrap())
        } else {
            None
        };
        let engine = DEFAULT_ENGINE.lock().await;
        let task_id = engine
            .create_backup_task(plan_id, real_parent_checkpoint_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let task_info = engine
            .get_task_info(&task_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = task_info.to_json_value();
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn create_restore_task(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "plan_id is required".to_string(),
            ));
        }
        let checkpoint_id = req.params.get("checkpoint_id");
        if checkpoint_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "checkpoint_id is required".to_string(),
            ));
        }
        let restore_config = req.params.get("cfg");
        if restore_config.is_none() {
            return Err(RPCErrors::ParseRequestError("cfg is required".to_string()));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let checkpoint_id = checkpoint_id.unwrap().as_str().unwrap();
        let restore_config = serde_json::from_value(restore_config.unwrap().clone())
            .map_err(|err| RPCErrors::ParseRequestError("cfg format error".to_string()))?;

        let engine = DEFAULT_ENGINE.lock().await;
        let task_id = engine
            .create_restore_task(plan_id, checkpoint_id, restore_config)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let task_info = engine
            .get_task_info(&task_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = task_info.to_json_value();
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn list_files_in_task(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let task_id_value = req.params.get("taskid");
        if task_id_value.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "taskid is required".to_string(),
            ));
        }
        let task_id = task_id_value
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| RPCErrors::ParseRequestError("taskid must be a string".to_string()))?;

        let subdir = match req.params.get("subdir") {
            Some(Value::Null) | None => None,
            Some(Value::String(s)) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Some(_) => {
                return Err(RPCErrors::ParseRequestError(
                    "subdir must be a string or null".to_string(),
                ))
            }
        };

        let task = self
            .task_db
            .load_task_by_id(&task_id)
            .map_err(|err| match err {
                BackupDbError::NotFound(_) => {
                    RPCErrors::ReasonError(format!("task {} not found", task_id))
                }
                _ => RPCErrors::ReasonError(err.to_string()),
            })?;

        // TODO: 这里条目可能太多，一次性加载过于消耗内存
        let checkpoint_id = task.checkpoint_id.clone();
        let items = self
            .task_db
            .load_backup_chunk_items_by_checkpoint(&checkpoint_id, subdir.as_deref(), None, None)
            .map_err(|err| RPCErrors::ReasonError(err.to_string()))?;

        let normalized_subdir = subdir
            .as_ref()
            .map(|raw| ChunkInnerPathHelper::normalize_virtual_path(raw))
            .filter(|value| !value.is_empty());
        let subdir_prefix = normalized_subdir.as_ref().map(|base| format!("{}/", base));

        struct FileAggregate {
            total_size: u64,
            min_time: u64,
            max_time: u64,
        }

        let mut file_entries: HashMap<String, FileAggregate> = HashMap::new();
        let mut dir_entries: HashSet<String> = HashSet::new();

        for item in items {
            let cleaned_path =
                ChunkInnerPathHelper::strip_chunk_suffix(&item.item_id.replace('\\', "/"));
            if cleaned_path.is_empty() {
                continue;
            }

            let normalized_path = ChunkInnerPathHelper::normalize_virtual_path(&cleaned_path);
            if normalized_path.is_empty() {
                continue;
            }

            let relative_path = if let (Some(base), Some(prefix)) =
                (normalized_subdir.as_ref(), subdir_prefix.as_ref())
            {
                if normalized_path == *base {
                    continue;
                }
                if let Some(stripped) = normalized_path.strip_prefix(prefix.as_str()) {
                    let trimmed = stripped.trim_start_matches('/');
                    if trimmed.is_empty() {
                        continue;
                    }
                    trimmed.to_string()
                } else {
                    continue;
                }
            } else {
                normalized_path.clone()
            };

            if relative_path.is_empty() {
                continue;
            }

            let mut segments = relative_path.split('/');
            let first = match segments.next() {
                Some(segment) if !segment.is_empty() => segment,
                _ => continue,
            };

            if segments.next().is_some() {
                dir_entries.insert(first.to_string());
                continue;
            }

            let entry = file_entries
                .entry(first.to_string())
                .or_insert(FileAggregate {
                    total_size: 0,
                    min_time: item.last_update_time,
                    max_time: item.last_update_time,
                });
            entry.total_size = entry.total_size.saturating_add(item.size);
            entry.min_time = entry.min_time.min(item.last_update_time);
            entry.max_time = entry.max_time.max(item.last_update_time);
        }

        let mut entries: Vec<(bool, String, u64, u64, u64)> = Vec::new();
        for name in dir_entries {
            entries.push((true, name, 0, 0, 0));
        }
        for (name, agg) in file_entries {
            entries.push((false, name, agg.total_size, agg.min_time, agg.max_time));
        }

        entries.sort_by(|a, b| match (a.0, b.0) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
        });

        let files: Vec<Value> = entries
            .into_iter()
            .map(|(is_dir, name, len, create_time, update_time)| {
                json!({
                    "name": name,
                    "len": len,
                    "create_time": create_time,
                    "update_time": update_time,
                    "is_dir": is_dir,
                })
            })
            .collect();

        let result = json!({ "files": files });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn list_backup_task(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let filter_value = req.params.get("filter");
        let offset = match req.params.get("offset") {
            Some(Value::Number(n)) => n.as_u64().ok_or_else(|| {
                RPCErrors::ParseRequestError("offset must be a non-negative integer".to_string())
            })? as usize,
            Some(Value::Null) | None => 0,
            _ => {
                return Err(RPCErrors::ParseRequestError(
                    "offset must be a non-negative integer".to_string(),
                ))
            }
        };
        let limit = match req.params.get("limit") {
            Some(Value::Number(n)) => Some(n.as_u64().ok_or_else(|| {
                RPCErrors::ParseRequestError("limit must be a non-negative integer".to_string())
            })? as usize),
            Some(Value::Null) | None => None,
            _ => {
                return Err(RPCErrors::ParseRequestError(
                    "limit must be a non-negative integer".to_string(),
                ))
            }
        };
        let order_by = parse_order_by(req.params.get("order_by"))?;

        let mut legacy_filter: Option<String> = None;
        let mut state_filter: Option<Vec<TaskState>> = None;
        let mut type_filter: Option<Vec<TaskType>> = None;
        let mut owner_plan_id_filter: Option<Vec<String>> = None;
        let mut owner_plan_title_filter: Option<Vec<String>> = None;

        if let Some(filter_value) = filter_value {
            match filter_value {
                Value::Null => {}
                Value::String(s) => {
                    if !s.is_empty() {
                        legacy_filter = Some(s.clone());
                    }
                }
                Value::Object(obj) => {
                    if let Some(value) = obj.get("state") {
                        let parsed = parse_task_states(value)?;
                        if !parsed.is_empty() {
                            state_filter = Some(parsed);
                        }
                    }
                    if let Some(value) = obj.get("type") {
                        let parsed = parse_task_types(value)?;
                        if !parsed.is_empty() {
                            type_filter = Some(parsed);
                        }
                    }
                    if let Some(value) = obj.get("owner_plan_id") {
                        let parsed = parse_string_list(value, "owner_plan_id")?;
                        if !parsed.is_empty() {
                            owner_plan_id_filter = Some(parsed);
                        }
                    }
                    if let Some(value) = obj.get("owner_plan_title") {
                        let parsed = parse_string_list(value, "owner_plan_title")?;
                        if !parsed.is_empty() {
                            owner_plan_title_filter = Some(parsed);
                        }
                    }
                }
                _ => {
                    return Err(RPCErrors::ParseRequestError(
                        "filter must be a string or object".to_string(),
                    ))
                }
            }
        }

        let plan_title_filters_lower = owner_plan_title_filter
            .as_ref()
            .map(|titles| {
                titles
                    .iter()
                    .map(|title| title.to_lowercase())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_else(|| Vec::new());

        let query = TaskListQuery {
            legacy_filter,
            states: state_filter.unwrap_or_default(),
            types: type_filter.unwrap_or_default(),
            owner_plan_ids: owner_plan_id_filter.unwrap_or_default(),
            owner_plan_titles: plan_title_filters_lower,
            order_by: order_by.unwrap_or_default(),
            offset,
            limit,
        };

        let (task_ids, total) = self
            .task_db
            .query_task_ids(&query)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = json!({
            "task_list": task_ids,
            "total": total,
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn get_task_info(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let task_id = req.params.get("taskid");
        if task_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "taskid is required".to_string(),
            ));
        }
        let task_id = task_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        let task_info = engine
            .get_task_info(task_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let total_size = self
            .task_db
            .sum_backup_item_sizes(&task_info.checkpoint_id)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let mut result = task_info.to_json_value();
        result["total_size"] = total_size.into();
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn resume_backup_task(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let task_id = req.params.get("taskid");
        if task_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "taskid is required".to_string(),
            ));
        }
        let task_id = task_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        engine
            .resume_work_task(task_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let result = json!({
            "result": "success"
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn pause_backup_task(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let task_id = req.params.get("taskid");
        if task_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "taskid is required".to_string(),
            ));
        }
        let task_id = task_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        engine
            .pause_work_task(task_id)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        let result = json!({
            "result": "success"
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn consume_size_summary(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let total = self
            .task_db
            .sum_all_completed_backup_items_size()
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let now = Local::now();
        let naive_midnight = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| RPCErrors::ReasonError("failed to compute start of day".to_string()))?;
        let today_start = Local
            .from_local_datetime(&naive_midnight)
            .single()
            .ok_or_else(|| {
                RPCErrors::ReasonError("failed to resolve local start of day".to_string())
            })?
            .timestamp_millis();

        let today = self
            .task_db
            .sum_completed_backup_items_size_since(today_start)
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        let result = json!({
            "total": total,
            "today": today,
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn list_directory_children(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let path_value = req.params.get("path");
        let path_opt = match path_value {
            Some(Value::Null) | None => None,
            Some(Value::String(s)) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Some(_) => {
                return Err(RPCErrors::ParseRequestError(
                    "path must be a string if provided".to_string(),
                ));
            }
        };

        let options = match req.params.get("options") {
            Some(Value::Null) | None => ListDirectoryOptions::default(),
            Some(value) => {
                serde_json::from_value::<ListDirectoryOptions>(value.clone()).map_err(|_| {
                    RPCErrors::ParseRequestError(
                        "options must be an object with boolean fields".to_string(),
                    )
                })?
            }
        };

        let entries: Vec<DirectoryEntry> = match path_opt {
            Some(path_str) => {
                let resolved_path = resolve_requested_path(&path_str);
                match fs::metadata(&resolved_path) {
                    Ok(metadata) => {
                        if metadata.is_dir() {
                            list_directory_entries_for_path(&resolved_path)?
                        } else {
                            Vec::new()
                        }
                    }
                    Err(err) => {
                        warn!(
                            "list_directory_children unable to stat path {}: {}",
                            resolved_path.display(),
                            err
                        );
                        Vec::new()
                    }
                }
            }
            None => list_root_children()?,
        };

        let only_dirs = options.only_dirs;
        let only_files = options.only_files;

        let filtered_entries = entries
            .into_iter()
            .filter(|entry| match (only_dirs, only_files) {
                (true, false) => entry.is_directory_like(),
                (false, true) => entry.is_file_like(),
                (true, true) => false,
                _ => true,
            })
            .map(DirectoryEntry::into_child)
            .collect::<Vec<_>>();

        Ok(RPCResponse::new(
            RPCResult::Success(json!(filtered_entries)),
            req.id,
        ))
    }

    async fn validate_path(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let path = req.params.get("path");
        if path.is_none() {
            return Err(RPCErrors::ParseRequestError("path is required".to_string()));
        }
        let path = path.unwrap().as_str().unwrap();
        //is path exist
        let path_exist = Path::new(path).exists();
        let result = json!({
            "path_exist": path_exist
        });
        info!("validate_path: {} -> {}", path, path_exist);
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }

    async fn is_plan_running(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "plan_id is required".to_string(),
            ));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        let is_running = engine.is_plan_have_running_backup_task(plan_id).await;
        let result = json!({
            "is_running": is_running
        });
        Ok(RPCResponse::new(RPCResult::Success(result), req.id))
    }
}

fn parse_order_by(
    value: Option<&Value>,
) -> Result<Option<Vec<(TaskOrderField, SortOrder)>>, RPCErrors> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let array = match raw {
        Value::Array(items) => items,
        Value::Null => return Ok(None),
        _ => {
            return Err(RPCErrors::ParseRequestError(
                "order_by must be an array of [field, order] pairs".to_string(),
            ))
        }
    };
    if array.is_empty() {
        return Ok(None);
    }

    let mut result = Vec::with_capacity(array.len());
    for entry in array {
        let pair = entry.as_array().ok_or_else(|| {
            RPCErrors::ParseRequestError(
                "order_by entries must be [field, order] arrays".to_string(),
            )
        })?;
        if pair.len() != 2 {
            return Err(RPCErrors::ParseRequestError(
                "order_by entries must be [field, order] arrays".to_string(),
            ));
        }
        let field_str = pair[0].as_str().ok_or_else(|| {
            RPCErrors::ParseRequestError("order_by field must be a string".to_string())
        })?;
        let order_str = pair[1].as_str().ok_or_else(|| {
            RPCErrors::ParseRequestError("order_by order must be a string".to_string())
        })?;

        let field = match field_str {
            "create_time" => TaskOrderField::CreateTime,
            "update_time" => TaskOrderField::UpdateTime,
            "complete_time" => TaskOrderField::CompleteTime,
            _ => {
                return Err(RPCErrors::ParseRequestError(format!(
                    "unsupported order_by field: {}",
                    field_str
                )))
            }
        };
        let direction = match order_str {
            "asc" => SortOrder::Asc,
            "desc" => SortOrder::Desc,
            _ => {
                return Err(RPCErrors::ParseRequestError(format!(
                    "unsupported order direction: {}",
                    order_str
                )))
            }
        };
        result.push((field, direction));
    }
    Ok(Some(result))
}

fn parse_task_states(value: &Value) -> Result<Vec<TaskState>, RPCErrors> {
    match value {
        Value::Array(items) => {
            let mut result = Vec::with_capacity(items.len());
            for item in items {
                let state_str = item.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError(
                        "state filter must be an array of strings".to_string(),
                    )
                })?;
                let state = parse_task_state_from_str(state_str).ok_or_else(|| {
                    RPCErrors::ParseRequestError(format!(
                        "unsupported task state filter: {}",
                        state_str
                    ))
                })?;
                result.push(state);
            }
            Ok(result)
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(RPCErrors::ParseRequestError(
            "state filter must be an array of strings".to_string(),
        )),
    }
}

fn parse_task_types(value: &Value) -> Result<Vec<TaskType>, RPCErrors> {
    match value {
        Value::Array(items) => {
            let mut result = Vec::with_capacity(items.len());
            for item in items {
                let type_str = item.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError(
                        "type filter must be an array of strings".to_string(),
                    )
                })?;
                let ty = parse_task_type_from_str(type_str).ok_or_else(|| {
                    RPCErrors::ParseRequestError(format!(
                        "unsupported task type filter: {}",
                        type_str
                    ))
                })?;
                result.push(ty);
            }
            Ok(result)
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(RPCErrors::ParseRequestError(
            "type filter must be an array of strings".to_string(),
        )),
    }
}

fn parse_string_list(value: &Value, field: &str) -> Result<Vec<String>, RPCErrors> {
    match value {
        Value::Array(items) => {
            let mut result = Vec::with_capacity(items.len());
            for item in items {
                let value_str = item.as_str().ok_or_else(|| {
                    RPCErrors::ParseRequestError(format!(
                        "{} filter must be an array of strings",
                        field
                    ))
                })?;
                result.push(value_str.to_string());
            }
            Ok(result)
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(RPCErrors::ParseRequestError(format!(
            "{} filter must be an array of strings",
            field
        ))),
    }
}

fn parse_task_state_from_str(value: &str) -> Option<TaskState> {
    match value {
        "RUNNING" => Some(TaskState::Running),
        "PENDING" => Some(TaskState::Pending),
        "PAUSED" => Some(TaskState::Paused),
        "DONE" => Some(TaskState::Done),
        "FAILED" => Some(TaskState::Failed("".to_string())),
        _ => None,
    }
}

fn parse_task_type_from_str(value: &str) -> Option<TaskType> {
    match value {
        "BACKUP" => Some(TaskType::Backup),
        "RESTORE" => Some(TaskType::Restore),
        _ => None,
    }
}

fn list_root_children() -> Result<Vec<DirectoryEntry>, RPCErrors> {
    #[cfg(windows)]
    {
        Ok(list_windows_drive_roots())
    }
    #[cfg(not(windows))]
    {
        Ok(vec![DirectoryEntry {
            name: "/".to_string(),
            entry_type: DirectoryEntryType::Directory,
            target_type: None,
        }])
    }
}

fn list_directory_entries_for_path(path: &Path) -> Result<Vec<DirectoryEntry>, RPCErrors> {
    let read_dir = fs::read_dir(path).map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
    let mut entries = Vec::new();

    for entry_result in read_dir {
        match entry_result {
            Ok(entry) => {
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(err) => {
                        warn!(
                            "list_directory_children failed to read file type for {}: {}",
                            entry.path().display(),
                            err
                        );
                        continue;
                    }
                };
                let entry_path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                let (entry_type, target_type) =
                    classify_directory_entry(&entry_path, &file_type, path);
                entries.push(DirectoryEntry {
                    name,
                    entry_type,
                    target_type,
                });
            }
            Err(err) => {
                warn!(
                    "list_directory_children failed to read entry in {}: {}",
                    path.display(),
                    err
                );
            }
        }
    }

    entries.sort_by(
        |a, b| match (a.is_directory_like(), b.is_directory_like()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        },
    );
    Ok(entries)
}

const MAX_SYMLINK_DEPTH: usize = 40;

fn classify_directory_entry(
    entry_path: &Path,
    file_type: &fs::FileType,
    parent: &Path,
) -> (DirectoryEntryType, Option<ResolvedTargetType>) {
    if file_type.is_symlink() {
        (
            DirectoryEntryType::Link,
            resolve_link_target_type(entry_path, parent),
        )
    } else if file_type.is_dir() {
        (DirectoryEntryType::Directory, None)
    } else if file_type.is_file() {
        (DirectoryEntryType::File, None)
    } else {
        (DirectoryEntryType::Other, None)
    }
}

fn resolve_link_target_type(entry_path: &Path, parent: &Path) -> Option<ResolvedTargetType> {
    let mut current_path = entry_path.to_path_buf();
    let mut visited = HashSet::new();

    for depth in 0..MAX_SYMLINK_DEPTH {
        if !visited.insert(current_path.clone()) {
            warn!(
                "resolve_link_target_type detected a symlink loop at {} after {} steps",
                entry_path.display(),
                depth
            );
            return None;
        }

        let metadata = match fs::symlink_metadata(&current_path) {
            Ok(metadata) => metadata,
            Err(err) => {
                warn!(
                    "resolve_link_target_type failed to read metadata for {}: {}",
                    current_path.display(),
                    err
                );
                return None;
            }
        };

        if metadata.file_type().is_symlink() {
            let target = match fs::read_link(&current_path) {
                Ok(target) => target,
                Err(err) => {
                    warn!(
                        "resolve_link_target_type failed to read link target for {}: {}",
                        current_path.display(),
                        err
                    );
                    return None;
                }
            };

            current_path = if target.is_absolute() {
                target
            } else {
                let base = current_path.parent().unwrap_or(parent);
                base.join(target)
            };
            continue;
        }

        if metadata.is_dir() {
            return Some(ResolvedTargetType::Directory);
        }
        if metadata.is_file() {
            return Some(ResolvedTargetType::File);
        }
        return None;
    }

    warn!(
        "resolve_link_target_type exceeded maximum depth while resolving {}",
        entry_path.display()
    );
    None
}

fn resolve_requested_path(raw: &str) -> PathBuf {
    #[cfg(windows)]
    {
        resolve_requested_path_windows(raw)
    }
    #[cfg(not(windows))]
    {
        resolve_requested_path_unix(raw)
    }
}

#[cfg(windows)]
fn resolve_requested_path_windows(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PathBuf::from("\\");
    }

    let mut normalized = trimmed.replace('/', "\\");
    if normalized == "\\" {
        return PathBuf::from("\\");
    }
    if normalized.len() == 2 && normalized.ends_with(':') {
        normalized.push('\\');
        return PathBuf::from(normalized);
    }
    if !normalized.contains(':') && !normalized.starts_with('\\') {
        normalized = format!("\\{}", normalized);
    }
    while normalized.len() > 3 && normalized.ends_with('\\') {
        normalized.pop();
    }
    PathBuf::from(normalized)
}

#[cfg(not(windows))]
fn resolve_requested_path_unix(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return PathBuf::from("/");
    }

    let mut buf = PathBuf::from("/");
    for segment in trimmed
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty())
    {
        buf.push(segment);
    }
    buf
}

#[cfg(windows)]
fn list_windows_drive_roots() -> Vec<DirectoryEntry> {
    let mut drives = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if Path::new(&drive).is_dir() {
            drives.push(DirectoryEntry {
                name: format!("{}:", letter as char),
                entry_type: DirectoryEntryType::Directory,
                target_type: None,
            });
        }
    }
    drives.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    drives
}

#[async_trait]
impl InnerServiceHandler for WebControlServer {
    async fn handle_rpc_call(
        &self,
        req: RPCRequest,
        ip_from: IpAddr,
    ) -> Result<RPCResponse, RPCErrors> {
        match req.method.as_str() {
            "create_backup_plan" => self.create_backup_plan(req).await,
            "list_backup_plan" => self.list_backup_plan(req).await,
            "get_backup_plan" => self.get_backup_plan(req).await,
            "create_backup_target" => self.create_backup_target(req).await,
            "list_backup_target" => self.list_backup_target(req).await,
            "get_backup_target" => self.get_backup_target(req).await,
            "update_backup_target" => self.update_backup_target(req).await,
            "remove_backup_target" => self.remove_backup_target(req).await,
            "create_backup_task" => self.create_backup_task(req).await,
            "create_restore_task" => self.create_restore_task(req).await,
            "get_task_info" => self.get_task_info(req).await,
            "resume_backup_task" => self.resume_backup_task(req).await,
            "pause_backup_task" => self.pause_backup_task(req).await,
            "consume_size_summary" => self.consume_size_summary(req).await,
            "list_files_in_task" => self.list_files_in_task(req).await,
            "list_backup_task" => self.list_backup_task(req).await,
            "list_directory_children" => self.list_directory_children(req).await,
            "validate_path" => self.validate_path(req).await,
            "is_plan_running" => self.is_plan_running(req).await,
            _ => Err(RPCErrors::UnknownMethod(req.method)),
        }
    }

    async fn handle_http_get(&self, req_path: &str, ip_from: IpAddr) -> Result<String, RPCErrors> {
        unimplemented!()
    }
}

pub async fn start_web_control_service() {
    let web_control_server = WebControlServer::new();
    //register WebControlServer  as inner service
    register_inner_service_builder("backup_control", move || {
        Box::new(web_control_server.clone())
    })
    .await;
    let web_root_dir = get_buckyos_system_bin_dir()
        .join("backup_suite")
        .join("webui");

    let web_control_server_config = json!({
      "tls_port":5143,
      "http_port":5180,
      "hosts": {
        "*": {
          "enable_cors":true,
          "routes": {
            "/": {
              "local_dir": web_root_dir.to_str().unwrap()
            },
            "/kapi/backup_control" : {
                "inner_service":"backup_control"
            }
          }
        }
      }
    });

    let web_control_server_config: WarpServerConfig =
        serde_json::from_value(web_control_server_config).unwrap();
    //start!
    info!("start BackupSuite web control service...");
    let _ = start_cyfs_warp_server(web_control_server_config).await;
}
