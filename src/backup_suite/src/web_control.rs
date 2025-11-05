#![allow(unused)]
use crate::engine::*;
use crate::task_db::{BackupPlanConfig, TaskState, TaskType};
use ::kRPC::*;
use async_trait::async_trait;
use buckyos_backup_lib::RestoreConfig;
use buckyos_kit::get_buckyos_system_bin_dir;
use cyfs_gateway_lib::*;
use cyfs_warp::*;
use log::*;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::result::Result;

#[derive(Clone)]
struct WebControlServer {}

impl WebControlServer {
    fn new() -> Self {
        Self {}
    }

    async fn create_backup_plan(&self, req: RPCRequest) -> Result<RPCResponse, RPCErrors> {
        let source_type = req.params.get("source_type");
        let source_url = req.params.get("source");
        let target_type = req.params.get("target_type");
        let target_url = req.params.get("target");
        let title = req.params.get("title");
        let description = req.params.get("description");
        let type_str = req.params.get("type_str");

        if type_str.is_none()
            || source_type.is_none()
            || source_url.is_none()
            || target_type.is_none()
            || target_url.is_none()
        {
            return Err(RPCErrors::ParseRequestError(
                "type_str, source_type, source_url, target_type, target_url are required"
                    .to_string(),
            ));
        }

        let type_str = type_str.unwrap().as_str().unwrap();
        let source_type = source_type.unwrap().as_str().unwrap();
        let source_url = source_url.unwrap().as_str().unwrap();
        let target_type = target_type.unwrap().as_str().unwrap();
        let target_url = target_url.unwrap().as_str().unwrap();

        if title.is_none() || description.is_none() {
            return Err(RPCErrors::ParseRequestError(
                "title, description are required".to_string(),
            ));
        }

        let title = title.unwrap().as_str().unwrap();
        let description = description.unwrap().as_str().unwrap();
        let plan_id: String;
        let engine = DEFAULT_ENGINE.lock().await;
        match type_str {
            "c2c" => {
                let new_plan =
                    BackupPlanConfig::chunk2chunk(source_url, target_url, title, description);
                plan_id = engine
                    .create_backup_plan(new_plan)
                    .await
                    .map_err(|e| RPCErrors::ReasonError(e.to_string()))?;
            }
            _ => {
                return Err(RPCErrors::ParseRequestError(format!(
                    "unknown type_str: {}",
                    type_str
                )));
            }
        }

        let result = json!({
            "plan_id": plan_id
        });
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
            "create_backup_task" => self.create_backup_task(req).await,
            "create_restore_task" => self.create_restore_task(req).await,
            "get_task_info" => self.get_task_info(req).await,
            "resume_backup_task" => self.resume_backup_task(req).await,
            "pause_backup_task" => self.pause_backup_task(req).await,
            "list_backup_task" => self.list_backup_task(req).await,
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
