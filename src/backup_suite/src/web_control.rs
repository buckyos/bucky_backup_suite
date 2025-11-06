#![allow(unused)]
use crate::engine::*;
use crate::task_db::{
    BackupDbError, BackupPlanConfig, BackupTargetRecord, BackupTaskDb, TaskState, TaskType,
};
use ::kRPC::*;
use async_trait::async_trait;
use buckyos_backup_lib::RestoreConfig;
use buckyos_kit::{get_buckyos_service_data_dir, get_buckyos_system_bin_dir};
use cyfs_gateway_lib::*;
use cyfs_warp::*;
use log::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashMap;
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
                    target_record.url.as_str(),
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

        // Create plan via engine
        let engine = DEFAULT_ENGINE.lock().await;
        let last_checkpoint_index = plan_config.last_checkpoint_index;
        let plan_id = engine
            .create_backup_plan(plan_config)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
        drop(engine);
        let now = chrono::Utc::now().timestamp_millis();

        let result = json!({
            "plan_id": plan_id,
            "type_str": type_str,
            "source_type": source_type,
            "source": display_source,
            "target_type": target_record.target_type,
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
        let mut result = plan.to_json_value();
        let is_running = engine.is_plan_have_running_backup_task(plan_id).await;
        result["is_running"] = json!(is_running);
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
        let target_url = target_url_value
            .unwrap()
            .as_str()
            .ok_or_else(|| RPCErrors::ParseRequestError("url must be a string".to_string()))?;
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
            target_url,
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
        let mut use_advanced_filter = false;

        if let Some(filter_value) = filter_value {
            match filter_value {
                Value::Null => {}
                Value::String(s) => {
                    if !s.is_empty() {
                        legacy_filter = Some(s.clone());
                    }
                }
                Value::Object(obj) => {
                    use_advanced_filter = true;
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

        if order_by.is_some() {
            use_advanced_filter = true;
        }

        let filter_str = legacy_filter.as_deref().unwrap_or("");

        let engine = DEFAULT_ENGINE.lock().await;
        let mut task_ids = engine
            .list_backup_tasks(filter_str)
            .await
            .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;

        if !use_advanced_filter {
            let total = task_ids.len();
            let limit_value = limit.unwrap_or(usize::MAX);
            let task_ids = task_ids
                .into_iter()
                .skip(offset)
                .take(limit_value)
                .collect::<Vec<_>>();
            let result = json!({
                "task_list": task_ids,
                "total": total,
            });
            return Ok(RPCResponse::new(RPCResult::Success(result), req.id));
        }

        let plan_title_filters_lower = owner_plan_title_filter.as_ref().map(|titles| {
            titles
                .iter()
                .map(|title| title.to_lowercase())
                .collect::<Vec<String>>()
        });
        let mut plan_title_cache: HashMap<String, String> = HashMap::new();
        let mut tasks = Vec::with_capacity(task_ids.len());

        for task_id in &task_ids {
            let task = engine
                .get_task_info(task_id)
                .await
                .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
            if plan_title_filters_lower.is_some()
                && !plan_title_cache.contains_key(&task.owner_plan_id)
            {
                match engine.get_backup_plan(&task.owner_plan_id).await {
                    Ok(plan) => {
                        plan_title_cache
                            .insert(task.owner_plan_id.clone(), plan.title.to_lowercase());
                    }
                    Err(_) => {
                        plan_title_cache.insert(task.owner_plan_id.clone(), String::new());
                    }
                }
            }
            tasks.push(task);
        }
        drop(task_ids);
        drop(engine);

        let filtered_tasks: Vec<_> = tasks
            .into_iter()
            .filter(|task| {
                if let Some(states) = &state_filter {
                    if !states.iter().any(|state| *state == task.state) {
                        return false;
                    }
                }
                if let Some(types) = &type_filter {
                    if !types.iter().any(|ty| *ty == task.task_type) {
                        return false;
                    }
                }
                if let Some(plan_ids) = &owner_plan_id_filter {
                    if !plan_ids.iter().any(|plan| plan == &task.owner_plan_id) {
                        return false;
                    }
                }
                if let Some(title_filters) = &plan_title_filters_lower {
                    if let Some(plan_title) = plan_title_cache.get(&task.owner_plan_id) {
                        if !title_filters
                            .iter()
                            .any(|needle| plan_title.contains(needle))
                        {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            })
            .collect();

        let mut filtered_tasks = filtered_tasks;
        if let Some(order_rules) = order_by.as_ref() {
            filtered_tasks.sort_by(|a, b| {
                for (field, direction) in order_rules {
                    let mut cmp = match field {
                        TaskOrderField::CreateTime => a.create_time.cmp(&b.create_time),
                        TaskOrderField::UpdateTime => a.update_time.cmp(&b.update_time),
                        TaskOrderField::CompleteTime => {
                            match (a.state == TaskState::Done, b.state == TaskState::Done) {
                                (true, true) => a.update_time.cmp(&b.update_time),
                                (true, false) => Ordering::Greater,
                                (false, true) => Ordering::Less,
                                (false, false) => Ordering::Equal,
                            }
                        }
                    };
                    if *direction == SortOrder::Desc {
                        cmp = cmp.reverse();
                    }
                    if cmp != Ordering::Equal {
                        return cmp;
                    }
                }
                Ordering::Equal
            });
        }

        let total = filtered_tasks.len();
        let selected_task_ids: Vec<String> = filtered_tasks
            .into_iter()
            .skip(offset)
            .take(limit.unwrap_or(usize::MAX))
            .map(|task| task.taskid)
            .collect();

        let result = json!({
            "task_list": selected_task_ids,
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
        let result = task_info.to_json_value();
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

        let entries: Vec<DirectoryChild> = match path_opt {
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

        Ok(RPCResponse::new(RPCResult::Success(json!(entries)), req.id))
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

#[derive(Clone, Copy)]
enum TaskOrderField {
    CreateTime,
    UpdateTime,
    CompleteTime,
}

#[derive(Clone, Copy, PartialEq)]
enum SortOrder {
    Asc,
    Desc,
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
        "FAILED" => Some(TaskState::Failed),
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

fn list_root_children() -> Result<Vec<DirectoryChild>, RPCErrors> {
    #[cfg(windows)]
    {
        Ok(list_windows_drive_roots())
    }
    #[cfg(not(windows))]
    {
        list_directory_entries_for_path(Path::new("/"))
    }
}

fn list_directory_entries_for_path(path: &Path) -> Result<Vec<DirectoryChild>, RPCErrors> {
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
                let name = entry.file_name().to_string_lossy().into_owned();
                entries.push(DirectoryChild {
                    name,
                    is_directory: file_type.is_dir(),
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

    entries.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
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
fn list_windows_drive_roots() -> Vec<DirectoryChild> {
    let mut drives = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if Path::new(&drive).is_dir() {
            drives.push(DirectoryChild {
                name: format!("{}:", letter as char),
                is_directory: true,
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
