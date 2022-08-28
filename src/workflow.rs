//! High level user invoked workflows for nomad.

use std::collections::HashSet;

use anyhow::{anyhow, Result};

use crate::{
    git_binary::GitBinary,
    snapshot::Snapshot,
    types::{Branch, Host, NomadRef, Remote, User},
};

/// A boundary type that separates the CLI interface from high level nomad workflows.
#[derive(Debug, PartialEq, Eq)]
pub enum Workflow<'a> {
    Sync {
        user: User<'a>,
        host: Host<'a>,
        remote: Remote<'a>,
    },
    Ls {
        user: User<'a>,
    },
    Purge {
        user: User<'a>,
        remote: Remote<'a>,
        purge_filter: PurgeFilter<'a>,
    },
    Ref {
        should_fetch: bool,
        user: User<'a>,
        remote: Remote<'a>,
        branch: Branch<'a>,
        for_host: ForHost<'a>,
    },
}

/// How should local and remote refs be deleted during the `purge` workflow.
#[derive(Debug, PartialEq, Eq)]
pub enum PurgeFilter<'a> {
    /// Delete all nomad managed refs for the given [`User`].
    All,
    /// Delete only nomad managed refs for given [`Host`]s under the given [`User`].
    Hosts(HashSet<Host<'a>>),
}

impl Workflow<'_> {
    /// Imperatively execute the workflow.
    pub fn execute(self, git: &GitBinary) -> Result<()> {
        match self {
            Self::Sync { user, host, remote } => sync(git, &user, &host, &remote),
            Self::Ls { user } => ls(git, &user),
            Self::Purge {
                user,
                remote,
                purge_filter,
            } => purge(git, &user, &remote, purge_filter),
            Self::Ref {
                should_fetch,
                user,
                remote,
                branch,
                for_host,
            } => print_ref(git, should_fetch, user, remote, branch, for_host),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ForHost<'a> {
    Specific { host: Host<'a> },
    Other { ignoring: Host<'a> },
}

impl ForHost<'_> {
    fn matches<Ref>(&self, nomad_ref: &NomadRef<Ref>) -> bool {
        match self {
            Self::Specific { host } => &nomad_ref.host == host,
            Self::Other { ignoring } => &nomad_ref.host != ignoring,
        }
    }

    fn error(&self, branch: &Branch) -> anyhow::Error {
        match self {
            Self::Specific { host } => anyhow!("No branch `{}` for host `{}`", branch.0, host.0),
            Self::Other { ignoring } => {
                anyhow!("No branch `{}` other than host `{}`", branch.0, ignoring.0)
            }
        }
    }
}

/// Synchronize current local branches with nomad managed refs in the given remote.
fn sync(git: &GitBinary, user: &User, host: &Host, remote: &Remote) -> Result<()> {
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
fn ls(git: &GitBinary, user: &User) -> Result<()> {
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
fn purge(git: &GitBinary, user: &User, remote: &Remote, purge_filter: PurgeFilter) -> Result<()> {
    git.fetch_nomad_refs(user, remote)?;
    let snapshot = git.snapshot(user)?;
    let prune = match purge_filter {
        PurgeFilter::All => snapshot.prune_all(),
        PurgeFilter::Hosts(host_set) => snapshot.prune_all_by_hosts(&host_set),
    };
    git.prune_nomad_refs(remote, prune.into_iter())?;
    Ok(())
}

/// FIXME
fn print_ref(
    git: &GitBinary,
    should_fetch: bool,
    user: User,
    remote: Remote,
    branch: Branch,
    for_host: ForHost,
) -> Result<()> {
    if should_fetch {
        git.fetch_nomad_refs(&user, &remote)?;
    }

    let Snapshot { mut nomad_refs, .. } = git.snapshot(&user)?;
    nomad_refs.retain(|nomad_ref| {
        nomad_ref.user == user && nomad_ref.branch == branch && for_host.matches(nomad_ref)
    });

    match nomad_refs.as_slice() {
        [] => Err(for_host.error(&branch)),

        [only] => {
            println!("{}", only.ref_.name);
            Ok(())
        }

        // The filtering strategies above should lead to 0 or 1 result. Anything else indicates
        // programmer error.
        _ => todo!(),
    }
}
