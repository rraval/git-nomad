//! See [`Backend`] for the primary entry point.
use std::collections::HashSet;

use anyhow::Result;

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
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Branch(pub String);

impl Branch {
    pub fn str<S: AsRef<str>>(s: S) -> Branch {
        Branch(s.as_ref().to_string())
    }
}

/// A nomad managed ref representing a branch for the current host, where "current" is relative to
/// whatever [`Config`] was passed in.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct HostBranch<Ref> {
    pub branch: Branch,
    pub ref_: Ref,
}

/// A point in time view of branches we care about.
pub struct Snapshot<Ref> {
    /// The active branches in this clone that the user manipulates directly with `git branch` etc.
    pub local_branches: HashSet<Branch>,
    /// The refs that nomad manages to follow the local branches.
    pub host_branches: HashSet<HostBranch<Ref>>,
}

impl<Ref> Snapshot<Ref> {
    /// Find nomad host branches that can be pruned because the local branch they were based on no
    /// longer exists.
    pub fn prune(&self) -> impl Iterator<Item = &HostBranch<Ref>> {
        self.host_branches
            .iter()
            .filter(move |hb| !self.local_branches.contains(&hb.branch))
    }
}

/// An abstraction point between the high level operation of nomad ("synchronize git branches")
/// with the low level implementation ("invoke a git binary").
pub trait Backend {
    /// Any additional information the backend would like to carry about a nomad managed ref.
    type Ref;

    /// Extract the persistent nomad [`Config`] for this git clone.
    fn read_config(&self) -> Result<Option<Config>>;

    /// Persist a new [`Config`] for this git clone.
    fn write_config(&self, config: &Config) -> Result<()>;

    /// Fetch refs from a git remote and produce a point in time [`Snapshot`].
    fn fetch(&self, config: &Config, remote: &Remote) -> Result<Snapshot<Self::Ref>>;

    /// Push local branches to nomad managed refs in the remote.
    fn push(&self, config: &Config, remote: &Remote) -> Result<()>;

    /// Prune the given nomad managed refs from both the local and remote clones.
    fn prune<'a, Prune>(&self, config: &Config, remote: &Remote, prune: Prune) -> Result<()>
    where
        Self::Ref: 'a,
        Prune: Iterator<Item = &'a HostBranch<Self::Ref>>;
}
