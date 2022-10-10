//! High level user invoked workflows for nomad.

use std::{collections::HashSet, hash::Hash, io::Write};

use anyhow::{Context, Result};

use crate::{
    git_binary::GitBinary,
    git_ref::GitRef,
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
        printer: LsPrinter,
        user: User<'a>,
        fetch_remote: Option<Remote<'a>>,
        host_filter: Filter<Host<'a>>,
        branch_filter: Filter<Branch<'a>>,
    },
    Purge {
        user: User<'a>,
        remote: Remote<'a>,
        host_filter: Filter<Host<'a>>,
    },
}

impl Workflow<'_> {
    /// Imperatively execute the workflow.
    pub fn execute(self, git: &GitBinary, output: &mut dyn Write) -> Result<()> {
        match self {
            Self::Sync { user, host, remote } => sync(git, output, &user, &host, &remote),
            Self::Ls {
                printer,
                user,
                fetch_remote,
                host_filter,
                branch_filter,
            } => ls(
                git,
                output,
                printer,
                &user,
                fetch_remote,
                host_filter,
                branch_filter,
            ),
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
    /// Everything except the specified values.
    Deny(HashSet<T>),
}

impl<T: PartialEq + Eq + Hash> Filter<T> {
    pub fn contains(&self, t: &T) -> bool {
        match self {
            Self::All => true,
            Self::Allow(hash_set) => hash_set.contains(t),
            Self::Deny(hash_set) => !hash_set.contains(t),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LsPrinter {
    Grouped,
    Ref,
    Commit,
}

impl LsPrinter {
    pub fn print_host(self, output: &mut dyn Write, host: &Host) -> Result<()> {
        match self {
            Self::Grouped => writeln!(output, "{}", host.0).context("printing grouped host"),
            Self::Ref | Self::Commit => Ok(()),
        }
    }

    pub fn print_ref(self, output: &mut dyn Write, ref_: &GitRef) -> Result<()> {
        match self {
            Self::Grouped => writeln!(output, "  {} -> {}", ref_.name, ref_.commit_id)
                .context("printing ref and commit"),
            Self::Ref => writeln!(output, "{}", ref_.name).context("printing ref"),
            Self::Commit => writeln!(output, "{}", ref_.commit_id).context("printing commit"),
        }
    }
}

/// Synchronize current local branches with nomad managed refs in the given remote.
fn sync(
    git: &GitBinary,
    output: &mut dyn Write,
    user: &User,
    host: &Host,
    remote: &Remote,
) -> Result<()> {
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
        writeln!(output)?;
        ls(
            git,
            output,
            LsPrinter::Grouped,
            user,
            None,
            Filter::All,
            Filter::All,
        )?
    }

    Ok(())
}

/// List all nomad managed refs organized by host.
///
/// Does not respect [`Progress::is_output_allowed`] because output is the whole point of this
/// command.
fn ls(
    git: &GitBinary,
    output: &mut dyn Write,
    printer: LsPrinter,
    user: &User,
    fetch_remote: Option<Remote>,
    host_filter: Filter<Host>,
    branch_filter: Filter<Branch>,
) -> Result<()> {
    if let Some(remote) = fetch_remote {
        git.fetch_nomad_refs(user, &remote)?;
    }

    let snapshot = git.snapshot(user)?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        if !host_filter.contains(&host) {
            continue;
        }

        printer.print_host(output, &host)?;

        for NomadRef { ref_, branch, .. } in branches {
            if branch_filter.contains(&branch) {
                printer.print_ref(output, &ref_)?;
            }
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

#[cfg(test)]
mod test {
    use crate::{git_testing::GitRemote, output::OutputStream, workflow::sync};

    use super::{Filter, LsPrinter, Workflow};

    #[test]
    fn ls_printer() {
        let remote = GitRemote::init();

        let clone = remote.clone("user0", "host0");
        let commit_id = clone.current_commit();

        sync(
            &clone.git,
            &mut OutputStream::new_sink(),
            &clone.user,
            &clone.host,
            &clone.remote,
        )
        .unwrap();

        for (printer, expected) in &[
            (
                LsPrinter::Grouped,
                format!(
                    "{}\n  refs/nomad/{}/master -> {}\n",
                    clone.host.0, clone.host.0, commit_id.0
                ),
            ),
            (
                LsPrinter::Ref,
                format!("refs/nomad/{}/master\n", clone.host.0),
            ),
            (LsPrinter::Commit, format!("{}\n", commit_id.0)),
        ] {
            let mut output = OutputStream::new_vec();

            Workflow::Ls {
                printer: *printer,
                user: clone.user.clone(),
                fetch_remote: Some(clone.remote.clone()),
                host_filter: Filter::All,
                branch_filter: Filter::All,
            }
            .execute(&clone.git, &mut output)
            .unwrap();

            assert_eq!(output.as_str(), expected);
        }
    }
}
