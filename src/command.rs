//! High level user invoked workflows for nomad.

use anyhow::{bail, Result};

use crate::{
    git_binary::GitBinary,
    git_ref::GitRef,
    snapshot::{PruneFrom, Snapshot},
    types::{Host, NomadRef, Remote, User},
};

/// Initialize a git clone to have branches managed by nomad.
///
/// Will refuse to overwrite an already existing configuration.
pub fn init(git: &GitBinary, new_user: &User, new_host: &Host) -> Result<()> {
    if let Some(existing_config) = git.read_nomad_config()? {
        bail!(
            "Found existing config, refusing to init again: {:#?}",
            existing_config
        );
    }

    git.write_nomad_config(new_user, new_host)?;
    if git.is_output_allowed() {
        println!("Initialized {} @ {}", new_user.0, new_host.0);
    }

    Ok(())
}

/// Synchronize current local branches with nomad managed refs in the given remote.
pub fn sync(git: &GitBinary, user: &User, host: &Host, remote: &Remote) -> Result<()> {
    git.push_nomad_refs(user, host, remote)?;
    git.fetch_nomad_refs(user, remote)?;
    let remote_nomad_refs = git.list_nomad_refs(user, remote)?.collect();
    let snapshot = git.snapshot(user)?;
    git.prune_nomad_refs(
        remote,
        snapshot
            .prune_deleted_branches(host, &remote_nomad_refs)
            .into_iter(),
    )?;

    if git.is_output_allowed() {
        println!();
        ls(git, user)?
    }

    Ok(())
}

/// List all nomad managed refs organized by host.
///
/// Does not respect [`Progress::is_output_allowed`] because output is the whole point of this
/// command.
pub fn ls(git: &GitBinary, user: &User) -> Result<()> {
    let snapshot = git.snapshot(user)?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        println!("{}", host.0);

        for NomadRef { ref_, .. } in branches {
            println!("  {}", ref_);
        }
    }

    Ok(())
}

/// Delete nomad managed refs returned by `to_prune`.
pub fn prune<F>(git: &GitBinary, user: &User, remote: &Remote, to_prune: F) -> Result<()>
where
    F: Fn(Snapshot<GitRef>) -> Vec<PruneFrom<GitRef>>,
{
    git.fetch_nomad_refs(user, remote)?;
    let snapshot = git.snapshot(user)?;
    let prune = to_prune(snapshot);
    git.prune_nomad_refs(remote, prune.into_iter())?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, iter::FromIterator};

    use crate::{
        command::prune,
        git_testing::{GitClone, GitRemote, INITIAL_BRANCH},
        snapshot::Snapshot,
        types::Branch,
    };

    use super::sync;

    fn sync_host(host: &GitClone) {
        sync(&host.git, &host.user, &host.host, &host.remote()).unwrap();
    }

    #[test]
    fn issue_1() {
        let origin = GitRemote::init();
        let feature = &Branch::from("feature");

        let host0 = origin.clone("user0", "host0");
        sync_host(&host0);

        let host1 = origin.clone("user0", "host1");
        host1
            .git
            .create_branch("Start feature branch", feature)
            .unwrap();
        sync_host(&host1);

        // both hosts have synced, the origin should have refs from both (including the one for the
        // feature branch on host1)
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref("feature").unwrap(),
            ])
        );

        // host0 hasn't observed host1 yet
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),])
        );

        // sync host0, which should observe host1 refs
        sync_host(&host0);
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref("feature").unwrap(),
            ])
        );

        // host1 deletes the branch and syncs, removing it from origin
        host1
            .git
            .delete_branch("Abandon feature branch", feature)
            .unwrap();
        sync_host(&host1);

        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // host0 syncs and removes the ref for the deleted feature branch
        sync_host(&host0);
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );
    }

    #[test]
    fn issue_2_other_host() {
        let origin = GitRemote::init();

        let host0 = origin.clone("user0", "host0");
        sync_host(&host0);

        let host1 = origin.clone("user0", "host1");
        sync_host(&host1);

        // both hosts have synced, the origin should have both refs
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // pruning refs for host0 from host1
        prune(&host1.git, &host1.user, &host1.remote(), |snapshot| {
            snapshot.prune_all_by_hosts(&HashSet::from_iter([host0.host.clone()]))
        })
        .unwrap();

        // the origin should only have refs for host1
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),])
        );
    }

    #[test]
    fn issue_2_all() {
        let origin = GitRemote::init();

        let host0 = origin.clone("user0", "host0");
        sync_host(&host0);

        let host1 = origin.clone("user0", "host1");
        sync_host(&host1);

        // both hosts have synced, the origin should have both refs
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // pruning refs for all hosts from host1
        prune(
            &host1.git,
            &host1.user,
            &host1.remote(),
            Snapshot::prune_all,
        )
        .unwrap();

        // the origin should have no refs
        assert_eq!(origin.nomad_refs(), HashSet::new(),);
    }
}
