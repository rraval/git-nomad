//! High level user invoked workflows for nomad.

use std::{collections::HashSet, hash::Hash, io::Write};

use anyhow::{Context, Result};

use crate::{
    git_binary::GitBinary,
    git_ref::GitRef,
    renderer::{Renderer, add_newline_if_spinners_are_visible},
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
    Completions(clap_complete::Shell),
}

impl Workflow<'_> {
    /// Imperatively execute the workflow.
    pub fn execute(self, renderer: &mut impl Renderer, git: &GitBinary) -> Result<()> {
        match self {
            Self::Sync { user, host, remote } => sync(renderer, git, &user, &host, &remote),
            Self::Ls {
                printer,
                user,
                fetch_remote,
                host_filter,
                branch_filter,
            } => ls(
                renderer,
                git,
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
            } => purge(renderer, git, &user, &remote, host_filter),
            Self::Completions(shell) => print_completions(renderer, shell),
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
    renderer: &mut impl Renderer,
    git: &GitBinary,
    user: &User,
    host: &Host,
    remote: &Remote,
) -> Result<()> {
    git.push_nomad_refs(renderer, user, host, remote)?;
    git.fetch_nomad_refs(renderer, user, remote)?;
    let remote_nomad_refs = git.list_nomad_refs(renderer, user, remote)?.collect();
    let snapshot = git.snapshot(renderer, user)?;
    git.prune_nomad_refs(
        renderer,
        remote,
        snapshot
            .prune_deleted_branches(host, &remote_nomad_refs)
            .into_iter(),
    )?;

    if git.is_output_allowed() {
        add_newline_if_spinners_are_visible(renderer)?;

        ls(
            renderer,
            git,
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
/// Does not respect [`GitBinary::is_output_allowed`] because output is the whole point of this
/// command.
fn ls(
    renderer: &mut impl Renderer,
    git: &GitBinary,
    printer: LsPrinter,
    user: &User,
    fetch_remote: Option<Remote>,
    host_filter: Filter<Host>,
    branch_filter: Filter<Branch>,
) -> Result<()> {
    if let Some(remote) = fetch_remote {
        git.fetch_nomad_refs(renderer, user, &remote)?;
    }

    let snapshot = git.snapshot(renderer, user)?;

    for (host, branches) in snapshot.sorted_hosts_and_branches() {
        if !host_filter.contains(&host) {
            continue;
        }

        renderer.writer(|w| {
            printer.print_host(w, &host)?;

            for NomadRef { ref_, branch, .. } in branches {
                if branch_filter.contains(&branch) {
                    printer.print_ref(w, &ref_)?;
                }
            }

            Ok(())
        })?;
    }

    Ok(())
}

/// Delete nomad managed refs returned by `to_prune`.
fn purge(
    renderer: &mut impl Renderer,
    git: &GitBinary,
    user: &User,
    remote: &Remote,
    host_filter: Filter<Host>,
) -> Result<()> {
    git.fetch_nomad_refs(renderer, user, remote)?;
    let snapshot = git.snapshot(renderer, user)?;
    let prune = snapshot.prune_by_hosts(|h| host_filter.contains(h));
    git.prune_nomad_refs(renderer, remote, prune.into_iter())?;
    Ok(())
}

/// Use [`clap_complete`] to emit shell syntax for tab-completions
fn print_completions(
    renderer: &mut impl Renderer,
    generator: impl clap_complete::Generator,
) -> Result<()> {
    let mut cmd = crate::build_cli(None, None);
    let bin_name = cmd.get_name().to_string();
    renderer.writer(|writer| {
        clap_complete::generate(generator, &mut cmd, bin_name, writer);
        Ok(())
    })
}

#[cfg(test)]
mod test {
    use crate::{
        git_testing::GitRemote,
        renderer::test::{MemoryRenderer, NoRenderer},
        workflow::sync,
    };

    use super::{Filter, LsPrinter, Workflow};

    #[test]
    fn ls_one_host() {
        let remote = GitRemote::init(None);

        let clone = remote.clone("user0", "host0");
        let commit_id = clone.current_commit();

        sync(
            &mut NoRenderer,
            &clone.git,
            &clone.user,
            &clone.host,
            &clone.remote,
        )
        .unwrap();

        for (printer, expected) in [
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
            let mut renderer = MemoryRenderer::new();

            Workflow::Ls {
                printer,
                user: clone.user.clone(),
                fetch_remote: Some(clone.remote.clone()),
                host_filter: Filter::All,
                branch_filter: Filter::All,
            }
            .execute(&mut renderer, &clone.git)
            .unwrap();

            assert_eq!(renderer.as_str(), expected);
        }
    }

    /// Exercise `LsPrinter::Grouped` with a bunch of `Filter::Deny`s.
    #[test]
    fn ls_two_hosts() {
        let remote = GitRemote::init(None);

        let host0 = remote.clone("user0", "host0");
        let host1 = remote.clone("user0", "host1");

        sync(
            &mut NoRenderer,
            &host0.git,
            &host0.user,
            &host0.host,
            &host0.remote,
        )
        .unwrap();

        sync(
            &mut NoRenderer,
            &host1.git,
            &host1.user,
            &host1.host,
            &host1.remote,
        )
        .unwrap();

        let mut renderer = MemoryRenderer::new();
        Workflow::Ls {
            printer: LsPrinter::Grouped,
            user: host1.user,
            fetch_remote: Some(host1.remote),
            host_filter: Filter::Deny([host0.host].into()),
            branch_filter: Filter::Deny([host1.git.current_branch(&mut renderer).unwrap()].into()),
        }
        .execute(&mut renderer, &host1.git)
        .unwrap();

        assert_eq!(renderer.as_str(), "host1\n");
    }

    #[test]
    fn filter_does_filtering() {
        for (filter, expected) in [
            (Filter::All, vec!["foo", "bar"]),
            (Filter::Allow(["foo"].into()), vec!["foo"]),
            (Filter::Deny(["foo"].into()), vec!["bar"]),
        ] {
            let mut got = vec!["foo", "bar"];
            got.retain(|i| filter.contains(i));
            assert_eq!(got, expected);
        }
    }
}
