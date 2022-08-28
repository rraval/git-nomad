//! High level user invoked workflows for nomad.

use std::{collections::HashSet, hash::Hash};

use anyhow::Result;

use crate::{
    git_binary::GitBinary,
    git_ref::GitRef,
    types::{Host, NomadRef, Remote, User},
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
        style: LsStyle,
        user: User<'a>,
        fetch_remote: Option<Remote<'a>>,
    },
    Purge {
        user: User<'a>,
        remote: Remote<'a>,
        host_filter: Filter<Host<'a>>,
    },
}

impl Workflow<'_> {
    /// Imperatively execute the workflow.
    pub fn execute(self, git: &GitBinary) -> Result<()> {
        match self {
            Self::Sync { user, host, remote } => sync(git, &user, &host, &remote),
            Self::Ls {
                style,
                user,
                fetch_remote,
            } => ls(git, style, &user, fetch_remote),
            Self::Purge {
                user,
                remote,
                host_filter,
            } => purge(git, &user, &remote, host_filter),
        }
    }
}

/// Declarative representation of a limited filter function.
#[derive(Debug, PartialEq, Eq)]
pub enum Filter<T: PartialEq + Eq + Hash> {
    /// Everything.
    All,
    /// Only the specified values.
    Allow(HashSet<T>),
}

impl<T: PartialEq + Eq + Hash> Filter<T> {
    pub fn contains(&self, t: &T) -> bool {
        match self {
            Self::All => true,
            Self::Allow(hash_set) => hash_set.contains(t),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LsStyle {
    Grouped,
    Ref,
    Commit,
}

impl LsStyle {
    pub fn print_host(self, host: &Host) {
        match self {
            Self::Grouped => println!("{}", host.0),
            Self::Ref | Self::Commit => (),
        }
    }

    pub fn print_ref(self, ref_: &GitRef) {
        match self {
            Self::Grouped => println!("  {} -> {}", ref_.name, ref_.commit_id),
            Self::Ref => println!("{}", ref_.name),
            Self::Commit => println!("{}", ref_.commit_id),
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
        ls(git, LsStyle::Grouped, user, None)?
    }

    Ok(())
}

/// List all nomad managed refs organized by host.
///
/// Does not respect [`Progress::is_output_allowed`] because output is the whole point of this
/// command.
fn ls(git: &GitBinary, style: LsStyle, user: &User, fetch_remote: Option<Remote>) -> Result<()> {
    if let Some(remote) = fetch_remote {
        git.fetch_nomad_refs(user, &remote)?;
    }

    let snapshot = git.snapshot(user)?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        style.print_host(&host);

        for NomadRef { ref_, .. } in branches {
            style.print_ref(&ref_);
        }
    }

    Ok(())
}

/// Delete nomad managed refs returned by `to_prune`.
fn purge(git: &GitBinary, user: &User, remote: &Remote, host_filter: Filter<Host>) -> Result<()> {
    git.fetch_nomad_refs(user, remote)?;
    let snapshot = git.snapshot(user)?;
    let prune = snapshot.prune_by_hosts(|h| host_filter.contains(h));
    git.prune_nomad_refs(remote, prune.into_iter())?;
    Ok(())
}
