//! See [`GitBinary`] for the primary entry point.

use anyhow::{bail, Result};
use std::{collections::HashSet, ffi::OsStr, path::Path, process::Command};

use crate::{
    git_ref::GitRef,
    snapshot::{PruneFrom, Snapshot},
    types::{Branch, Host, NomadRef, Remote, User},
    verbosity::{is_output_allowed, output_stdout, run_notable, run_trivial, Verbosity},
};

/// Attempt to run a git binary without impurities from the environment slipping in.
///
/// Doing this correctly seems to have a long and complicated history:
/// https://stackoverflow.com/a/67512433
pub fn git_command<S: AsRef<OsStr>>(name: S) -> Command {
    let mut command = Command::new(name);

    let author_name = "git-nomad";
    let author_email = "git-nomad@invalid";
    let author_date = "1970-01-01T00:00:00";

    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_NOGLOBAL", "1")
        .env("HOME", "")
        .env("XDG_CONFIG_HOME", "")
        .env("GIT_AUTHOR_NAME", author_name)
        .env("GIT_AUTHOR_EMAIL", author_email)
        .env("GIT_AUTHOR_DATE", author_date)
        .env("GIT_COMMITTER_NAME", author_name)
        .env("GIT_COMMITTER_EMAIL", author_email)
        .env("GIT_COMMITTER_DATE", author_date);
    command
}

/// Containerizes all the naming schemes used by nomad from the wild west of all other git tools,
/// both built-in and third party.
mod namespace {
    use crate::{
        git_ref::GitRef,
        types::{Branch, Host, NomadRef, User},
    };

    /// The main name that we declare to be ours and nobody elses. This lays claim to the section
    /// in `git config` and the `refs/{PREFIX}` hierarchy in all git repos!
    const PREFIX: &str = "nomad";

    /// Where information is stored for `git config`.
    pub fn config_key(key: &str) -> String {
        format!("{}.{}", PREFIX, key)
    }

    /// The refspec to list remote nomad managed refs.
    pub fn list_refspec(user: &User) -> String {
        format!("refs/{prefix}/{user}/*", prefix = PREFIX, user = user.0)
    }

    /// The refspec to fetch remote nomad managed refs as local refs.
    ///
    /// `refs/nomad/rraval/apollo/master` becomes `refs/nomad/apollo/master`.
    ///
    /// `refs/nomad/rraval/boreas/feature` becomes `refs/nomad/boreas/feature`.
    pub fn fetch_refspec(user: &User) -> String {
        format!(
            "+{remote_pattern}:refs/{prefix}/*",
            remote_pattern = list_refspec(user),
            prefix = PREFIX,
        )
    }

    /// The refspec to push local branches as nomad managed refs in the remote.
    ///
    /// When run on host `boreas` that has a branch named `feature`:
    /// `refs/heads/feature` becomes `refs/nomad/rraval/boreas/feature`.
    pub fn push_refspec(user: &User, host: &Host) -> String {
        format!(
            "+refs/heads/*:refs/{prefix}/{user}/{host}/*",
            prefix = PREFIX,
            user = user.0,
            host = host.0,
        )
    }

    impl<'user, 'host, 'branch, Ref> NomadRef<'user, 'host, 'branch, Ref> {
        /// A nomad ref in the local clone, which elides the user name for convenience.
        #[cfg(test)]
        pub fn to_git_local_ref(&self) -> String {
            format!("refs/{}/{}/{}", PREFIX, self.host.0, self.branch.0)
        }

        /// A nomad ref in the remote. The remote may have many users that all use `git-nomad` and
        /// so shouldn't step on each others toes.
        pub fn to_git_remote_ref(&self) -> String {
            format!(
                "refs/{}/{}/{}/{}",
                PREFIX, self.user.0, self.host.0, self.branch.0
            )
        }
    }

    impl NomadRef<'_, '_, '_, GitRef> {
        /// Constructs a [`NomadRef`] from a git ref in the local clone, which elides the user name
        /// for convenience.
        pub fn from_git_local_ref<'user>(
            user: &'user User,
            git_ref: GitRef,
        ) -> Result<NomadRef<'user, 'static, 'static, GitRef>, GitRef> {
            let parts = git_ref.name.split('/').collect::<Vec<_>>();
            match parts.as_slice() {
                ["refs", prefix, host, branch_name] => {
                    if prefix != &PREFIX {
                        return Err(git_ref);
                    }

                    Ok(NomadRef {
                        user: user.always_borrow(),
                        host: Host::from(host.to_string()),
                        branch: Branch::from(branch_name.to_string()),
                        ref_: git_ref,
                    })
                }
                _ => Err(git_ref),
            }
        }

        /// Constructs a [`NomadRef`] from a git ref in the remote, which includes the user as part
        /// of the ref name.
        pub fn from_git_remote_ref(
            git_ref: GitRef,
        ) -> Result<NomadRef<'static, 'static, 'static, GitRef>, GitRef> {
            let parts = git_ref.name.split('/').collect::<Vec<_>>();
            match parts.as_slice() {
                ["refs", prefix, user, host, branch_name] => {
                    if prefix != &PREFIX {
                        return Err(git_ref);
                    }

                    Ok(NomadRef {
                        user: User::from(user.to_string()),
                        host: Host::from(host.to_string()),
                        branch: Branch::from(branch_name.to_string()),
                        ref_: git_ref,
                    })
                }
                _ => Err(git_ref),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::{
            git_ref::GitRef,
            types::{Branch, Host, NomadRef, User},
        };

        const USER: &str = "user0";
        const HOST: &str = "host0";
        const BRANCH: &str = "branch0";

        /// [`NomadRef::from_git_local_ref`] should be able to parse ref names produced by
        /// [`NomadRef::to_git_local_ref`] (they are duals).
        #[test]
        fn test_to_and_from_local_ref() {
            let local_ref_name = NomadRef {
                user: User::from(USER),
                host: Host::from(HOST),
                branch: Branch::from(BRANCH),
                ref_: (),
            }
            .to_git_local_ref();

            let local_git_ref = GitRef {
                commit_id: "some_commit_id".to_string(),
                name: local_ref_name,
            };

            let user = &User::from(USER);
            let nomad_ref = NomadRef::<GitRef>::from_git_local_ref(user, local_git_ref).unwrap();

            assert_eq!(&nomad_ref.user.0, USER);
            assert_eq!(&nomad_ref.host.0, HOST);
            assert_eq!(&nomad_ref.branch.0, BRANCH);
        }

        /// [`NomadRef::from_git_remote_ref`] should be able to parse ref names produced by
        /// [`NomadRef::to_git_local_ref`] (they are duals).
        #[test]
        fn test_to_and_from_remote_ref() {
            let remote_ref_name = NomadRef {
                user: User::from(USER),
                host: Host::from(HOST),
                branch: Branch::from(BRANCH),
                ref_: (),
            }
            .to_git_remote_ref();

            let remote_git_ref = GitRef {
                commit_id: "some_commit_id".to_string(),
                name: remote_ref_name,
            };

            let nomad_ref = NomadRef::<GitRef>::from_git_remote_ref(remote_git_ref).unwrap();

            assert_eq!(&nomad_ref.user.0, USER);
            assert_eq!(&nomad_ref.host.0, HOST);
            assert_eq!(&nomad_ref.branch.0, BRANCH);
        }

        /// [`NomadRef::from_git_remote_ref`] should refuse to parse refs with a different prefix.
        #[test]
        fn test_from_remote_ref_wrong_prefix() {
            let remote_git_ref = GitRef {
                commit_id: "some_commit_id".to_string(),
                name: "refs/something/user/host/branch".to_string(),
            };

            let parsed = NomadRef::<GitRef>::from_git_remote_ref(remote_git_ref);
            assert!(parsed.is_err());
        }
    }
}

/// Implements repository manipulations by delegating to some ambient `git` binary that exists
/// somewhere on the system.
#[derive(PartialEq, Eq)]
pub struct GitBinary<'name> {
    /// Used to actually execute commands while reporting progress to the user.
    verbosity: Option<Verbosity>,

    /// The name of the `git` binary to use. Implemented on top of [`Command::new`], so
    /// non-absolute paths are looked up against `$PATH`.
    name: &'name OsStr,

    /// The absolute path to the `.git` directory of the repository.
    git_dir: String,
}

impl<'name> GitBinary<'name> {
    /// Create a new [`GitBinary`] by finding the `.git` dir relative to `cwd`, which implements
    /// the usual git rules of searching ancestor directories.
    pub fn new(
        verbosity: Option<Verbosity>,
        name: &'name str,
        cwd: &Path,
    ) -> Result<GitBinary<'name>> {
        let name = name.as_ref();
        let git_dir = run_trivial(
            verbosity,
            "Resolving .git directory",
            git_command(name)
                .current_dir(cwd)
                .args(&["rev-parse", "--absolute-git-dir"]),
        )
        .and_then(output_stdout)
        .map(LineArity::from)
        .and_then(LineArity::one)?;

        Ok(GitBinary {
            verbosity,
            name,
            git_dir,
        })
    }

    /// Invoke a git sub-command with an explicit `--git-dir` to make it independent of the working
    /// directory it is invoked from.
    fn command(&self) -> Command {
        let mut command = git_command(self.name);
        command.args(&["--git-dir", &self.git_dir]);
        command
    }

    /// Wraps `git config` to read a single namespaced value.
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        run_trivial(
            self.verbosity,
            format!("Get config {}", key),
            self.command().args(&[
                "config",
                // Use a default to prevent git from returning a non-zero exit code when the value does
                // not exist.
                "--default",
                "",
                "--get",
                &namespace::config_key(key),
            ]),
        )
        .and_then(output_stdout)
        .map(LineArity::from)
        .and_then(LineArity::zero_or_one)
    }

    /// Wraps `git config` to write a single namespaced value.
    #[cfg(test)]
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        run_trivial(
            self.verbosity,
            format!("Set config {} = {}", key, value),
            self.command().args(&[
                "config",
                "--local",
                "--replace-all",
                &namespace::config_key(key),
                value,
            ]),
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
        remote: &Remote,
        refspecs: &[RefSpec],
    ) -> Result<()>
    where
        Description: AsRef<str>,
        RefSpec: AsRef<OsStr>,
    {
        assert!(!refspecs.is_empty());
        run_notable(
            self.verbosity,
            description,
            self.command().args(&["fetch", &remote.0]).args(refspecs),
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
        remote: &Remote,
        refspecs: &[RefSpec],
    ) -> Result<()>
    where
        Description: AsRef<str>,
        RefSpec: AsRef<OsStr>,
    {
        assert!(!refspecs.is_empty());
        run_notable(
            self.verbosity,
            description,
            self.command()
                .args(&["push", "--no-verify", &remote.0])
                .args(refspecs),
        )?;
        Ok(())
    }

    /// Extract a single `GitRef` for a given `ref_name`.
    #[cfg(test)]
    pub fn get_ref<Description, RefName>(
        &self,
        description: Description,
        ref_name: RefName,
    ) -> Result<GitRef>
    where
        Description: AsRef<str>,
        RefName: AsRef<str>,
    {
        run_trivial(
            self.verbosity,
            description,
            self.command()
                .args(&["show-ref", "--verify", ref_name.as_ref()]),
        )
        .and_then(output_stdout)
        .map(LineArity::from)
        .and_then(LineArity::one)
        .and_then(|line| GitRef::parse_show_ref_line(&line).map_err(Into::into))
    }

    /// List all the non-HEAD refs in the repository as `GitRef`s.
    pub fn list_refs<Description>(&self, description: Description) -> Result<Vec<GitRef>>
    where
        Description: AsRef<str>,
    {
        let output = run_trivial(self.verbosity, description, self.command().arg("show-ref"))
            .and_then(output_stdout)?;
        output
            .lines()
            .map(|line| GitRef::parse_show_ref_line(line).map_err(Into::into))
            .collect()
    }

    /// Wraps `git ls-remote` to query a remote for all refs that match the given `refspecs`.
    ///
    /// # Panics
    ///
    /// If `refspecs` is empty, which means git will list all refs, which is never what we want.
    fn list_remote_refs<Description, RefSpec>(
        &self,
        description: Description,
        remote: &Remote,
        refspecs: &[RefSpec],
    ) -> Result<Vec<GitRef>>
    where
        Description: AsRef<str>,
        RefSpec: AsRef<OsStr>,
    {
        assert!(!refspecs.is_empty());
        let output = run_notable(
            self.verbosity,
            description,
            self.command()
                .arg("ls-remote")
                .arg(&remote.0.as_ref())
                .args(refspecs),
        )
        .and_then(output_stdout)?;
        output
            .lines()
            .map(|line| GitRef::parse_ls_remote_line(line).map_err(Into::into))
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
        run_notable(self.verbosity, description, &mut command)?;
        Ok(())
    }

    /// Create a git branch named `branch_name`.
    #[cfg(test)]
    pub fn create_branch(&self, description: impl AsRef<str>, branch_name: &Branch) -> Result<()> {
        let mut command = self.command();
        command.args(&["branch", &branch_name.0]);
        run_notable(self.verbosity, description, &mut command)?;
        Ok(())
    }

    /// Delete a git branch named `branch_name`.
    #[cfg(test)]
    pub fn delete_branch(&self, description: impl AsRef<str>, branch_name: &Branch) -> Result<()> {
        let mut command = self.command();
        command.args(&["branch", "-d", &branch_name.0]);
        run_notable(self.verbosity, description, &mut command)?;
        Ok(())
    }

    /// Should higher level commands be producing output, or has the user requested quiet mode?
    pub fn is_output_allowed(&self) -> bool {
        is_output_allowed(self.verbosity)
    }

    /// Build a point in time snapshot for all refs that nomad cares about from the state in the
    /// local git clone.
    pub fn snapshot<'user>(&self, user: &'user User) -> Result<Snapshot<'user, 'static, GitRef>> {
        let refs = self.list_refs("Fetching all refs")?;

        let mut local_branches = HashSet::<Branch>::new();
        let mut nomad_refs = Vec::<NomadRef<'static, 'static, 'static, GitRef>>::new();

        for r in refs {
            if let Some(name) = r.name.strip_prefix("refs/heads/") {
                local_branches.insert(Branch::from(name.to_string()));
            }

            if let Ok(nomad_ref) = NomadRef::<GitRef>::from_git_local_ref(user, r) {
                nomad_refs.push(nomad_ref);
            }
        }

        Ok(Snapshot::new(user, local_branches, nomad_refs))
    }

    /// Fetch all nomad managed refs from a given remote.
    pub fn fetch_nomad_refs(&self, user: &User, remote: &Remote) -> Result<()> {
        self.fetch_refspecs(
            format!("Fetching branches from {}", remote.0),
            remote,
            &[&namespace::fetch_refspec(user)],
        )
    }

    /// List all nomad managed refs from a given remote.
    ///
    /// Separated from [`Self::fetch_nomad_refs`] because not all callers want to pay the overhead
    /// of actually listing the fetched refs.
    pub fn list_nomad_refs(
        &self,
        user: &User,
        remote: &Remote,
    ) -> Result<impl Iterator<Item = NomadRef<GitRef>>> {
        // In an ideal world, we would be able to get the list of refs fetched directly from `git`.
        //
        // However, `git fetch` is a porcelain command and we don't want to get into parsing its
        // output, so do an entirely separate network fetch with the plumbing `git ls-remote` which
        // we can parse instead.
        let remote_refs = self.list_remote_refs(
            format!("Listing branches at {}", remote.0),
            remote,
            &[&namespace::list_refspec(user)],
        )?;

        Ok(remote_refs
            .into_iter()
            .filter_map(|ref_| NomadRef::<GitRef>::from_git_remote_ref(ref_).ok()))
    }

    /// Push local branches to nomad managed refs in the remote.
    pub fn push_nomad_refs(&self, user: &User, host: &Host, remote: &Remote) -> Result<()> {
        self.push_refspecs(
            format!("Pushing local branches to {}", remote.0),
            remote,
            &[&namespace::push_refspec(user, host)],
        )
    }

    /// Delete the given nomad managed refs.
    pub fn prune_nomad_refs<'user, 'host>(
        &self,
        remote: &Remote,
        prune: impl Iterator<Item = PruneFrom<'user, 'host, GitRef>>,
    ) -> Result<()> {
        let mut refspecs = Vec::<String>::new();
        let mut refs = Vec::<GitRef>::new();

        for prune_from in prune {
            if let PruneFrom::LocalAndRemote(ref nomad_ref) = prune_from {
                refspecs.push(format!(":{}", nomad_ref.to_git_remote_ref()));
            }

            refs.push(
                match prune_from {
                    PruneFrom::LocalOnly(nomad_ref) | PruneFrom::LocalAndRemote(nomad_ref) => {
                        nomad_ref
                    }
                }
                .ref_,
            );
        }

        // Delete from the remote first
        if !refspecs.is_empty() {
            self.push_refspecs(
                format!("Pruning branches at {}", remote.0),
                remote,
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
            self.delete_ref(format!("  Delete {} (was {})", r.name, r.commit_id), &r)?;
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

impl From<String> for LineArity {
    /// Parse a [`LineArity`] from an arbitrary line.
    ///
    /// Coerces the empty line as [`LineArity::Zero`].
    fn from(string: String) -> Self {
        let mut lines = string.lines().take(2).collect::<Vec<_>>();
        let last = lines.pop();

        match last {
            None => LineArity::Zero(),
            Some(last) => {
                if lines.is_empty() {
                    if last.is_empty() {
                        LineArity::Zero()
                    } else {
                        LineArity::One(last.to_owned())
                    }
                } else {
                    LineArity::Many(string)
                }
            }
        }
    }
}

impl LineArity {
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
    use std::fs::create_dir;

    use tempfile::{tempdir, TempDir};

    use crate::{git_testing::VERBOSITY, verbosity::run_notable};

    use super::{git_command, GitBinary};
    use anyhow::Result;

    /// Initializes a git repository in a temporary directory.
    fn git_init() -> Result<(String, TempDir)> {
        let name = "git".to_owned();
        let tmpdir = tempdir()?;

        run_notable(
            VERBOSITY,
            "",
            git_command(&name).current_dir(tmpdir.path()).args(&[
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

        let git = GitBinary::new(VERBOSITY, &name, tmpdir.path())?;
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

        let git = GitBinary::new(VERBOSITY, &name, subdir.as_path())?;
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
        let git = GitBinary::new(VERBOSITY, &name, tmpdir.path())?;

        let got = git.get_config("test.key")?;
        assert_eq!(got, None);

        Ok(())
    }

    /// Verify read-your-writes.
    #[test]
    fn write_then_read_config() -> Result<()> {
        let (name, tmpdir) = git_init()?;
        let git = GitBinary::new(VERBOSITY, &name, tmpdir.path())?;

        git.set_config("key", "testvalue")?;
        let got = git.get_config("key")?;

        assert_eq!(got, Some("testvalue".to_string()));

        Ok(())
    }
}

#[cfg(test)]
mod test_backend {
    use crate::git_testing::{GitCommitId, GitRemote, INITIAL_BRANCH};
    use std::{collections::HashSet, iter::FromIterator};

    use crate::types::NomadRef;

    /// Push should put local branches to remote `refs/nomad/{user}/{host}/{branch}`
    #[test]
    fn push() {
        let origin = GitRemote::init();
        let host0 = origin.clone("user0", "host0");
        host0.push();

        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap()]),
        );
    }

    /// Fetch should pull refs for all hosts that have pushed under the configured user under
    /// `refs/nomad/{host}/{branch}`
    #[test]
    fn fetch() {
        let origin = GitRemote::init();

        let host0 = origin.clone("user0", "host0");
        host0.push();

        let host1 = origin.clone("user0", "host1");

        // Before fetch, the host1 clone should have no nomad refs
        assert_eq!(host1.nomad_refs(), HashSet::new());

        // After fetch, we should see the one host0 branch
        host1.fetch();
        let nomad_refs = host1
            .list()
            .map(Into::into)
            .collect::<HashSet<NomadRef<GitCommitId>>>();
        assert_eq!(
            nomad_refs,
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap()])
        );
    }

    /// Pushing should create nomad refs in the remote.
    /// Fetching should create nomad refs locally.
    /// Pruning should delete refs in the local and remote.
    #[test]
    fn push_fetch_prune() {
        let origin = GitRemote::init();
        let host0 = origin.clone("user0", "host0");

        // In the beginning, there are no nomad refs
        assert_eq!(origin.nomad_refs(), HashSet::new());
        assert_eq!(host0.nomad_refs(), HashSet::new());

        // Pushing creates a remote nomad ref, but local remains empty
        host0.push();
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap()]),
        );
        assert_eq!(host0.nomad_refs(), HashSet::new());

        // Fetching creates a local nomad ref
        host0.fetch();
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap()]),
        );
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap()]),
        );

        // Pruning removes the ref remotely and locally
        host0.prune_local_and_remote([INITIAL_BRANCH]);
        assert_eq!(origin.nomad_refs(), HashSet::new());
        assert_eq!(host0.nomad_refs(), HashSet::new());
    }
}
