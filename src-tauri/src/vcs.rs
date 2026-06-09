//! VcsProvider: repository and collaboration operations.
//! Target: `git2` plus a GitHub API client (v0.5+). Kept behind a trait so a
//! Forgejo/Tangled provider can be added later.

use crate::error::WfError;

pub trait VcsProvider {
    fn current_branch(&self) -> Result<String, WfError>;
    fn status(&self) -> Result<(), WfError>;
    fn commit(&self, message: &str) -> Result<(), WfError>;
    fn push(&self) -> Result<(), WfError>;
    fn pull(&self) -> Result<(), WfError>;
}
