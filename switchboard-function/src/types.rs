#[derive(Default, Clone, Debug)]
pub enum WorkerStatus {
    #[default]
    Initializing,
    Ready,
}
