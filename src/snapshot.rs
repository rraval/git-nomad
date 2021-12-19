use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use crate::types::{Branch, Config, NomadRef};

/// A point in time view of refs we care about. [`Snapshot`] is only for local branches and refs
/// and thus is scoped under a specific [`Config::user`].
#[allow(clippy::manual_non_exhaustive)]
pub struct Snapshot<Ref> {
    /// The active branches in this clone that the user manipulates directly with `git branch` etc.
    pub local_branches: HashSet<Branch>,
    /// The refs that nomad manages to follow the local branches.
    pub nomad_refs: Vec<NomadRef<Ref>>,
    /// Force all callers to go through [`Snapshot::new`] which can validate invariants.
    _private: (),
}

/// Describes where a ref should be removed from.
#[derive(Debug, PartialEq, Eq)]
pub enum PruneFrom<Ref> {
    LocalOnly(NomadRef<Ref>),
    LocalAndRemote(NomadRef<Ref>),
}

impl<Ref> Snapshot<Ref> {
    /// Smart constructor that enforces the "scoped under a specific [`Config::user`]" invariant.
    pub fn new(
        config: &Config,
        local_branches: HashSet<Branch>,
        nomad_refs: Vec<NomadRef<Ref>>,
    ) -> Self {
        for nomad_ref in &nomad_refs {
            assert_eq!(config.user, nomad_ref.user);
        }

        Snapshot {
            local_branches,
            nomad_refs,
            _private: (),
        }
    }

    /// Return all nomad branches regardless of host.
    pub fn prune_all(self) -> Vec<PruneFrom<Ref>> {
        let Self { nomad_refs, .. } = self;
        nomad_refs
            .into_iter()
            .map(PruneFrom::LocalAndRemote)
            .collect()
    }

    /// Return all nomad branches for specific hosts.
    pub fn prune_all_by_hosts(self, hosts: &HashSet<&str>) -> Vec<PruneFrom<Ref>> {
        let Self { nomad_refs, .. } = self;
        nomad_refs
            .into_iter()
            .filter_map(|nomad_ref| {
                if !hosts.contains(nomad_ref.host.as_str()) {
                    return None;
                }

                Some(PruneFrom::LocalAndRemote(nomad_ref))
            })
            .collect()
    }

    /// Return all [`NomadRef`]s grouped by host in sorted order.
    pub fn sorted_hosts_and_branches(self) -> Vec<(String, Vec<NomadRef<Ref>>)> {
        let mut by_host = HashMap::<String, Vec<NomadRef<Ref>>>::new();
        let Self { nomad_refs, .. } = self;

        for nomad_ref in nomad_refs {
            by_host
                .entry(nomad_ref.host.clone())
                .or_insert_with(Vec::new)
                .push(nomad_ref);
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

impl<Ref: Eq + Hash> Snapshot<Ref> {
    /// Find nomad host branches that can be pruned because:
    /// 1. The local branch they were based on no longer exists.
    /// 2. The remote branch they were based on no longer exists.
    pub fn prune_deleted_branches(
        self,
        config: &Config,
        remote_nomad_refs: &HashSet<NomadRef<Ref>>,
    ) -> Vec<PruneFrom<Ref>> {
        let Self {
            nomad_refs,
            local_branches,
            ..
        } = self;

        let mut prune = Vec::<PruneFrom<Ref>>::new();

        for nomad_ref in nomad_refs {
            if nomad_ref.host == config.host {
                if !local_branches.contains(&nomad_ref.branch) {
                    prune.push(PruneFrom::LocalAndRemote(nomad_ref));
                }
            } else if !remote_nomad_refs.contains(&nomad_ref) {
                prune.push(PruneFrom::LocalOnly(nomad_ref));
            }
        }

        prune
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fmt,
        iter::{self, FromIterator},
    };

    use crate::types::Config;

    use super::{Branch, NomadRef, PruneFrom, Snapshot};

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct Ref;

    impl fmt::Display for Ref {
        fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
            unreachable!()
        }
    }

    fn snapshot(
        config: &Config,
        local_branches: impl IntoIterator<Item = &'static str>,
    ) -> Snapshot<Ref> {
        Snapshot::new(
            config,
            local_branches.into_iter().map(Branch::str).collect(),
            vec![
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
        )
    }

    fn config() -> Config {
        Config {
            user: "user0".to_string(),
            host: "host0".to_string(),
        }
    }

    fn remote_nomad_refs(
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
        let config = &config();
        let prune = snapshot(config, ["branch0", "branch1"])
            .prune_deleted_branches(config, &remote_nomad_refs([("user0", "host1", "branch1")]));

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
        let config = &config();
        let prune = snapshot(config, ["branch0", "branch1"]).prune_deleted_branches(
            config,
            &remote_nomad_refs([
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
        let config = &config();
        let prune = snapshot(
            config,
            [
                "branch0",
                // This branch has been removed
                // "branch1",
            ],
        )
        .prune_deleted_branches(
            config,
            &remote_nomad_refs([
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
        let config = &config();
        let prune = snapshot(config, ["branch0", "branch1"]).prune_deleted_branches(
            config,
            &remote_nomad_refs([
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
        let prune = snapshot(&config(), ["branch0", "branch1"]).prune_all();
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
        let prune = snapshot(&config(), ["branch0", "branch1"])
            .prune_all_by_hosts(&iter::once("host0").collect());
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
