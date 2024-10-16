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
pub mod fs;
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
pub use format_bulleted_list::format_bulleted_list_multiline;
pub use git::repository_url_destination;
pub use git::AddWorktreeOpts;
pub use git::BranchRef;
pub use git::CommitHash;
pub use git::Git;
pub use git::GitBranch;
pub use git::GitConfig;
pub use git::GitPath;
pub use git::GitRefs;
pub use git::GitRemote;
pub use git::GitStatus;
pub use git::GitWorktree;
pub use git::HeadKind;
pub use git::LocalBranchRef;
pub use git::Ref;
pub use git::RemoteBranchRef;
pub use git::RenamedWorktree;
pub use git::ResolveUniqueNameOpts;
pub use git::ResolvedCommitish;
pub use git::Status;
pub use git::StatusCode;
pub use git::StatusEntry;
pub use git::Worktree;
pub use git::WorktreeHead;
pub use git::Worktrees;
pub use normal_path::NormalPath;
pub use utf8tempdir::Utf8TempDir;
