mod engine;
mod task_db;
mod web_control;
mod work_task;

use buckyos_kit::*;
pub use engine::*;
use log::*;
use web_control::*;

#[tokio::main]
async fn main() {
    init_logging("backup_suite");
    info!("backup suite start");
    let engine = DEFAULT_ENGINE.lock().await;
    engine.start().await.unwrap();
    drop(engine);
    info!("backup engine start ok,start web control service");
    start_web_control_service().await;

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await
    }
}
