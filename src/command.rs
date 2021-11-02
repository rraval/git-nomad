//! High level user invoked workflows for nomad.

use anyhow::{bail, Result};

use crate::{backend::{Backend, Config, HostBranch, PruneFrom, Remote, Snapshot}, progress::Progress};

/// Initialize a git clone to have branches managed by nomad.
///
/// Will refuse to overwrite an already existing configuration.
pub fn init<B: Backend>(progress: &Progress, backend: B, new_config: &Config) -> Result<()> {
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
    backend: B,
    config: &Config,
    remote: &Remote,
) -> Result<()> {
    backend.push(config, remote)?;
    let remote_host_branches = backend.fetch(config, remote)?;
    let snapshot = backend.snapshot()?;
    backend.prune(
        config,
        remote,
        snapshot
            .prune_deleted_branches(config, &remote_host_branches)
            .iter(),
    )?;

    if progress.is_output_allowed() {
        println!();
        ls(backend)?
    }

    Ok(())
}

/// List all nomad managed refs organized by host.
///
/// Does not respect [`Progress::is_output_allowed`] because output is the whole point of this
/// command.
pub fn ls<B: Backend>(backend: B) -> Result<()> {
    let snapshot = backend.snapshot()?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        println!("{}", host);

        for HostBranch { ref_, .. } in branches {
            println!("  {}", ref_);
        }
    }

    Ok(())
}

/// Delete nomad managed refs returned by `to_prune`.
pub fn prune<B: Backend, F>(backend: B, config: &Config, remote: &Remote, to_prune: F) -> Result<()>
where
    F: Fn(Snapshot<B::Ref>) -> Vec<PruneFrom<B::Ref>>,
{
    backend.fetch(config, remote)?;
    let snapshot = backend.snapshot()?;
    let prune = to_prune(snapshot);
    backend.prune(config, remote, prune.iter())?;
    Ok(())
}
