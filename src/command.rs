use anyhow::{bail, Result};

use crate::backend::{Backend, Config, LocalBranch, Remote};

pub fn init<B: Backend>(backend: B, new_config: &Config) -> Result<()> {
    if let Some(existing_config) = backend.read_config()? {
        bail!(
            "Found existing config, refusing to init again: {:#?}",
            existing_config
        );
    }

    backend.write_config(new_config)?;
    println!("Wrote {:#?}", new_config);

    Ok(())
}

pub fn sync<B: Backend>(backend: B, config: &Config, remote: &Remote) -> Result<()> {
    backend.push(config, remote)?;
    let (local_branches, host_branches) = backend.fetch(config, remote)?;
    let prune = host_branches
        .iter()
        .filter(|b| !local_branches.contains(&LocalBranch(b.name.clone())));
    backend.prune(config, remote, prune)?;
    Ok(())
}
