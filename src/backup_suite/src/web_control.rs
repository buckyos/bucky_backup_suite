#![allow(unused)]
use ::kRPC::*;
use async_trait::async_trait;
use cyfs_gateway_lib::*;
use cyfs_warp::*;
use serde_json::{Value,json};
use log::*;
use std::result::Result;
use std::net::IpAddr;

#[derive(Clone)]
struct WebControlServer {

}

impl WebControlServer {
    fn new()->Self{
        Self{}
    }
}

#[async_trait]
impl kRPCHandler for WebControlServer {
    async fn handle_rpc_call(&self, req:RPCRequest,ip_from:IpAddr) -> Result<RPCResponse,RPCErrors> {
        unimplemented!()
    }
}

pub async fn start_web_control_service() {
    let web_control_server = WebControlServer::new();
    //register WebControlServer  as inner service
    register_inner_service_builder("backup_control", move || {  
        Box::new(web_control_server.clone())
    }).await;
    let web_control_dir = format!("./web_control");
    let web_control_server_config = json!({
      "tls_port":5143,
      "http_port":5180,
      "hosts": {
        "*": {
          "enable_cors":true,
          "routes": {
            "/": {
              "local_dir": web_control_dir
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
