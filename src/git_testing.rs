use std::{
    borrow::Cow,
    collections::HashSet,
    fs::{create_dir, write},
    path::{Path, PathBuf},
};

use tempfile::{TempDir, tempdir};

use crate::{
    git_binary::{GitBinary, LineArity, git_command},
    git_ref::GitRef,
    renderer::test::NoRenderer,
    snapshot::PruneFrom,
    types::{Branch, Host, NomadRef, Remote, User},
    verbosity::{Verbosity, output_stdout, run_notable},
};

const GIT: &str = "git";
const ORIGIN: &str = "origin";
pub const INITIAL_BRANCH: &str = "master";

/// Only stores the hexadecimal git commit ID.
///
/// Meant to be used as the `Ref` in a `NomadRef<Ref>`.
///
/// Useful for comparing just the commit IDs without caring what the [`GitRef::name`]` actually
/// was.
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct GitCommitId(pub String);

impl From<GitRef> for GitCommitId {
    fn from(git_ref: GitRef) -> Self {
        let GitRef { commit_id, .. } = git_ref;
        Self(commit_id)
    }
}

impl<'a> From<NomadRef<'a, GitRef>> for NomadRef<'a, GitCommitId> {
    fn from(nomad_ref: NomadRef<'a, GitRef>) -> Self {
        Self {
            user: nomad_ref.user,
            host: nomad_ref.host,
            branch: nomad_ref.branch,
            ref_: nomad_ref.ref_.into(),
        }
    }
}

/// Simulates a git remote in a temporary directory.
pub struct GitRemote {
    root_dir: TempDir,
    remote_dir: PathBuf,
    pub git: GitBinary<'static>,
}

impl GitRemote {
    /// Initializes a git remote in a temporary directory.
    pub fn init(verbosity: Option<Verbosity>) -> GitRemote {
        let root_dir = tempdir().unwrap();
        let remote_dir = root_dir.path().join("remote");

        {
            let remote_dir = remote_dir.as_path();

            let git = |args: &[&str]| {
                run_notable(
                    &mut NoRenderer,
                    verbosity,
                    "",
                    git_command(GIT).current_dir(remote_dir).args(args),
                )
                .unwrap();
            };

            create_dir(remote_dir).unwrap();
            git(&["init", "--initial-branch", INITIAL_BRANCH]);

            let file0 = remote_dir.join("file0");
            write(file0, "line0\nline1\n").unwrap();

            git(&["add", "."]);
            git(&["commit", "-m", "commit0"]);
        }

        let git = GitBinary::new(&mut NoRenderer, verbosity, Cow::from(GIT), &remote_dir).unwrap();

        GitRemote {
            root_dir,
            remote_dir,
            git,
        }
    }

    pub fn working_directory(&self) -> &Path {
        &self.remote_dir
    }

    fn verbosity(&self) -> Option<Verbosity> {
        self.git.verbosity
    }

    /// Creates a git clone that can act like a [`Host`].
    pub fn clone<'a>(&'a self, user: &'static str, host: &'static str) -> GitClone<'a> {
        let clone_dir = {
            let mut dir = PathBuf::from(self.root_dir.path());
            dir.push("clones");
            dir.push(host);
            dir
        };

        run_notable(
            &mut NoRenderer,
            self.verbosity(),
            "",
            git_command(GIT)
                .current_dir(&self.root_dir)
                .arg("clone")
                .args(["--origin", ORIGIN])
                .arg(&self.remote_dir)
                .arg(&clone_dir),
        )
        .unwrap();

        let git = GitBinary::new(
            &mut NoRenderer,
            self.verbosity(),
            Cow::from(GIT),
            &clone_dir,
        )
        .unwrap();

        GitClone {
            git_remote: self,
            _clone_dir: clone_dir,
            remote: Remote::from(ORIGIN),
            user: User::from(user),
            host: Host::from(host),
            git,
        }
    }

    /// List all nomad managed refs in the remote.
    pub fn nomad_refs(&self) -> HashSet<NomadRef<'_, GitCommitId>> {
        self.git
            .list_refs(&mut NoRenderer, "")
            .unwrap()
            .into_iter()
            .filter_map(|git_ref| {
                NomadRef::<GitRef>::from_git_remote_ref(git_ref)
                    .ok()
                    .map(Into::into)
            })
            .collect::<HashSet<_>>()
    }
}

/// Acts like a separate [`Host`] in a temporary directory.
pub struct GitClone<'a> {
    git_remote: &'a GitRemote,
    _clone_dir: PathBuf,
    pub remote: Remote<'static>,
    pub user: User<'static>,
    pub host: Host<'static>,
    pub git: GitBinary<'static>,
}

impl<'a> GitClone<'a> {
    /// Get the commit ID at HEAD.
    pub fn current_commit(&self) -> GitCommitId {
        let commit_id = run_notable(
            &mut NoRenderer,
            self.git_remote.verbosity(),
            "Get current commit",
            self.git.command().arg("rev-parse").arg("HEAD"),
        )
        .and_then(output_stdout)
        .map(LineArity::from)
        .and_then(LineArity::one)
        .unwrap();

        GitCommitId(commit_id)
    }

    /// Push all nomad managed refs to the remote.
    pub fn push(&self) {
        self.git
            .push_nomad_refs(&mut NoRenderer, &self.user, &self.host, &self.remote)
            .unwrap();
    }

    /// Fetch all nomad managed refs from the remote.
    pub fn fetch(&self) {
        self.git
            .fetch_nomad_refs(&mut NoRenderer, &self.user, &self.remote)
            .unwrap()
    }

    /// List all nomad managed refs in the current clone.
    pub fn list(&self) -> Vec<NomadRef<'_, GitRef>> {
        self.git
            .list_nomad_refs(&mut NoRenderer, &self.user, &self.remote)
            .unwrap()
            // Limitations of Rust RPIT prevent returning impl Iterator directly, since the borrow
            // checker conservatively assumes that the `NoRenderer` is borrowed inside the iterator.
            .collect()
    }

    /// Delete the nomad managed refs backed by `branch_names` from both the local and remote.
    pub fn prune_local_and_remote(&'a self, branch_names: impl IntoIterator<Item = &'a str>) {
        let prune_from = branch_names.into_iter().map(|name| {
            let nomad_ref = NomadRef::<()> {
                user: self.user.always_borrow(),
                host: self.host.always_borrow(),
                branch: Branch::from(name),
                ref_: (),
            };

            let ref_name = nomad_ref.to_git_local_ref();

            let nomad_ref = NomadRef {
                user: nomad_ref.user,
                host: nomad_ref.host,
                branch: nomad_ref.branch,
                ref_: self.git.get_ref(&mut NoRenderer, "", ref_name).unwrap(),
            };

            PruneFrom::LocalAndRemote(nomad_ref)
        });

        self.git
            .prune_nomad_refs(&mut NoRenderer, &self.remote, prune_from)
            .unwrap();
    }

    /// Resolve a specific nomad managed ref in the local clone by `branch` name.
    pub fn get_nomad_ref(&'a self, branch: &'a str) -> Option<NomadRef<'a, GitCommitId>> {
        self.git
            .get_ref(&mut NoRenderer, "", format!("refs/heads/{}", branch))
            .ok()
            .map(|git_ref| NomadRef {
                user: self.user.always_borrow(),
                host: self.host.always_borrow(),
                branch: Branch::from(branch),
                ref_: git_ref.into(),
            })
    }

    /// Get all nomad managed refs in the local clone.
    pub fn nomad_refs(&self) -> HashSet<NomadRef<'_, GitCommitId>> {
        self.git
            .list_refs(&mut NoRenderer, &self.host.0)
            .unwrap()
            .into_iter()
            .filter_map(|git_ref| {
                NomadRef::<GitRef>::from_git_local_ref(&self.user, git_ref)
                    .ok()
                    .map(Into::into)
            })
            .collect::<HashSet<_>>()
    }
}
