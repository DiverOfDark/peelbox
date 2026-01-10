//! Go FileMode bit constants for fsutil protocol compatibility
//!
//! Go's os.FileMode format:
//! - Directories: 0x80000000 | perms (bit 31 set)
//! - Symlinks: 0x08000000 | perms (bit 27 set)
//! - Regular files: just perms (no special bits)

/// Directory bit in Go FileMode (bit 31)
pub const GO_MODE_DIR: u32 = 0x80000000;

/// Symlink bit in Go FileMode (bit 27)
pub const GO_MODE_SYMLINK: u32 = 0x08000000;

/// Mask for file type bits
pub const GO_MODE_TYPE_MASK: u32 = GO_MODE_DIR | GO_MODE_SYMLINK;

/// Check if mode represents a regular file (not dir or symlink)
pub fn is_regular_file(mode: u32, is_dir: bool, has_linkname: bool) -> bool {
    (mode & GO_MODE_TYPE_MASK) == 0 && !is_dir && !has_linkname
}
