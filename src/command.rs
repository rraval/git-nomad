//! High level user invoked workflows for nomad.

use anyhow::{bail, Result};

use crate::{
    backend::{Backend, Config, NomadRef, Remote},
    progress::Progress,
    snapshot::{PruneFrom, Snapshot},
};

/// Initialize a git clone to have branches managed by nomad.
///
/// Will refuse to overwrite an already existing configuration.
pub fn init<B: Backend>(progress: &Progress, backend: &B, new_config: &Config) -> Result<()> {
    if let Some(existing_config) = backend.read_config()? {
        bail!(
            "Found existing config, refusing to init again: {:#?}",
            existing_config
        );
    }

    backend.write_config(new_config)?;
    if progress.is_output_allowed() {
        println!("Wrote {:#?}", new_config);
    }

    Ok(())
}

/// Synchronize current local branches with nomad managed refs in the given remote.
pub fn sync<B: Backend>(
    progress: &Progress,
    backend: &B,
    config: &Config,
    remote: &Remote,
) -> Result<()> {
    backend.push(config, remote)?;
    let remote_nomad_refs = backend.fetch(config, remote)?;
    let snapshot = backend.snapshot(config)?;
    backend.prune(
        remote,
        snapshot
            .prune_deleted_branches(config, &remote_nomad_refs)
            .iter(),
    )?;

    if progress.is_output_allowed() {
        println!();
        ls(backend, config)?
    }

    Ok(())
}

/// List all nomad managed refs organized by host.
///
/// Does not respect [`Progress::is_output_allowed`] because output is the whole point of this
/// command.
pub fn ls<B: Backend>(backend: &B, config: &Config) -> Result<()> {
    let snapshot = backend.snapshot(config)?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        println!("{}", host);

        for NomadRef { ref_, .. } in branches {
            println!("  {}", ref_);
        }
    }

    Ok(())
}

/// Delete nomad managed refs returned by `to_prune`.
pub fn prune<B: Backend, F>(
    backend: &B,
    config: &Config,
    remote: &Remote,
    to_prune: F,
) -> Result<()>
where
    F: Fn(Snapshot<B::Ref>) -> Vec<PruneFrom<B::Ref>>,
{
    backend.fetch(config, remote)?;
    let snapshot = backend.snapshot(config)?;
    let prune = to_prune(snapshot);
    backend.prune(remote, prune.iter())?;
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
