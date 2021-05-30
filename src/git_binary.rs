//! See [`GitBinary`] for the primary entry point.

use anyhow::{bail, Result};
use std::{collections::HashSet, ffi::OsStr, path::Path, process::Command};

use crate::{
    backend::{Backend, Branch, Config, HostBranch, Remote, Snapshot},
    git_ref::GitRef,
    progress::{output_stdout, Progress, Run},
};

/// Containerizes all the naming schemes used by nomad from the wild west of all other git tools,
/// both built-in and third party.
mod namespace {
    use crate::{
        backend::{Branch, Config, HostBranch},
        git_ref::GitRef,
    };

    /// The main name that we declare to be ours and nobody elses. This lays claim to the section
    /// in `git config` and the `refs/{PREFIX}` hierarchy in all git repos!
    const PREFIX: &str = "nomad";

    /// Where information is stored for `git config`.
    pub fn config_key(key: &str) -> String {
        format!("{}.{}", PREFIX, key)
    }

    /// The refspec to fetch remote nomad managed refs as local refs.
    ///
    /// `refs/nomad/rraval/apollo/master` becomes `refs/nomad/apollo/master`.
    ///
    /// `refs/nomad/rraval/boreas/feature` becomes `refs/nomad/boreas/feature`.
    pub fn fetch_refspec(config: &Config) -> String {
        format!(
            "+refs/{prefix}/{user}/*:refs/{prefix}/*",
            prefix = PREFIX,
            user = config.user
        )
    }

    /// The refspec to push local branches as nomad managed refs in the remote.
    ///
    /// When run on host `boreas` that has a branch named `feature`:
    /// `refs/heads/feature` becomes `refs/nomad/rraval/boreas/feature`.
    pub fn push_refspec(config: &Config) -> String {
        format!(
            "+refs/heads/*:refs/{prefix}/{user}/{host}/*",
            prefix = PREFIX,
            user = config.user,
            host = config.host,
        )
    }

    /// A nomad ref in the local clone, which elides the user name for convenience.
    ///
    /// Note that `branch` can be the empty string which conveniently acts as a prefix for parsing
    /// `git show-ref` output.
    #[cfg(test)]
    pub fn local_ref(config: &Config, branch: &str) -> String {
        format!("refs/{}/{}/{}", PREFIX, config.host, branch)
    }

    /// A nomad ref in the remote.
    ///
    /// Note that `branch` can be the empty string which conveniently acts as a prefix for parsing
    /// `git show-ref` output.
    pub fn remote_ref(config: &Config, branch: &str) -> String {
        format!("refs/{}/{}/{}/{}", PREFIX, config.user, config.host, branch)
    }

    pub fn as_host_branch(git_ref: GitRef) -> Result<HostBranch<GitRef>, GitRef> {
        let parts = git_ref.name.split('/').collect::<Vec<_>>();
        match parts.as_slice() {
            ["refs", prefix, host, branch_name] => {
                if prefix != &PREFIX {
                    return Err(git_ref);
                }

                Ok(HostBranch {
                    host: host.to_string(),
                    branch: Branch::str(branch_name),
                    ref_: git_ref,
                })
            }
            _ => Err(git_ref),
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::{
            backend::{Branch, Config, HostBranch},
            git_ref::GitRef,
        };

        use super::{as_host_branch, local_ref};

        /// [`as_host_branch`] should be able to parse ref names produced by [`local_ref`] (they
        /// are duals).
        #[test]
        fn test_create_and_parse() {
            let git_ref = GitRef {
                commit_id: "some_commit_id".to_string(),
                name: local_ref(
                    &Config {
                        user: "user0".to_string(),
                        host: "host0".to_string(),
                    },
                    "branch0",
                ),
            };

            assert_eq!(
                as_host_branch(git_ref.clone()),
                Ok(HostBranch {
                    host: "host0".to_string(),
                    branch: Branch::str("branch0"),
                    ref_: git_ref,
                })
            );
        }
    }
}

/// Implements repository manipulations by delegating to some ambient `git` binary that exists
/// somewhere on the system.
pub struct GitBinary<'progress, 'name> {
    /// Used to actually execute commands while reporting progress to the user.
    progress: &'progress Progress,

    /// The name of the `git` binary to use. Implemented on top of [`Command::new`], so
    /// non-absolute paths are looked up against `$PATH`.
    name: &'name OsStr,

    /// The absolute path to the `.git` directory of the repository.
    git_dir: String,
}

impl<'progress, 'name> GitBinary<'progress, 'name> {
    /// Create a new [`GitBinary`] by finding the `.git` dir relative to `cwd`, which implements
    /// the usual git rules of searching ancestor directories.
    pub fn new(
        progress: &'progress Progress,
        name: &'name str,
        cwd: &Path,
    ) -> Result<GitBinary<'progress, 'name>> {
        let name = name.as_ref();
        let git_dir = progress
            .run(
                Run::Trivial,
                "Resolving .git directory",
                Command::new(name)
                    .current_dir(cwd)
                    .args(&["rev-parse", "--absolute-git-dir"]),
            )
            .and_then(output_stdout)
            .map(LineArity::of)
            .and_then(LineArity::one)?;

        Ok(GitBinary {
            progress,
            name,
            git_dir,
        })
    }

    /// Invoke a git sub-command with an explicit `--git-dir` to make it independent of the working
    /// directory it is invoked from.
    fn command(&self) -> Command {
        let mut command = Command::new(self.name);
        command.args(&["--git-dir", &self.git_dir]);
        command
    }

    /// Wraps `git config` to read a single value from the local git repository.
    ///
    /// Explicitly ignores user level or global config to keep things nice and sealed.
    fn get_config(&self, key: &str) -> Result<Option<String>> {
        self.progress
            .run(
                Run::Trivial,
                format!("Get config {}", key),
                self.command().args(&[
                    "config",
                    "--local",
                    // Use a default to prevent git from returning a non-zero exit code when the value does
                    // not exist.
                    "--default",
                    "",
                    "--get",
                    key,
                ]),
            )
            .and_then(output_stdout)
            .map(LineArity::of)
            .and_then(LineArity::zero_or_one)
    }

    /// Wraps `git config` to write a single value to the local git repository.
    fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.progress.run(
            Run::Trivial,
            format!("Set config {} = {}", key, value),
            self.command()
                .args(&["config", "--local", "--replace-all", key, value]),
        )?;
        Ok(())
    }

    /// Wraps `git fetch` to fetch refs from a given remote into the local repository.
    ///
    /// # Panics
    ///
    /// If `refspecs` is empty, which means git will use the user configured default behaviour
    /// which is definitely not what we want.
    fn fetch_refspecs<Description, RefSpec>(
        &self,
        description: Description,
        remote: &str,
        refspecs: &[RefSpec],
    ) -> Result<()>
    where
        Description: AsRef<str>,
        RefSpec: AsRef<OsStr>,
    {
        assert!(!refspecs.is_empty());
        self.progress.run(
            Run::Notable,
            description,
            self.command().args(&["fetch", remote]).args(refspecs),
        )?;
        Ok(())
    }

    /// Wraps `git push` to push refs from the local repository into the given remote.
    ///
    /// # Panics
    ///
    /// If `refspecs` is empty, which means git will use the user configured default behaviour
    /// which is definitely not what we want.
    fn push_refspecs<Description, RefSpec>(
        &self,
        description: Description,
        remote: &str,
        refspecs: &[RefSpec],
    ) -> Result<()>
    where
        Description: AsRef<str>,
        RefSpec: AsRef<OsStr>,
    {
        assert!(!refspecs.is_empty());
        self.progress.run(
            Run::Notable,
            description,
            self.command().args(&["push", remote]).args(refspecs),
        )?;
        Ok(())
    }

    /// Extract a single `GitRef` for a given `ref_name`.
    #[cfg(test)]
    fn get_ref<Description, RefName>(
        &self,
        description: Description,
        ref_name: RefName,
    ) -> Result<GitRef>
    where
        Description: AsRef<str>,
        RefName: AsRef<str>,
    {
        self.progress
            .run(
                Run::Trivial,
                description,
                self.command()
                    .args(&["show-ref", "--verify", ref_name.as_ref()]),
            )
            .and_then(output_stdout)
            .map(LineArity::of)
            .and_then(LineArity::one)
            .and_then(|line| GitRef::parse_show_ref_line(&line).map_err(Into::into))
    }

    /// List all the non-HEAD refs in the repository as `GitRef`s.
    fn list_refs<Description>(&self, description: Description) -> Result<Vec<GitRef>>
    where
        Description: AsRef<str>,
    {
        let output = self
            .progress
            .run(Run::Trivial, description, self.command().arg("show-ref"))
            .and_then(output_stdout)?;
        output
            .lines()
            .map(|line| GitRef::parse_show_ref_line(line).map_err(Into::into))
            .collect()
    }

    /// Delete a ref from the repository.
    ///
    /// Note that deleting refs on a remote is done via [`GitBinary::push_refspecs`].
    fn delete_ref<Description>(&self, description: Description, git_ref: &GitRef) -> Result<()>
    where
        Description: AsRef<str>,
    {
        let mut command = self.command();
        command.args(&["update-ref", "-d", &git_ref.name, &git_ref.commit_id]);
        self.progress.run(Run::Notable, description, &mut command)?;
        Ok(())
    }
}

/// Implements nomad workflows over an ambient `git` binary.
impl<'progress, 'name> Backend for GitBinary<'progress, 'name> {
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

    fn snapshot(&self) -> Result<Snapshot<Self::Ref>> {
        let refs = self.list_refs("Fetching all refs")?;

        let mut local_branches = HashSet::<Branch>::new();
        let mut host_branches = Vec::<HostBranch<GitRef>>::new();

        for r in refs {
            if let Some(name) = r.name.strip_prefix("refs/heads/") {
                local_branches.insert(Branch::str(name));
            }

            if let Ok(host_branch) = namespace::as_host_branch(r) {
                host_branches.push(host_branch);
            }
        }

        Ok(Snapshot {
            local_branches,
            host_branches,
        })
    }

    fn fetch(&self, config: &Config, remote: &Remote) -> Result<()> {
        self.fetch_refspecs(
            format!("Fetching branches from {}", remote.0),
            &remote.0,
            &[&namespace::fetch_refspec(config)],
        )
    }

    fn push(&self, config: &Config, remote: &Remote) -> Result<()> {
        self.push_refspecs(
            format!("Pushing local branches to {}", remote.0),
            &remote.0,
            &[&namespace::push_refspec(config)],
        )
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
                namespace::remote_ref(config, &host_branch.branch.0)
            ));

            refs.push(host_branch.ref_.clone());
        }

        // Delete from the remote first
        if !refspecs.is_empty() {
            self.push_refspecs(
                format!("Pruning deleted branches at {}", remote.0),
                &remote.0,
                &refspecs,
            )?;
        }

        // ... then delete locally. This order means that interruptions leave the local ref around
        // to be picked up and pruned again.
        //
        // In practice, we do a fetch from the remote first anyways, which would recreate the local
        // ref if this code deleted local refs first and then was interrupted.
        //
        // But that is non-local reasoning and this ordering is theoretically correct.
        for r in refs {
            self.delete_ref(format!("  Prune {}", r.name), &r)?;
        }

        Ok(())
    }
}

/// Utility to parse line based output of various `git` sub-commands.
#[derive(Debug)]
enum LineArity {
    /// The command produced no lines.
    Zero(),
    /// The command produced exactly one line.
    One(String),
    /// The command produced two or more lines.
    Many(String),
}

impl LineArity {
    /// Smart constructor to parse a [`LineArity`] from an arbitrary line.
    ///
    /// Coerces the empty line as [`LineArity::Zero`].
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

    /// The caller expects the output to only have a single line.
    fn one(self) -> Result<String> {
        if let LineArity::One(line) = self {
            Ok(line)
        } else {
            bail!("Expected one line, got {:?}", self);
        }
    }

    /// The caller expects the output to have zero or one line.
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

    use crate::progress::{Progress, Run, Verbosity};

    use super::GitBinary;
    use anyhow::Result;

    const PROGRESS: Progress = Progress::Verbose(Verbosity::CommandAndOutput);

    fn git_init() -> Result<(String, TempDir)> {
        let name = "git".to_owned();
        let tmpdir = tempdir()?;

        PROGRESS.run(
            Run::Notable,
            "",
            Command::new(&name).current_dir(tmpdir.path()).args(&[
                "init",
                "--initial-branch",
                "branch0",
            ]),
        )?;

        Ok((name, tmpdir))
    }

    /// Find the `.git` directory when run from the root of the repo.
    #[test]
    fn toplevel_at_root() -> Result<()> {
        let (name, tmpdir) = git_init()?;

        let git = GitBinary::new(&PROGRESS, &name, tmpdir.path())?;
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

        let git = GitBinary::new(&PROGRESS, &name, subdir.as_path())?;
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
        let git = GitBinary::new(&PROGRESS, &name, tmpdir.path())?;

        let got = git.get_config("test.key")?;
        assert_eq!(got, None);

        Ok(())
    }

    /// Verify read-your-writes.
    #[test]
    fn write_then_read_config() -> Result<()> {
        let (name, tmpdir) = git_init()?;
        let git = GitBinary::new(&PROGRESS, &name, tmpdir.path())?;

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
        backend::{Backend, Branch, Config, HostBranch, Remote},
        git_binary::namespace,
        progress::{Progress, Run, Verbosity},
    };

    use super::{GitBinary, GitRef};

    const GIT: &str = "git";
    const ORIGIN: &str = "origin";
    const BRANCH: &str = "branch0";
    const USER: &str = "user0";

    const PROGRESS: Progress = Progress::Verbose(Verbosity::CommandAndOutput);

    struct GitRemote {
        root_dir: TempDir,
        remote_dir: PathBuf,
        git: GitBinary<'static, 'static>,
    }

    impl GitRemote {
        fn init() -> GitRemote {
            let root_dir = tempdir().unwrap();
            let remote_dir = root_dir.path().join("remote");

            {
                let remote_dir = remote_dir.as_path();

                let git = |args: &[&str]| {
                    PROGRESS
                        .run(
                            Run::Notable,
                            "",
                            Command::new(GIT).current_dir(remote_dir).args(args),
                        )
                        .unwrap();
                };

                create_dir(remote_dir).unwrap();
                git(&["init", "--initial-branch", BRANCH]);

                let file0 = remote_dir.join("file0");
                write(file0, "line0\nline1\n").unwrap();

                git(&["add", "."]);
                git(&["commit", "--author", "Test <test@example.com>", "-m", "commit0"]);
            }

            let git = GitBinary::new(&PROGRESS, GIT, &remote_dir).unwrap();

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

            PROGRESS
                .run(
                    Run::Notable,
                    "",
                    Command::new(GIT)
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
        git: GitBinary<'static, 'static>,
    }

    impl<'a> GitClone<'a> {
        fn remote(&self) -> Remote {
            Remote(ORIGIN.to_owned())
        }

        fn push(&self) {
            self.git.push(&self.config, &self.remote()).unwrap();
        }

        fn fetch(&self) {
            self.git.fetch(&self.config, &self.remote()).unwrap()
        }

        fn prune(&self) {
            let ref_name = namespace::local_ref(&self.config, BRANCH);
            let git_ref = self.git.get_ref("", ref_name).unwrap();

            self.git
                .prune(
                    &self.config,
                    &self.remote(),
                    iter::once(&HostBranch {
                        host: self.config.host.clone(),
                        branch: Branch::str(BRANCH),
                        ref_: git_ref,
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
            .map(|r| r.commit_id.clone())
            .collect::<HashSet<_>>()
    }

    /// Push should put local branches to remote `refs/nomad/{user}/{host}/{branch}`
    #[test]
    fn push() {
        let origin = GitRemote::init();
        let host0 = origin.clone("host0");
        host0.push();

        let refs = origin
            .git
            .list_refs("Local branches should have remote refs")
            .unwrap();

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
            let refs = host1.git.list_refs("Before fetch").unwrap();
            assert_eq!(ref_names(&refs), pre_fetch_refs());
        }

        // host branches ought to be empty here since host1 has not pushed
        assert_eq!(
            host1
                .git
                .snapshot()
                .unwrap()
                .branches_for_host(&host0.config),
            vec![]
        );
        host1.fetch();

        // After fetch, we should have the additional ref
        {
            let refs = host1.git.list_refs("After fetch").unwrap();
            assert_eq!(ref_names(&refs), {
                let mut set = pre_fetch_refs();
                // the additional ref from the host0 push
                set.insert(namespace::local_ref(&host0.config, BRANCH));
                set
            });
            assert_eq!(
                host1
                    .git
                    .snapshot()
                    .unwrap()
                    .branches_for_host(&host0.config),
                vec![Branch::str(BRANCH)]
            );
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
                .list_refs("Remote refs")
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
                .list_refs("Local refs")
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
