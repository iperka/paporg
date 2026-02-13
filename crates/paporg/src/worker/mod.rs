pub mod job;
pub mod multi_scanner;
pub mod pool;
pub mod scanner;

pub use job::{EmailMetadata, Job, JobResult};
pub use multi_scanner::MultiSourceScanner;
pub use pool::WorkerPool;
pub use scanner::DirectoryScanner;

// Re-export crossbeam_channel for use in main
pub use crossbeam_channel;
