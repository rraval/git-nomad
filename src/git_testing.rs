use std::{
    collections::HashSet,
    fs::{create_dir, write},
    path::PathBuf,
};

use tempfile::{tempdir, TempDir};

use crate::{
    git_binary::{git_command, GitBinary},
    git_ref::GitRef,
    progress::{Progress, Run, Verbosity},
    snapshot::PruneFrom,
    types::{Branch, Host, NomadRef, Remote, User},
};

const GIT: &str = "git";
const ORIGIN: &str = "origin";
pub const INITIAL_BRANCH: &str = "master";
const PROGRESS: Progress = Progress::Verbose(Verbosity::CommandAndOutput);

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct GitCommitId {
    commit_id: String,
}

impl From<GitRef> for GitCommitId {
    fn from(git_ref: GitRef) -> Self {
        let GitRef { commit_id, .. } = git_ref;
        Self { commit_id }
    }
}

impl<'user, 'host, 'branch> From<NomadRef<'user, 'host, 'branch, GitRef>>
    for NomadRef<'user, 'host, 'branch, GitCommitId>
{
    fn from(nomad_ref: NomadRef<'user, 'host, 'branch, GitRef>) -> Self {
        Self {
            user: nomad_ref.user,
            host: nomad_ref.host,
            branch: nomad_ref.branch,
            ref_: nomad_ref.ref_.into(),
        }
    }
}

pub struct GitRemote {
    root_dir: TempDir,
    remote_dir: PathBuf,
    git: GitBinary<'static, 'static>,
}

impl GitRemote {
    pub fn init() -> GitRemote {
        let root_dir = tempdir().unwrap();
        let remote_dir = root_dir.path().join("remote");

        {
            let remote_dir = remote_dir.as_path();

            let git = |args: &[&str]| {
                PROGRESS
                    .run(
                        Run::Notable,
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

        let git = GitBinary::new(&PROGRESS, GIT, &remote_dir).unwrap();

        GitRemote {
            root_dir,
            remote_dir,
            git,
        }
    }

    pub fn clone<'a>(&'a self, user: &'static str, host: &'static str) -> GitClone<'a> {
        let clone_dir = {
            let mut dir = PathBuf::from(self.root_dir.path());
            dir.push("clones");
            dir.push(host);
            dir
        };

        PROGRESS
            .run(
                Run::Notable,
                "",
                git_command(GIT)
                    .current_dir(&self.root_dir)
                    .arg("clone")
                    .args(&["--origin", ORIGIN])
                    .arg(&self.remote_dir)
                    .arg(&clone_dir),
            )
            .unwrap();

        let git = GitBinary::new(&PROGRESS, GIT, &clone_dir).unwrap();

        GitClone {
            _remote: self,
            _clone_dir: clone_dir,
            user: User::from(user),
            host: Host::from(host),
            git,
        }
    }

    pub fn nomad_refs(&self) -> HashSet<NomadRef<GitCommitId>> {
        self.git
            .list_refs("")
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

pub struct GitClone<'a> {
    _remote: &'a GitRemote,
    _clone_dir: PathBuf,
    pub user: User<'static>,
    pub host: Host<'static>,
    pub git: GitBinary<'static, 'static>,
}

impl<'a> GitClone<'a> {
    pub fn remote(&self) -> Remote {
        Remote::from(ORIGIN)
    }

    pub fn push(&self) {
        self.git
            .push_nomad_refs(&self.user, &self.host, &self.remote())
            .unwrap();
    }

    pub fn fetch(&self) {
        self.git
            .fetch_nomad_refs(&self.user, &self.remote())
            .unwrap()
    }

    pub fn list(&self) -> impl Iterator<Item = NomadRef<GitRef>> {
        self.git
            .list_nomad_refs(&self.user, &self.remote())
            .unwrap()
    }

    pub fn prune_local_and_remote<'b, B: IntoIterator<Item = &'b str>>(&self, branch_names: B) {
        let prune_from = branch_names.into_iter().map(|name| {
            let nomad_ref = NomadRef::<()> {
                user: self.user.clone(),
                host: self.host.clone(),
                branch: Branch::from(name.to_string()),
                ref_: (),
            };

            let ref_name = nomad_ref.to_git_local_ref();

            let nomad_ref = NomadRef {
                user: nomad_ref.user,
                host: nomad_ref.host,
                branch: nomad_ref.branch,
                ref_: self.git.get_ref("", ref_name).unwrap(),
            };

            PruneFrom::LocalAndRemote(nomad_ref)
        });

        self.git
            .prune_nomad_refs(&self.remote(), prune_from)
            .unwrap();
    }

    pub fn get_nomad_ref<'branch>(
        &'a self,
        branch: &'branch str,
    ) -> Option<NomadRef<'a, 'a, 'branch, GitCommitId>> {
        self.git
            .get_ref("", format!("refs/heads/{}", branch))
            .ok()
            .map(|git_ref| NomadRef {
                user: self.user.clone(),
                host: self.host.clone(),
                branch: Branch::from(branch),
                ref_: git_ref.into(),
            })
    }

    pub fn nomad_refs(&self) -> HashSet<NomadRef<GitCommitId>> {
        self.git
            .list_refs(&self.host.0)
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
