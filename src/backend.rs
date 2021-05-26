use std::collections::HashSet;

use anyhow::Result;

#[derive(Debug)]
pub struct Config {
    pub user: String,
    pub host: String,
}

pub struct Remote(pub String);

/// The branch name part of a ref. `refs/head/master` would be `Branch("master")`.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Branch(pub String);

impl Branch {
    pub fn str<S: AsRef<str>>(s: S) -> Branch {
        Branch(s.as_ref().to_string())
    }
}

/// A nomad managed ref representing a branch for the current host, where "current" is relative to
/// whatever [`Config.host`] was passed in.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct HostBranch<Ref> {
    pub branch: Branch,
    pub ref_: Ref,
}

/// A point in time view of branches we care about.
pub struct Snapshot<Ref> {
    pub local_branches: HashSet<Branch>,
    pub host_branches: HashSet<HostBranch<Ref>>,
}

impl<Ref> Snapshot<Ref> {
    pub fn prune(&self) -> impl Iterator<Item = &HostBranch<Ref>> {
        self.host_branches
            .iter()
            .filter(move |hb| !self.local_branches.contains(&hb.branch))
    }
}

pub trait Backend {
    type Ref;

    fn read_config(&self) -> Result<Option<Config>>;
    fn write_config(&self, config: &Config) -> Result<()>;

    fn fetch(&self, config: &Config, remote: &Remote) -> Result<Snapshot<Self::Ref>>;

    fn push(&self, config: &Config, remote: &Remote) -> Result<()>;

    fn prune<'a, Prune>(&self, config: &Config, remote: &Remote, prune: Prune) -> Result<()>
    where
        Self::Ref: 'a,
        Prune: Iterator<Item = &'a HostBranch<Self::Ref>>;
}
