//! See [`Backend`] for the primary entry point.
use std::{collections::HashSet, fmt::Display, hash::Hash};

use anyhow::Result;

use crate::snapshot::{PruneFrom, Snapshot};

/// The primary configuration that nomad works off.
#[derive(Debug)]
pub struct Config {
    /// Represents "who" a given branch belongs to. This value should be shared by multiple git
    /// clones that belong to the same user.
    ///
    /// This string is used when pushing branches to the remote so that multiple users can use
    /// nomad on that remote without stepping on each other.
    pub user: String,

    /// Represents "where" a given branch comes from. This value should be unique for every git
    /// clone belonging to a specific user.
    ///
    /// This string is used when pushing branches to the remote so that multiple hosts belonging to
    /// the same user can co-exist (i.e. the whole point of nomad).
    ///
    /// This string is also used when pulling branches for all hosts of the current user
    /// and for detecting when branches have been deleted.
    pub host: String,
}

/// A remote git repository identified by name, like `origin`.
pub struct Remote(pub String);

/// The branch name part of a ref. `refs/head/master` would be `Branch("master".to_string())`.
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Branch(pub String);

impl Branch {
    pub fn str<S: AsRef<str>>(s: S) -> Branch {
        Branch(s.as_ref().to_string())
    }
}

/// A ref representing a branch managed by nomad.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct NomadRef<Ref> {
    /// The user this branch belongs to.
    pub user: String,
    /// The host this branch comes from.
    pub host: String,
    /// The branch name.
    pub branch: Branch,
    /// Any additional data the [`Backend`] would like to carry around.
    pub ref_: Ref,
}

/// An abstraction point between the high level operation of nomad ("synchronize git branches")
/// with the low level implementation ("invoke a git binary").
pub trait Backend {
    /// Any additional information the backend would like to carry about a nomad managed ref.
    type Ref: Display + Eq + Hash;

    /// Extract the persistent nomad [`Config`] for this git clone.
    fn read_config(&self) -> Result<Option<Config>>;

    /// Persist a new [`Config`] for this git clone.
    fn write_config(&self, config: &Config) -> Result<()>;

    /// Build a point in time snapshot for all refs that nomad cares about.
    fn snapshot(&self, config: &Config) -> Result<Snapshot<Self::Ref>>;

    /// Fetch all nomad managed refs from a given remote.
    fn fetch(&self, config: &Config, remote: &Remote) -> Result<HashSet<NomadRef<Self::Ref>>>;

    /// Push local branches to nomad managed refs in the remote.
    fn push(&self, config: &Config, remote: &Remote) -> Result<()>;

    /// Prune the given nomad managed refs from both the local and remote clones.
    fn prune<'a, Prune>(&self, remote: &Remote, prune: Prune) -> Result<()>
    where
        Self::Ref: 'a,
        Prune: Iterator<Item = &'a PruneFrom<Self::Ref>>;
}
