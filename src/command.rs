//! High level user invoked workflows for nomad.

use anyhow::{bail, Result};

use crate::{
    git_binary::GitBinary,
    git_ref::GitRef,
    progress::Progress,
    snapshot::{PruneFrom, Snapshot},
    types::{Config, NomadRef, Remote},
};

/// Initialize a git clone to have branches managed by nomad.
///
/// Will refuse to overwrite an already existing configuration.
pub fn init(progress: &Progress, git: &GitBinary, new_config: &Config) -> Result<()> {
    if let Some(existing_config) = git.read_nomad_config()? {
        bail!(
            "Found existing config, refusing to init again: {:#?}",
            existing_config
        );
    }

    git.write_nomad_config(new_config)?;
    if progress.is_output_allowed() {
        println!("Wrote {:#?}", new_config);
    }

    Ok(())
}

/// Synchronize current local branches with nomad managed refs in the given remote.
pub fn sync(progress: &Progress, git: &GitBinary, config: &Config, remote: &Remote) -> Result<()> {
    git.push_nomad_refs(config, remote)?;
    let remote_nomad_refs = git.fetch_nomad_refs(config, remote)?;
    let snapshot = git.snapshot(config)?;
    git.prune_nomad_refs(
        remote,
        snapshot
            .prune_deleted_branches(config, &remote_nomad_refs)
            .into_iter(),
    )?;

    if progress.is_output_allowed() {
        println!();
        ls(git, config)?
    }

    Ok(())
}

/// List all nomad managed refs organized by host.
///
/// Does not respect [`Progress::is_output_allowed`] because output is the whole point of this
/// command.
pub fn ls(git: &GitBinary, config: &Config) -> Result<()> {
    let snapshot = git.snapshot(config)?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        println!("{}", host);

        for NomadRef { ref_, .. } in branches {
            println!("  {}", ref_);
        }
    }

    Ok(())
}

/// Delete nomad managed refs returned by `to_prune`.
pub fn prune<F>(git: &GitBinary, config: &Config, remote: &Remote, to_prune: F) -> Result<()>
where
    F: Fn(Snapshot<GitRef>) -> Vec<PruneFrom<GitRef>>,
{
    git.fetch_nomad_refs(config, remote)?;
    let snapshot = git.snapshot(config)?;
    let prune = to_prune(snapshot);
    git.prune_nomad_refs(remote, prune.into_iter())?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, iter::FromIterator};

    use crate::{
        command::prune,
        git_testing::{GitRemote, INITIAL_BRANCH, PROGRESS},
        snapshot::Snapshot,
    };

    use super::sync;

    #[test]
    fn issue_2_other_host() {
        let origin = GitRemote::init();

        let host0 = origin.clone("host0");
        sync(&PROGRESS, &host0.git, &host0.config, &host0.remote()).unwrap();

        let host1 = origin.clone("host1");
        sync(&PROGRESS, &host1.git, &host1.config, &host1.remote()).unwrap();

        // both hosts have synced, the origin should have both refs
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // pruning refs for host0 from host1
        prune(&host1.git, &host1.config, &host1.remote(), |snapshot| {
            snapshot.prune_all_by_hosts(&HashSet::from_iter(["host0"]))
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

        let host0 = origin.clone("host0");
        sync(&PROGRESS, &host0.git, &host0.config, &host0.remote()).unwrap();

        let host1 = origin.clone("host1");
        sync(&PROGRESS, &host1.git, &host1.config, &host1.remote()).unwrap();

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
            &host1.config,
            &host1.remote(),
            Snapshot::prune_all,
        )
        .unwrap();

        // the origin should have no refs
        assert_eq!(origin.nomad_refs(), HashSet::new(),);
    }
}
