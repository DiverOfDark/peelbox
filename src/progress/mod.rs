//! Progress reporting for detection operations

mod handler;
mod logging;

pub use handler::{NoOpHandler, ProgressEvent, ProgressHandler};
pub use logging::LoggingHandler;
