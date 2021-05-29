//! High level user invoked workflows for nomad.

use anyhow::{bail, Result};

use crate::{
    backend::{Backend, Config, Remote},
    progress::Progress,
};

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
pub fn sync<B: Backend>(backend: B, config: &Config, remote: &Remote) -> Result<()> {
    backend.push(config, remote)?;
    let snapshot = backend.fetch(config, remote)?;
    backend.prune(config, remote, snapshot.prune())?;
    Ok(())
}
