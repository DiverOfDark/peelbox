//! FileSystem abstraction for testable file operations

mod mock;
mod real;
mod r#trait;

pub use mock::MockFileSystem;
pub use r#trait::{DirEntry, FileMetadata, FileSystem, FileType};
pub use real::RealFileSystem;
