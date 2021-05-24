use anyhow::{bail, Result};

use crate::backend::{Backend, Config, Remote};

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
    backend.fetch(config, remote)?;
    backend.push(config, remote)?;
    Ok(())
}
