use anyhow::{bail, Context, Result};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};

use crate::backend::{Backend, Config, HostBranch, LocalBranch, Remote};

mod namespace {
    use crate::backend::Config;

    const PREFIX: &str = "nomad";

    pub fn config_key(key: &str) -> String {
        format!("{}.{}", PREFIX, key)
    }

    pub fn fetch_refspec(config: &Config) -> String {
        format!(
            "+refs/{prefix}/{user}/*:refs/{prefix}/*",
            prefix = PREFIX,
            user = config.user
        )
    }

    pub fn push_refspec(config: &Config) -> String {
        format!(
            "+refs/heads/*:refs/{prefix}/{user}/{host}/*",
            prefix = PREFIX,
            user = config.user,
            host = config.host,
        )
    }

    pub fn local_ref(config: &Config, branch: &str) -> String {
        format!("refs/{}/{}/{}", PREFIX, config.host, branch)
    }

    pub fn remote_ref(config: &Config, branch: &str) -> String {
        format!("refs/{}/{}/{}/{}", PREFIX, config.user, config.host, branch)
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct GitRef {
    commit_id: Option<String>,
    name: String,
}

impl GitRef {
    fn parse_show_ref_line(line: &str) -> GitRef {
        let mut parts = line.split(' ').map(String::from).collect::<Vec<_>>();
        let name = parts.pop().expect("Missing ref name");
        let commit_id = parts.pop().expect("Missing ref commit ID");
        assert!(
            parts.is_empty(),
            "Unexpected show-ref line format: {}",
            line
        );
        GitRef {
            commit_id: Some(commit_id),
            name,
        }
    }
}

pub struct GitBinary<'a> {
    name: &'a OsStr,
    git_dir: String,
}

impl<'a> GitBinary<'a> {
    pub fn new(name: &'a str, cwd: &Path) -> Result<GitBinary<'a>> {
        let name = name.as_ref();
        let git_dir = check_output(
            Command::new(name)
                .current_dir(cwd)
                .args(&["rev-parse", "--absolute-git-dir"]),
        )
        .map(LineArity::of)
        .and_then(LineArity::one)?;

        Ok(GitBinary { name, git_dir })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(self.name);
        command.args(&["--git-dir", &self.git_dir]);
        command
    }

    fn get_config(&self, key: &str) -> Result<Option<String>> {
        check_output(self.command().args(&[
            "config",
            "--local",
            // Use a default to prevent git from returning a non-zero exit code when the value does
            // not exist.
            "--default",
            "",
            "--get",
            key,
        ]))
        .map(LineArity::of)
        .and_then(LineArity::zero_or_one)
    }

    fn set_config(&self, key: &str, value: &str) -> Result<()> {
        check_run(
            self.command()
                .args(&["config", "--local", "--replace-all", key, value]),
        )?;
        Ok(())
    }

    fn fetch_refspecs<S: AsRef<OsStr>>(&self, remote: &str, refspecs: &[S]) -> Result<()> {
        check_run(self.command().args(&["fetch", remote]).args(refspecs))?;
        Ok(())
    }

    fn push_refspecs<S: AsRef<OsStr>>(&self, remote: &str, refspecs: &[S]) -> Result<()> {
        check_run(self.command().args(&["push", remote]).args(refspecs))?;
        Ok(())
    }

    fn list_refs(&self) -> Result<Vec<GitRef>> {
        let output = check_output(self.command().arg("show-ref"))?;
        Ok(output.lines().map(GitRef::parse_show_ref_line).collect())
    }

    fn delete_ref(&self, git_ref: &GitRef) -> Result<()> {
        let mut command = self.command();
        command.args(&["update-ref", "-d", &git_ref.name]);
        if let Some(ref commit_id) = git_ref.commit_id {
            command.arg(commit_id);
        }

        check_run(&mut command)?;
        Ok(())
    }
}

impl<'a> Backend for GitBinary<'a> {
    type Ref = GitRef;

    fn read_config(&self) -> Result<Option<Config>> {
        let get = |k: &str| self.get_config(&namespace::config_key(k));

        let user = get("user")?;
        let host = get("host")?;

        match (user, host) {
            (Some(user), Some(host)) => Ok(Some(Config { user, host })),
            (None, None) => Ok(None),
            (user, host) => {
                bail!("Partial configuration {:?} {:?}", user, host)
            }
        }
    }

    fn write_config(&self, config: &Config) -> Result<()> {
        let set = |k: &str, v: &str| self.set_config(&namespace::config_key(k), v);

        set("user", &config.user)?;
        set("host", &config.host)?;

        Ok(())
    }

    fn fetch(
        &self,
        config: &Config,
        remote: &Remote,
    ) -> Result<(HashSet<LocalBranch>, HashSet<HostBranch<GitRef>>)> {
        self.fetch_refspecs(&remote.0, &[&namespace::fetch_refspec(config)])?;
        let refs = self.list_refs()?;

        let mut local_branches = HashSet::<LocalBranch>::new();
        let mut host_branches = HashSet::<HostBranch<GitRef>>::new();

        for r in refs {
            if let Some(name) = r.name.strip_prefix("refs/heads/") {
                local_branches.insert(LocalBranch(name.to_string()));
            }

            if let Some(name) = r.name.strip_prefix(&namespace::local_ref(&config, "")) {
                let name = name.to_string();
                host_branches.insert(HostBranch { name, ref_: r });
            }
        }

        Ok((local_branches, host_branches))
    }

    fn push(&self, config: &Config, remote: &Remote) -> Result<()> {
        self.push_refspecs(&remote.0, &[&namespace::push_refspec(config)])
    }

    fn prune<'b, Prune>(&self, config: &Config, remote: &Remote, prune: Prune) -> Result<()>
    where
        Prune: Iterator<Item = &'b HostBranch<GitRef>>,
    {
        let mut refspecs = Vec::<String>::new();
        let mut refs = Vec::<GitRef>::new();

        for host_branch in prune {
            refspecs.push(format!(
                ":{}",
                namespace::remote_ref(config, &host_branch.name)
            ));

            refs.push(host_branch.ref_.clone());
        }

        // Delete from the remote first
        self.push_refspecs(&remote.0, &refspecs)?;

        // ... then delete locally. This order means that interruptions leave the local ref around
        // to be picked up and pruned again.
        //
        // In practice, we do a fetch from the remote first anyways, which would recreate the local
        // ref if this code deleted local refs first and then was interrupted.
        //
        // But that is non-local reasoning and this ordering is theoretically correct.
        for r in refs {
            self.delete_ref(&r)?;
        }

        Ok(())
    }
}

fn check_run(command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("Running {:?}", command))?;

    if !output.status.success() {
        bail!("command failure {:#?}", output);
    }

    Ok(output)
}

fn check_output(command: &mut Command) -> Result<String> {
    let output = check_run(command)?;
    Ok(String::from_utf8(output.stdout)?)
}

#[derive(Debug)]
enum LineArity {
    Zero(),
    One(String),
    Many(String),
}

impl LineArity {
    fn of(string: String) -> LineArity {
        let mut lines = string.lines().map(String::from).collect::<Vec<_>>();
        let last = lines.pop();

        match last {
            None => LineArity::Zero(),
            Some(last) => {
                if lines.is_empty() {
                    if last.is_empty() {
                        LineArity::Zero()
                    } else {
                        LineArity::One(last)
                    }
                } else {
                    LineArity::Many(string)
                }
            }
        }
    }

    fn one(self) -> Result<String> {
        if let LineArity::One(line) = self {
            Ok(line)
        } else {
            bail!("Expected one line, got {:?}", self);
        }
    }

    fn zero_or_one(self) -> Result<Option<String>> {
        match self {
            LineArity::Zero() => Ok(None),
            LineArity::One(line) => Ok(Some(line)),
            LineArity::Many(string) => bail!("Expected 0 or 1 line, got {:?}", string),
        }
    }
}

#[cfg(test)]
mod test_impl {
    use std::{fs::create_dir, process::Command};

    use tempfile::{tempdir, TempDir};

    use super::{check_output, GitBinary};
    use anyhow::Result;

    fn git_init() -> Result<(String, TempDir)> {
        let name = "git".to_owned();
        let tmpdir = tempdir()?;

        check_output(
            Command::new(&name)
                .current_dir(tmpdir.path())
                .args(&["init"]),
        )?;

        Ok((name, tmpdir))
    }

    /// Find the `.git` directory when run from the root of the repo.
    #[test]
    fn toplevel_at_root() -> Result<()> {
        let (name, tmpdir) = git_init()?;

        let git = GitBinary::new(&name, tmpdir.path())?;
        assert_eq!(
            Some(git.git_dir.as_str()),
            tmpdir.path().join(".git").to_str()
        );

        Ok(())
    }

    /// Find the `.git` directory when run from a subdirectory of the repo.
    #[test]
    fn toplevel_in_subdir() -> Result<()> {
        let (name, tmpdir) = git_init()?;
        let subdir = tmpdir.path().join("subdir");
        create_dir(&subdir)?;

        let git = GitBinary::new(&name, subdir.as_path())?;
        assert_eq!(
            Some(git.git_dir.as_str()),
            tmpdir.path().join(".git").to_str(),
        );

        Ok(())
    }

    /// `get_config` should handle missing configuration.
    #[test]
    fn read_empty_config() -> Result<()> {
        let (name, tmpdir) = git_init()?;
        let git = GitBinary::new(&name, tmpdir.path())?;

        let got = git.get_config("test.key")?;
        assert_eq!(got, None);

        Ok(())
    }

    /// Verify read-your-writes.
    #[test]
    fn write_then_read_config() -> Result<()> {
        let (name, tmpdir) = git_init()?;
        let git = GitBinary::new(&name, tmpdir.path())?;

        git.set_config("test.key", "testvalue")?;
        let got = git.get_config("test.key")?;

        assert_eq!(got, Some("testvalue".to_string()));

        Ok(())
    }
}

#[cfg(test)]
mod test_backend {
    use std::{
        collections::HashSet,
        fs::{create_dir, write},
        iter,
        path::PathBuf,
        process::Command,
    };

    use tempfile::{tempdir, TempDir};

    use crate::{
        backend::{Backend, Config, HostBranch, LocalBranch, Remote},
        git_binary::namespace,
    };

    use super::{check_run, GitBinary, GitRef};

    const GIT: &str = "git";
    const ORIGIN: &str = "origin";
    const BRANCH: &str = "branch0";
    const USER: &str = "user0";

    struct GitRemote {
        root_dir: TempDir,
        remote_dir: PathBuf,
        git: GitBinary<'static>,
    }

    impl GitRemote {
        fn init() -> GitRemote {
            let root_dir = tempdir().unwrap();
            let remote_dir = root_dir.path().join("remote");

            {
                let remote_dir = remote_dir.as_path();

                let git = |args: &[&str]| {
                    check_run(Command::new(GIT).current_dir(remote_dir).args(args)).unwrap();
                };

                create_dir(remote_dir).unwrap();
                git(&["init", "--initial-branch", BRANCH]);

                let file0 = remote_dir.join("file0");
                write(file0, "line0\nline1\n").unwrap();

                git(&["add", "."]);
                git(&["commit", "-m", "commit0"]);
            }

            let git = GitBinary::new(GIT, &remote_dir).unwrap();

            GitRemote {
                root_dir,
                remote_dir,
                git,
            }
        }

        fn clone<'a>(&'a self, host: &str) -> GitClone<'a> {
            let clone_dir = {
                let mut dir = PathBuf::from(self.root_dir.path());
                dir.push("clones");
                dir.push(host);
                dir
            };

            check_run(
                Command::new(GIT)
                    .current_dir(&self.root_dir)
                    .arg("clone")
                    .args(&["--origin", ORIGIN])
                    .arg(&self.remote_dir)
                    .arg(&clone_dir),
            )
            .unwrap();

            let git = GitBinary::new(GIT, &clone_dir).unwrap();

            GitClone {
                _remote: self,
                _clone_dir: clone_dir,
                config: Config {
                    user: USER.to_owned(),
                    host: host.to_owned(),
                },
                git,
            }
        }
    }

    struct GitClone<'a> {
        _remote: &'a GitRemote,
        _clone_dir: PathBuf,
        config: Config,
        git: GitBinary<'static>,
    }

    impl<'a> GitClone<'a> {
        fn remote(&self) -> Remote {
            Remote(ORIGIN.to_owned())
        }

        fn push(&self) {
            self.git.push(&self.config, &self.remote()).unwrap();
        }

        fn fetch(&self) -> (HashSet<LocalBranch>, HashSet<HostBranch<GitRef>>) {
            self.git.fetch(&self.config, &self.remote()).unwrap()
        }

        fn prune(&self) {
            self.git
                .prune(
                    &self.config,
                    &self.remote(),
                    iter::once(&HostBranch {
                        name: BRANCH.to_string(),
                        ref_: GitRef {
                            commit_id: None,
                            name: namespace::local_ref(&self.config, BRANCH),
                        },
                    }),
                )
                .unwrap();
        }
    }

    fn ref_names(refs: &[GitRef]) -> HashSet<String> {
        refs.iter().map(|r| r.name.clone()).collect::<HashSet<_>>()
    }

    fn ref_commit_ids(refs: &[GitRef]) -> HashSet<String> {
        refs.iter()
            .filter_map(|r| r.commit_id.clone())
            .collect::<HashSet<_>>()
    }

    /// Push should put local branches to remote `refs/nomad/{user}/{host}/{branch}`
    #[test]
    fn push() {
        let origin = GitRemote::init();
        let host0 = origin.clone("host0");
        host0.push();

        let refs = origin.git.list_refs().unwrap();

        assert_eq!(ref_names(&refs), {
            let mut set: HashSet<String> = HashSet::new();
            set.insert(format!("refs/heads/{}", BRANCH));
            set.insert(namespace::remote_ref(&host0.config, BRANCH));
            set
        });

        // even though there are 2 refs above, both should be pointing to the same commit
        assert_eq!(ref_commit_ids(&refs).len(), 1);
    }

    /// Fetch should pull refs for all hosts that have pushed under the configured user under
    /// `refs/nomad/{host}/{branch}`
    #[test]
    fn fetch() {
        let origin = GitRemote::init();

        let host0 = origin.clone("host0");
        host0.push();

        let host1 = origin.clone("host1");

        // Before fetch, the host1 clone should only have the local and remote branch
        let pre_fetch_refs = || {
            let mut set: HashSet<String> = HashSet::new();

            // the local branch
            set.insert(format!("refs/heads/{}", BRANCH));

            // the remote branch and what's checked out
            set.insert(format!("refs/remotes/{}/{}", ORIGIN, BRANCH));
            set.insert(format!("refs/remotes/{}/HEAD", ORIGIN));

            set
        };

        {
            let refs = host1.git.list_refs().unwrap();
            assert_eq!(ref_names(&refs), pre_fetch_refs());
        }

        // host branches ought to be empty here since host1 has not pushed
        let (local_branches, host_branches) = host1.fetch();
        assert_eq!(local_branches, {
            let mut set = HashSet::<LocalBranch>::new();
            set.insert(LocalBranch(BRANCH.to_string()));
            set
        });
        assert_eq!(host_branches, HashSet::new());

        // After fetch, we should have the additional ref
        {
            let refs = host1.git.list_refs().unwrap();
            assert_eq!(ref_names(&refs), {
                let mut set = pre_fetch_refs();
                // the additional ref from the host0 push
                set.insert(namespace::local_ref(&host0.config, BRANCH));
                set
            });
        }
    }

    /// Pushing should create nomad refs in the remote.
    /// Fetching should create nomad refs locally.
    /// Pruning should delete refs in the local and remote.
    #[test]
    fn push_fetch_prune() {
        let remote = GitRemote::init();
        let local = remote.clone("local");

        let remote_nomad_refs = || {
            remote
                .git
                .list_refs()
                .unwrap()
                .into_iter()
                .filter_map(|r| {
                    r.name
                        .strip_prefix(&namespace::remote_ref(&local.config, ""))
                        .map(String::from)
                })
                .collect::<HashSet<_>>()
        };

        let local_nomad_refs = || {
            local
                .git
                .list_refs()
                .unwrap()
                .into_iter()
                .filter_map(|r| {
                    r.name
                        .strip_prefix(&namespace::local_ref(&local.config, ""))
                        .map(String::from)
                })
                .collect::<HashSet<_>>()
        };

        let empty_set = HashSet::new();
        let branch_set = {
            let mut set = HashSet::new();
            set.insert(BRANCH.to_string());
            set
        };

        // In the beginning, there are no nomad refs
        assert_eq!(remote_nomad_refs(), empty_set);
        assert_eq!(local_nomad_refs(), empty_set);

        // Pushing creates a remote nomad ref, but local remains empty
        local.push();
        assert_eq!(remote_nomad_refs(), branch_set);
        assert_eq!(local_nomad_refs(), empty_set);

        // Fetching creates a local nomad ref
        local.fetch();
        assert_eq!(remote_nomad_refs(), branch_set);
        assert_eq!(local_nomad_refs(), branch_set);

        // Pruning removes the ref remotely and locally
        local.prune();
        assert_eq!(remote_nomad_refs(), empty_set);
        assert_eq!(local_nomad_refs(), empty_set);
    }
}
