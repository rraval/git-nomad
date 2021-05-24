use anyhow::{Result, bail};

use crate::backend::{Backend, Config};

pub fn init<B: Backend>(backend: B, new_config: Config) -> Result<()> {
    if let Some(existing_config) = backend.read_config()? {
        bail!(
            "Found existing config, refusing to init again: {:#?}",
            existing_config
        );
    }

    backend.write_config(&new_config)?;
    println!("Wrote {:#?}", new_config);

    Ok(())
}
