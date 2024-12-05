#![allow(unused)]
use buckyos_kit::get_buckyos_system_bin_dir;
use ::kRPC::*;
use async_trait::async_trait;
use cyfs_gateway_lib::*;
use cyfs_warp::*;
use serde_json::{Value,json};
use log::*;
use std::path::Path;
use std::result::Result;
use std::net::IpAddr;
use crate::engine::*;
use crate::task_db::BackupPlanConfig;

#[derive(Clone)]
struct WebControlServer {

}

impl WebControlServer {
    fn new()->Self{
        Self{}
    }

    async fn create_backup_plan(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let source_type = req.params.get("source_type");
        let source_url = req.params.get("source");
        let target_type = req.params.get("target_type");
        let target_url = req.params.get("target");
        let title = req.params.get("title");
        let description = req.params.get("description");
        let type_str = req.params.get("type_str");

        if type_str.is_none() || source_type.is_none() || source_url.is_none() || target_type.is_none() || target_url.is_none() {
            return Err(RPCErrors::ParseRequestError("type_str, source_type, source_url, target_type, target_url are required".to_string()));
        }

        let type_str = type_str.unwrap().as_str().unwrap();
        let source_type = source_type.unwrap().as_str().unwrap();
        let source_url = source_url.unwrap().as_str().unwrap();
        let target_type = target_type.unwrap().as_str().unwrap();
        let target_url = target_url.unwrap().as_str().unwrap();

        if title.is_none() || description.is_none() {
            return Err(RPCErrors::ParseRequestError("title, description are required".to_string()));
        }

        let title = title.unwrap().as_str().unwrap();
        let description = description.unwrap().as_str().unwrap();
        let plan_id : String;
        let engine = DEFAULT_ENGINE.lock().await;
        match type_str {
            "c2c" => {
                let new_plan = BackupPlanConfig::chunk2chunk(source_url, target_url, title, description);
                plan_id = engine.create_backup_plan(new_plan).await
                    .map_err(|e| {
                        RPCErrors::ReasonError(e.to_string())
                    })?;
            }
            _ => {
                return Err(RPCErrors::ParseRequestError(format!("unknown type_str: {}", type_str)));
            }
        }

        let result = json!({
            "plan_id": plan_id
        });
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    async fn list_backup_plan(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let engine = DEFAULT_ENGINE.lock().await;
        let plans = engine.list_backup_plans().await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;

        let result = json!({
            "backup_plans": plans
        });
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    async fn get_backup_plan(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError("plan_id is required".to_string()));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        let plan = engine.get_backup_plan(plan_id).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;
        let mut result = plan.to_json_value();
        let is_running = engine.is_plan_have_running_backup_task(plan_id).await;
        result["is_running"] = json!(is_running);
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    //return the new task info
    async fn create_backup_task(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError("plan_id is required".to_string()));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let parent_checkpoint_id = req.params.get("parent_checkpoint_id");
        let real_parent_checkpoint_id = if parent_checkpoint_id.is_some() {
            Some(parent_checkpoint_id.unwrap().as_str().unwrap())
        } else {
            None
        };
        let engine = DEFAULT_ENGINE.lock().await;
        let task_id = engine.create_backup_task(plan_id, real_parent_checkpoint_id).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;

        let task_info = engine.get_task_info(&task_id).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;

        let result = task_info.to_json_value();
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }


    async fn list_backup_task(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let filter = req.params.get("filter");
        let filter_str = if filter.is_some() {
            filter.unwrap().as_str().unwrap()
        } else {
            ""
        };

        let engine = DEFAULT_ENGINE.lock().await;
        //task id list
        let result_task_list : Vec<String>;
        result_task_list = engine.list_backup_tasks(filter_str).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;
       
        let result = json!({
            "task_list": result_task_list
        });
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    async fn get_task_info(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let task_id = req.params.get("taskid");
        if task_id.is_none() {
            return Err(RPCErrors::ParseRequestError("taskid is required".to_string()));
        }
        let task_id = task_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        let task_info = engine.get_task_info(task_id).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;
        let result = task_info.to_json_value();
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    async fn resume_backup_task(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let task_id = req.params.get("taskid");
        if task_id.is_none() {
            return Err(RPCErrors::ParseRequestError("taskid is required".to_string()));
        }
        let task_id = task_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        engine.resume_work_task(task_id).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;
        let result = json!({
            "result": "success"
        });
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }
    
    async fn pause_backup_task(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let task_id = req.params.get("taskid");
        if task_id.is_none() {
            return Err(RPCErrors::ParseRequestError("taskid is required".to_string()));
        }
        let task_id = task_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        engine.pause_work_task(task_id).await
            .map_err(|e| {
                RPCErrors::ReasonError(e.to_string())
            })?;
        let result = json!({
            "result": "success"
        });
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    async fn validate_path(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
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
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }

    async fn is_plan_running(&self, req:RPCRequest) -> Result<RPCResponse,RPCErrors> {
        let plan_id = req.params.get("plan_id");
        if plan_id.is_none() {
            return Err(RPCErrors::ParseRequestError("plan_id is required".to_string()));
        }
        let plan_id = plan_id.unwrap().as_str().unwrap();
        let engine = DEFAULT_ENGINE.lock().await;
        let is_running = engine.is_plan_have_running_backup_task(plan_id).await;
        let result = json!({
            "is_running": is_running
        });
        Ok(RPCResponse::new(RPCResult::Success(result),req.seq))
    }
}

#[async_trait]
impl kRPCHandler for WebControlServer {
    async fn handle_rpc_call(&self, req:RPCRequest,ip_from:IpAddr) -> Result<RPCResponse,RPCErrors> {
        match req.method.as_str() {
            "create_backup_plan" => self.create_backup_plan(req).await,
            "list_backup_plan" => self.list_backup_plan(req).await,
            "get_backup_plan" => self.get_backup_plan(req).await,
            "create_backup_task" => self.create_backup_task(req).await,
            "get_task_info" => self.get_task_info(req).await,
            "resume_backup_task" => self.resume_backup_task(req).await,
            "pause_backup_task" => self.pause_backup_task(req).await,
            "list_backup_task" => self.list_backup_task(req).await,
            "validate_path" => self.validate_path(req).await,
            "is_plan_running" => self.is_plan_running(req).await,
            _ => Err(RPCErrors::UnknownMethod(req.method)),
        }
    }
}

pub async fn start_web_control_service() {
    let web_control_server = WebControlServer::new();
    //register WebControlServer  as inner service
    register_inner_service_builder("backup_control", move || {  
        Box::new(web_control_server.clone())
    }).await;
    let web_root_dir = get_buckyos_system_bin_dir().join("backup_suite").join("webui");
    
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

    let web_control_server_config:WarpServerConfig = serde_json::from_value(web_control_server_config).unwrap();
    //start!
    info!("start BackupSuite web control service...");
    let _ = start_cyfs_warp_server(web_control_server_config).await;
}
