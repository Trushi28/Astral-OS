//src/fs/mod.rs
pub mod vfs;
pub mod psychicfs;

pub use vfs::{
    Filesystem, VfsError, VfsResult, FileType, FileAttr, DirEntry,
    OpenFlags, Permissions, FileHandle, FsStats,
    mount, unmount, open, read, write, create, unlink, mkdir, readdir, stat, sync_all,
};
pub use psychicfs::*;