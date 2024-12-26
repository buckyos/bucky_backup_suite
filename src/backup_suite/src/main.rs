mod engine;
mod task_db;
mod web_control;
mod work_task;

pub use engine::*;
use web_control::*;
use buckyos_kit::*;
use log::*;

#[tokio::main]
async fn main() {
    init_logging("backup_suite");
    info!("backup suite start");
    let engine = DEFAULT_ENGINE.lock().await;
    engine.start().await.unwrap();
    drop(engine);
    info!("backup engine start ok,start web control service");
    start_web_control_service().await;
}

