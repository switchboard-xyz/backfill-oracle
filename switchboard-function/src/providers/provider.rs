use async_trait::async_trait;

#[async_trait]
pub trait DataProvider {
    async fn watch(&self, routine_interval: Option<u64>);
}
