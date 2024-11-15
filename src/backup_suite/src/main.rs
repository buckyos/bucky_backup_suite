mod engine;
mod task_db;
mod web_control;

use engine::*;
use web_control::*;
use buckyos_kit::*;
use log::*;

#[tokio::main]
async fn main() {
    init_logging("backup_suite");
    info!("backup suite start");
    let engine = BackupEngine::new();
    engine.start().await.unwrap();
    info!("backup engine start ok,start web control service");
    start_web_control_service().await;
}

