//! `git-prole` is a `git worktree` manager.
//!
//! The `git-prole` Rust library is a convenience and shouldn't be depended on. I do not
//! consider this to be a public/stable API and will make breaking changes here in minor version
//! bumps. If you'd like a stable `git-prole` Rust API for some reason, let me know and we can maybe
//! work something out.

mod add;
mod app;
mod app_git;
mod cli;
mod clone;
mod config;
mod convert;
mod copy_dir;
mod current_dir;
mod format_bulleted_list;
mod gh;
mod git;
mod install_tracing;
mod normal_path;
mod parse;
mod topological_sort;
mod utf8tempdir;

pub use app::App;
pub use app_git::AppGit;
pub use config::Config;
pub use format_bulleted_list::format_bulleted_list;
pub use git::Git;
pub use normal_path::NormalPath;
pub use utf8tempdir::Utf8TempDir;
