//! Usage: Skills domain (repositories, installed skills, local import, and CLI integration).

mod discover;
mod fs_ops;
mod git_url;
mod installed;
mod local;
mod ops;
mod paths;
mod repo_cache;
mod repos;
mod skill_md;
mod types;
mod util;

pub use discover::discover_available;
pub use installed::installed_list;
pub use local::{import_local, local_list};
pub use ops::{install, set_enabled, uninstall};
pub use paths::paths_get;
pub use repos::{repo_delete, repo_upsert, repos_list};
pub use types::{
    AvailableSkillSummary, InstalledSkillSummary, LocalSkillSummary, SkillRepoSummary, SkillsPaths,
};

#[cfg(test)]
mod tests;
