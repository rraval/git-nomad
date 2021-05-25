use anyhow::Result;

#[derive(Debug)]
pub struct Config {
    pub user: String,
    pub host: String,
}

pub struct Remote(pub String);

/// A user managed ref representing a local branch, like `refs/heads/master`.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalBranch(pub String);

/// A nomad managed ref representing a branch for the current host, where "current" is relative to
/// whatever [`Config.host`] was passed in.
#[derive(Debug, PartialEq, Eq)]
pub struct HostBranch(pub String);

pub trait Backend {
    fn read_config(&self) -> Result<Option<Config>>;
    fn write_config(&self, config: &Config) -> Result<()>;
    fn fetch(
        &self,
        config: &Config,
        remote: &Remote,
    ) -> Result<(Vec<LocalBranch>, Vec<HostBranch>)>;
    fn push(&self, config: &Config, remote: &Remote) -> Result<()>;
}
