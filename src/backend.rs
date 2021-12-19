//! See [`Backend`] for the primary entry point.
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    hash::Hash,
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

/// A point in time view of refs we care about.
pub struct Snapshot<Ref: Display + Eq + Hash> {
    /// The active branches in this clone that the user manipulates directly with `git branch` etc.
    pub local_branches: HashSet<Branch>,
    /// The refs that nomad manages to follow the local branches.
    pub host_branches: Vec<NomadRef<Ref>>,
}

/// Describes where a ref should be removed from.
#[derive(Debug, PartialEq, Eq)]
pub enum PruneFrom<Ref> {
    LocalOnly(NomadRef<Ref>),
    LocalAndRemote(NomadRef<Ref>),
}

impl<Ref: Display + Eq + Hash> Snapshot<Ref> {
    /// Find nomad host branches that can be pruned because:
    /// 1. The local branch they were based on no longer exists.
    /// 2. The remote branch they were based on no longer exists.
    pub fn prune_deleted_branches(
        self,
        config: &Config,
        remote_host_branches: &HashSet<NomadRef<Ref>>,
    ) -> Vec<PruneFrom<Ref>> {
        let Self {
            host_branches,
            local_branches,
        } = self;

        let mut prune = Vec::<PruneFrom<Ref>>::new();

        for hb in host_branches {
            if hb.host == config.host {
                if !local_branches.contains(&hb.branch) {
                    prune.push(PruneFrom::LocalAndRemote(hb));
                }
            } else if !remote_host_branches.contains(&hb) {
                prune.push(PruneFrom::LocalOnly(hb));
            }
        }

        prune
    }

    /// Return all nomad branches regardless of host.
    pub fn prune_all(self) -> Vec<PruneFrom<Ref>> {
        let Self { host_branches, .. } = self;
        host_branches
            .into_iter()
            .map(PruneFrom::LocalAndRemote)
            .collect()
    }

    /// Return all nomad branches for specific hosts.
    pub fn prune_all_by_hosts(self, hosts: &HashSet<&str>) -> Vec<PruneFrom<Ref>> {
        let Self { host_branches, .. } = self;
        host_branches
            .into_iter()
            .filter_map(|hb| {
                if !hosts.contains(hb.host.as_str()) {
                    return None;
                }

                Some(PruneFrom::LocalAndRemote(hb))
            })
            .collect()
    }

    /// Return all [`NomadRef`]s grouped by host in sorted order.
    pub fn sorted_hosts_and_branches(self) -> Vec<(String, Vec<NomadRef<Ref>>)> {
        let mut by_host = HashMap::<String, Vec<NomadRef<Ref>>>::new();
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

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fmt,
        iter::{self, FromIterator},
    };

    use crate::backend::{Config, PruneFrom};

    use super::{Branch, NomadRef, Snapshot};

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct Ref;

    impl fmt::Display for Ref {
        fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
            unreachable!()
        }
    }

    fn snapshot(local_branches: impl IntoIterator<Item = &'static str>) -> Snapshot<Ref> {
        Snapshot {
            local_branches: local_branches.into_iter().map(Branch::str).collect(),
            host_branches: vec![
                NomadRef {
                    user: "user0".to_string(),
                    host: "host0".to_string(),
                    branch: Branch::str("branch0"),
                    ref_: Ref,
                },
                NomadRef {
                    user: "user0".to_string(),
                    host: "host0".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },
                NomadRef {
                    user: "user0".to_string(),
                    host: "host1".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },
            ],
        }
    }

    fn config() -> Config {
        Config {
            user: "user0".to_string(),
            host: "host0".to_string(),
        }
    }

    fn remote_host_branches(
        collection: impl IntoIterator<Item = (&'static str, &'static str, &'static str)>,
    ) -> HashSet<NomadRef<Ref>> {
        HashSet::from_iter(collection.into_iter().map(|(user, host, branch)| NomadRef {
            user: user.to_string(),
            host: host.to_string(),
            branch: Branch::str(branch),
            ref_: Ref {},
        }))
    }

    /// Sets up the scenario where:
    ///
    ///     There are local branches
    ///     ... That DO NOT have nomad refs
    ///
    ///     There are local nomad refs from other hosts
    ///     ... That have corresponding remote nomad refs
    ///
    /// In this case, we should prune nothing.
    #[test]
    fn snapshot_prune_does_nothing0() {
        let prune = snapshot(["branch0", "branch1"]).prune_deleted_branches(
            &config(),
            &remote_host_branches([("user0", "host1", "branch1")]),
        );

        assert_eq!(prune, Vec::new(),);
    }

    /// Sets up the scenario where:
    ///
    ///     There are local branches
    ///     ... That have nomad refs
    ///
    ///     There are local nomad refs from other hosts
    ///     ... That have corresponding remote nomad refs
    ///
    /// In this case, we should prune nothing.
    #[test]
    fn snapshot_prune_does_nothing1() {
        let prune = snapshot(["branch0", "branch1"]).prune_deleted_branches(
            &config(),
            &remote_host_branches([
                ("user0", "host0", "branch0"),
                ("user0", "host0", "branch1"),
                ("user0", "host1", "branch1"),
            ]),
        );

        assert_eq!(prune, Vec::new(),);
    }

    /// Sets up the scenario where:
    ///
    ///     There are NO local branches
    ///     ... That have nomad refs
    ///
    ///     There are local nomad refs from other hosts
    ///     ... That have corresponding remote nomad refs
    ///
    /// In this case, we should remove the nomad refs for the local branches that no longer exist.
    #[test]
    fn snapshot_prune_removes_local_missing_branches() {
        let prune = snapshot([
            "branch0",
            // This branch has been removed
            // "branch1",
        ])
        .prune_deleted_branches(
            &config(),
            &remote_host_branches([
                ("user0", "host0", "branch0"),
                ("user0", "host0", "branch1"),
                ("user0", "host1", "branch1"),
            ]),
        );

        assert_eq!(
            prune,
            vec![PruneFrom::LocalAndRemote(NomadRef {
                user: "user0".to_string(),
                host: "host0".to_string(),
                branch: Branch::str("branch1"),
                ref_: Ref,
            })]
        );
    }

    /// Sets up the scenario where:
    ///
    ///     There are local branches
    ///     ... That have nomad refs
    ///
    ///     There are local nomad refs from other hosts
    ///     ... That DO NOT have corresponding remote nomad refs
    ///
    /// In this case, we should remove the local nomad refs from other hosts since the
    /// corresponding remote refs no longer exist.
    #[test]
    fn snapshot_prune_removes_remote_missing_branches() {
        let prune = snapshot(["branch0", "branch1"]).prune_deleted_branches(
            &config(),
            &remote_host_branches([
                ("user0", "host0", "branch0"),
                ("user0", "host0", "branch1"),
                // This remote nomad ref for another host has been removed
                // ("user0", "host1", "branch1"),
            ]),
        );

        assert_eq!(
            prune,
            vec![PruneFrom::LocalOnly(NomadRef {
                user: "user0".to_string(),
                host: "host1".to_string(),
                branch: Branch::str("branch1"),
                ref_: Ref,
            })]
        );
    }

    /// [`Snapshot::prune_all`] should remove all branches.
    #[test]
    fn snapshot_prune_all() {
        let prune = snapshot(["branch0", "branch1"]).prune_all();
        assert_eq!(
            prune,
            vec![
                PruneFrom::LocalAndRemote(NomadRef {
                    user: "user0".to_string(),
                    host: "host0".to_string(),
                    branch: Branch::str("branch0"),
                    ref_: Ref,
                },),
                PruneFrom::LocalAndRemote(NomadRef {
                    user: "user0".to_string(),
                    host: "host0".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },),
                PruneFrom::LocalAndRemote(NomadRef {
                    user: "user0".to_string(),
                    host: "host1".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },),
            ],
        );
    }

    /// [`Snapshot::prune_all_by_hosts`] should only remove branches for specified hosts.
    #[test]
    fn snapshot_prune_hosts() {
        let prune =
            snapshot(["branch0", "branch1"]).prune_all_by_hosts(&iter::once("host0").collect());
        assert_eq!(
            prune,
            vec![
                PruneFrom::LocalAndRemote(NomadRef {
                    user: "user0".to_string(),
                    host: "host0".to_string(),
                    branch: Branch::str("branch0"),
                    ref_: Ref,
                },),
                PruneFrom::LocalAndRemote(NomadRef {
                    user: "user0".to_string(),
                    host: "host0".to_string(),
                    branch: Branch::str("branch1"),
                    ref_: Ref,
                },),
            ],
        );
    }
}
