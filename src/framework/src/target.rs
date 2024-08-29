#[async_trait::async_trait]
pub trait Target {
    async fn classify() -> String;
}
