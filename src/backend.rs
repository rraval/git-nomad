//! See [`Backend`] for the primary entry point.
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

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
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Branch(pub String);

impl Branch {
    pub fn str<S: AsRef<str>>(s: S) -> Branch {
        Branch(s.as_ref().to_string())
    }
}

/// A nomad managed ref for the current user.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct HostBranch<Ref: Display> {
    /// The host this branch comes from.
    pub host: String,
    /// The branch name.
    pub branch: Branch,
    /// Any additional data the [`Backend`] would like to carry around.
    pub ref_: Ref,
}

/// A point in time view of refs we care about.
pub struct Snapshot<Ref: Display> {
    /// The active branches in this clone that the user manipulates directly with `git branch` etc.
    pub local_branches: HashSet<Branch>,
    /// The refs that nomad manages to follow the local branches.
    pub host_branches: Vec<HostBranch<Ref>>,
}

impl<Ref: Display> Snapshot<Ref> {
    /// Find nomad host branches that can be pruned because the local branch they were based on no
    /// longer exists.
    pub fn prune(self, config: &Config) -> Vec<HostBranch<Ref>> {
        let Self {
            mut host_branches,
            local_branches,
        } = self;
        host_branches.retain(|hb| hb.host == config.host && !local_branches.contains(&hb.branch));
        host_branches
    }

    /// Return all [`HostBranch`]s grouped by host in sorted order.
    pub fn sorted_hosts_and_branches(self) -> Vec<(String, Vec<HostBranch<Ref>>)> {
        let mut by_host = HashMap::<String, Vec<HostBranch<Ref>>>::new();
        let Self { host_branches, .. } = self;

        for hb in host_branches {
            by_host
                .entry(hb.host.clone())
                .or_insert_with(Vec::new)
                .push(hb);
        }

        let mut as_vec = by_host
            .into_iter()
            .map(|(host, mut branches)| {
                branches.sort_by(|a, b| a.branch.cmp(&b.branch));
                (host, branches)
            })
            .collect::<Vec<_>>();
        as_vec.sort_by(|(host_a, _), (host_b, _)| host_a.cmp(host_b));

        as_vec
    }

    /// Return only the nomad managed branch names for a given host.
    #[cfg(test)]
    pub fn branches_for_host(&self, config: &Config) -> Vec<Branch> {
        self.host_branches
            .iter()
            .filter(|hb| hb.host == config.host)
            .map(|hb| hb.branch.clone())
            .collect()
    }
}

/// An abstraction point between the high level operation of nomad ("synchronize git branches")
/// with the low level implementation ("invoke a git binary").
pub trait Backend {
    /// Any additional information the backend would like to carry about a nomad managed ref.
    type Ref: Display;

    /// Extract the persistent nomad [`Config`] for this git clone.
    fn read_config(&self) -> Result<Option<Config>>;

    /// Persist a new [`Config`] for this git clone.
    fn write_config(&self, config: &Config) -> Result<()>;

    /// Build a point in time snapshot for all refs that nomad cares about.
    fn snapshot(&self) -> Result<Snapshot<Self::Ref>>;

    /// Fetch all nomad managed refs from a given remote.
    fn fetch(&self, config: &Config, remote: &Remote) -> Result<()>;

    /// Push local branches to nomad managed refs in the remote.
    fn push(&self, config: &Config, remote: &Remote) -> Result<()>;

    /// Prune the given nomad managed refs from both the local and remote clones.
    fn prune<'a, Prune>(&self, config: &Config, remote: &Remote, prune: Prune) -> Result<()>
    where
        Self::Ref: 'a,
        Prune: Iterator<Item = &'a HostBranch<Self::Ref>>;
}

#[cfg(test)]
mod tests {
    use std::{fmt, iter};

    use crate::backend::{Config, HostBranch};

    use super::{Branch, Snapshot};

    #[derive(Debug, PartialEq, Eq)]
    struct Ref;

    impl fmt::Display for Ref {
        fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("Not expected to be called")
        }
    }

    /// [`Snapshot::prune`] should only remote branches for the current host.
    #[test]
    fn snapshot_prune_removes_branches() {
        let snapshot = Snapshot {
            local_branches: iter::once(Branch::str("branch0")).collect(),
            host_branches: vec![
                HostBranch {
                    host: "host0".to_string(),
                    branch: Branch::str("branch0"),
                    ref_: Ref,
                },
                HostBranch {
                    host: "host0".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },
                HostBranch {
                    host: "host1".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },
            ],
        };

        let prune = snapshot.prune(&Config {
            user: "user0".to_string(),
            host: "host0".to_string(),
        });

        assert_eq!(
            prune,
            vec![HostBranch {
                host: "host0".to_string(),
                branch: Branch::str("branch1"),
                ref_: Ref,
            }]
        );
    }
}
