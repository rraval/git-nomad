use std::{
    borrow::Cow,
    collections::HashSet,
    env::{self, current_dir},
    ffi::OsString,
};

use clap::{
    builder::PossibleValue, crate_authors, crate_description, crate_name, crate_version,
    parser::ValueSource, value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint,
};
use git_version::git_version;
use output::OutputStream;
use types::Branch;
use verbosity::Verbosity;

use crate::{
    git_binary::GitBinary,
    types::{Host, Remote, User},
    workflow::{Filter, LsPrinter, Workflow},
};

mod git_binary;
mod git_ref;
mod output;
mod snapshot;
mod types;
mod verbosity;
mod workflow;

#[cfg(test)]
mod git_testing;

const DEFAULT_REMOTE: Remote<'static> = Remote(Cow::Borrowed("origin"));
const ENV_USER: &str = "GIT_NOMAD_USER";
const ENV_HOST: &str = "GIT_NOMAD_HOST";
const ENV_REMOTE: &str = "GIT_NOMAD_REMOTE";
const CONFIG_USER: &str = "user";
const CONFIG_HOST: &str = "host";

// This value is only conditionally used if `git_version!` cannot find any other version.
const _FALLBACK_VERSION: &str = crate_version!();
const VERSION: &str = git_version!(
    prefix = "git:",
    args = ["--tags", "--always", "--dirty=-modified"],
    fallback = _FALLBACK_VERSION,
);

fn main() -> anyhow::Result<()> {
    let default_user = User::from(whoami::username());
    let default_host = Host::from(whoami::hostname());

    let mut matches =
        cli(default_user, default_host, &mut env::args_os()).unwrap_or_else(|e| e.exit());
    let verbosity = specified_verbosity(&mut matches);

    if verbosity.map_or(false, |v| v.display_version) {
        eprintln!();
        eprintln!("Version: {}", VERSION);
    }

    let git = GitBinary::new(
        verbosity,
        Cow::from(specified_git(&mut matches)),
        current_dir()?.as_path(),
    )?;
    let workflow = specified_workflow(&mut matches, &git)?;

    if verbosity.map_or(false, |v| v.display_workflow) {
        eprintln!();
        eprintln!("Workflow: {:?}", workflow);
    }

    workflow.execute(&git, &mut OutputStream::new_stdout())
}

/// Use [`clap`] to implement the intended command line interface.
fn cli(
    default_user: User,
    default_host: Host,
    args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
) -> clap::error::Result<ArgMatches> {
    Command::new(crate_name!())
        .arg_required_else_help(true)
        .version(VERSION)
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::new("git")
                .global(true)
                .long("git")
                .help("Git binary to use")
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::CommandName)
                .default_value("git"),
        )
        .arg(
            Arg::new("quiet")
                .global(true)
                .short('q')
                .long("quiet")
                .help("Suppress all output")
                .value_parser(value_parser!(bool))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .global(true)
                .short('v')
                .long("verbose")
                .help("Verbose output, repeat up to 2 times for increasing verbosity")
                .value_parser(value_parser!(u8))
                .action(ArgAction::Count),
        )
        .arg(
            Arg::new("user")
                .global(true)
                .short('U')
                .long("user")
                .help("User name, shared by multiple clones, unique per remote")
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::Username)
                .env(ENV_USER)
                .default_value(default_user.0.into_owned()),
        )
        .arg(
            Arg::new("host")
                .global(true)
                .short('H')
                .long("host")
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::Hostname)
                .env(ENV_HOST)
                .default_value(default_host.0.into_owned())
                .help("Host name, unique per clone"),
        )
        .arg(
            Arg::new("remote")
                .global(true)
                .short('R')
                .long("remote")
                .help("Git remote to operate against")
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::Other)
                .env(ENV_REMOTE)
                .default_value(DEFAULT_REMOTE.0.as_ref())
        )
        .subcommand(Command::new("sync").about("Sync local branches to remote"))
        .subcommand(
            Command::new("ls")
                .about("List nomad managed refs")
                .arg(
                    Arg::new("fetch")
                        .short('F')
                        .long("fetch")
                        .help("Fetch refs from remote before listing")
                        .value_parser(value_parser!(bool))
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("print")
                        .long("print")
                        .help("Format for listing nomad managed refs")
                        .value_parser([
                            PossibleValue::new("grouped")
                                .help("Print ref name and commit ID grouped by host"),
                            PossibleValue::new("ref").help("Print only the ref name"),
                            PossibleValue::new("commit").help("Print only the commit ID"),
                        ])
                        .default_value("grouped"),
                )
                .arg(
                    Arg::new("head")
                    .long("head")
                    .help("Only display refs for the current branch")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("branch")
                    .short('b')
                    .long("branch")
                    .help("Only display refs for the named branch (can be specified multiple times)")
                    .value_parser(value_parser!(String))
                    .action(ArgAction::Append)
                )
                .arg(
                    Arg::new("print_self")
                    .long("print-self")
                    .help("Print refs for the current host")
                    .value_parser(value_parser!(bool))
                    .action(ArgAction::SetTrue)
                ),
        )
        .subcommand(
            Command::new("purge")
                .about("Delete nomad refs locally and on the remote")
                .arg(
                    Arg::new("all")
                        .long("all")
                        .help("Delete refs for all hosts")
                        .value_parser(value_parser!(bool))
                        .action(ArgAction::SetTrue),
                ),
        )
        .try_get_matches_from(args)
}

/// The [`Verbosity`] intended by the user via the CLI.
fn specified_verbosity(matches: &mut ArgMatches) -> Option<Verbosity> {
    if matches.remove_one::<bool>("quiet").expect("has default") {
        None
    } else {
        match matches.remove_one::<u8>("verbose").expect("has default") {
            0 => Some(Verbosity::default()),
            1 => Some(Verbosity::verbose()),
            _ => Some(Verbosity::max()),
        }
    }
}

/// The [`GitBinary`] intended by the user via the CLI.
///
/// # Panics
///
/// If [`clap`] does not prevent certain assumed invalid states.
fn specified_git(matches: &mut ArgMatches) -> String {
    matches.remove_one("git").expect("default value")
}

/// The nomad workflow the user intends to execute via the CLI.
///
/// # Panics
///
/// If [`clap`] does not prevent certain assumed invalid states.
fn specified_workflow<'a>(
    matches: &'a mut ArgMatches,
    git: &GitBinary,
) -> anyhow::Result<Workflow<'a>> {
    let user = resolve(matches, "user", || {
        git.get_config(CONFIG_USER).map(|opt| opt.map(User::from))
    })?;

    let host = resolve(matches, "host", || {
        git.get_config(CONFIG_HOST).map(|opt| opt.map(Host::from))
    })?;

    let remote = Remote::from(
        matches
            .remove_one::<String>("remote")
            .expect("default value"),
    );

    let (subcommand, matches) = matches
        .remove_subcommand()
        .expect("subcommand is mandatory");

    return match (subcommand.as_str(), matches) {
        ("sync", _) => Ok(Workflow::Sync { user, host, remote }),

        ("ls", mut matches) => Ok(Workflow::Ls {
            printer: match matches
                .remove_one::<String>("print")
                .expect("has default")
                .as_str()
            {
                "grouped" => LsPrinter::Grouped,
                "ref" => LsPrinter::Ref,
                "commit" => LsPrinter::Commit,
                _ => unreachable!("has possible values"),
            },
            user,
            fetch_remote: if matches.remove_one::<bool>("fetch").expect("has default") {
                Some(remote)
            } else {
                None
            },
            host_filter: if matches
                .remove_one::<bool>("print_self")
                .expect("has default")
            {
                Filter::All
            } else {
                Filter::Deny([host].into())
            },
            branch_filter: {
                let mut branch_set = HashSet::<Branch>::new();

                if matches.remove_one::<bool>("head").expect("has default") {
                    branch_set.insert(git.current_branch()?);
                }

                if let Some(branches) = matches.remove_many::<String>("branch") {
                    branch_set.extend(branches.map(Branch::from));
                }

                if branch_set.is_empty() {
                    Filter::All
                } else {
                    Filter::Allow(branch_set)
                }
            },
        }),

        ("purge", mut matches) => {
            let remote = Remote::from(
                matches
                    .remove_one::<String>("remote")
                    .expect("<remote> is a required argument"),
            );
            let host_filter = if matches.remove_one::<bool>("all").expect("default value") {
                Filter::All
            } else {
                Filter::Allow(HashSet::from_iter([host]))
            };

            return Ok(Workflow::Purge {
                user,
                remote,
                host_filter,
            });
        }

        _ => unreachable!("unknown subcommand"),
    };
}

/// Extract user arguments in order of preference:
///
/// 1. Passed in as direct CLI options
/// 2. Specified as an environment variable
/// 3. Specified in `git config`
/// 4. A default from querying the operating system
fn resolve<T: Clone + From<String>>(
    matches: &mut ArgMatches,
    arg_name: &str,
    from_git_config: impl Fn() -> anyhow::Result<Option<T>>,
) -> anyhow::Result<T> {
    match (
        matches.value_source(arg_name).expect("default value"),
        matches
            .remove_one::<String>(arg_name)
            .expect("default value"),
    ) {
        (ValueSource::CommandLine | ValueSource::EnvVariable, value) => Ok(T::from(value)),
        (_, value) => match from_git_config()? {
            Some(git_value) => Ok(git_value),
            None => Ok(T::from(value)),
        },
    }
}

/// End-to-end workflow tests.
#[cfg(test)]
mod test_e2e {
    use std::{collections::HashSet, iter::FromIterator};

    use crate::{
        git_testing::{GitClone, GitRemote, INITIAL_BRANCH},
        output::OutputStream,
        types::Branch,
        verbosity::Verbosity,
        workflow::{Filter, Workflow},
    };

    fn sync_host(clone: &GitClone) {
        Workflow::Sync {
            user: clone.user.always_borrow(),
            host: clone.host.always_borrow(),
            remote: clone.remote.always_borrow(),
        }
        .execute(&clone.git, &mut OutputStream::new_vec())
        .unwrap();
    }

    /// Syncing should pick up nomad refs from other hosts.
    ///
    /// When the other host deletes their branch (and thus deletes their nomad ref on the remote),
    /// the equivalent local nomad ref for that host should also be deleted.
    ///
    /// See https://github.com/rraval/git-nomad/issues/1
    #[test]
    fn issue_1() {
        let origin = GitRemote::init(None);
        let feature = &Branch::from("feature");

        let host0 = origin.clone("user0", "host0");
        sync_host(&host0);

        let host1 = origin.clone("user0", "host1");
        host1
            .git
            .create_branch("Start feature branch", feature)
            .unwrap();
        sync_host(&host1);

        // both hosts have synced, the origin should have refs from both (including the one for the
        // feature branch on host1)
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref("feature").unwrap(),
            ])
        );

        // host0 hasn't observed host1 yet
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),])
        );

        // sync host0, which should observe host1 refs
        sync_host(&host0);
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref("feature").unwrap(),
            ])
        );

        // host1 deletes the branch and syncs, removing it from origin
        host1
            .git
            .delete_branch("Abandon feature branch", feature)
            .unwrap();
        sync_host(&host1);

        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // host0 syncs and removes the ref for the deleted feature branch
        sync_host(&host0);
        assert_eq!(
            host0.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );
    }

    /// Explicitly pruning other hosts should delete both local and remote nomad refs for that
    /// host.
    ///
    /// See https://github.com/rraval/git-nomad/issues/2
    #[test]
    fn issue_2_other_host() {
        let origin = GitRemote::init(None);

        let host0 = origin.clone("user0", "host0");
        sync_host(&host0);

        let host1 = origin.clone("user0", "host1");
        sync_host(&host1);

        // both hosts have synced, the origin should have both refs
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // pruning refs for host0 from host1
        Workflow::Purge {
            user: host1.user.always_borrow(),
            remote: host1.remote.always_borrow(),
            host_filter: Filter::Allow(HashSet::from_iter([host0.host.always_borrow()])),
        }
        .execute(&host1.git, &mut OutputStream::new_vec())
        .unwrap();

        // the origin should only have refs for host1
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),])
        );
    }

    /// Explicitly pruning everything should delete both local and remote refs for both the current
    /// and other host on the remote.
    ///
    /// See https://github.com/rraval/git-nomad/issues/2
    #[test]
    fn issue_2_all() {
        let origin = GitRemote::init(Some(Verbosity::max()));

        let host0 = origin.clone("user0", "host0");
        sync_host(&host0);

        let host1 = origin.clone("user0", "host1");
        sync_host(&host1);

        // both hosts have synced, the origin should have both refs
        assert_eq!(
            origin.nomad_refs(),
            HashSet::from_iter([
                host0.get_nomad_ref(INITIAL_BRANCH).unwrap(),
                host1.get_nomad_ref(INITIAL_BRANCH).unwrap(),
            ])
        );

        // pruning refs for all hosts from host1
        Workflow::Purge {
            user: host1.user.always_borrow(),
            remote: host1.remote,
            host_filter: Filter::All,
        }
        .execute(&host1.git, &mut OutputStream::new_vec())
        .unwrap();

        // the origin should have no refs
        assert_eq!(origin.nomad_refs(), HashSet::new(),);
    }
}

/// CLI invocation tests
#[cfg(test)]
mod test_cli {
    use std::{collections::HashSet, iter::FromIterator};

    use clap::{error::ErrorKind, ArgMatches};

    use crate::{
        cli,
        git_testing::GitRemote,
        specified_git, specified_verbosity, specified_workflow,
        types::{Branch, Host, Remote, User},
        verbosity::Verbosity,
        workflow::{Filter, LsPrinter, Workflow},
        CONFIG_HOST, CONFIG_USER, DEFAULT_REMOTE,
    };

    struct CliTest {
        default_user: User<'static>,
        default_host: Host<'static>,
    }

    impl CliTest {
        fn default_host_filter(&self) -> Filter<Host> {
            Filter::Deny([self.default_host.always_borrow()].into())
        }

        fn matches(&self, args: &[&str]) -> clap::error::Result<ArgMatches> {
            let mut vec = vec!["git-nomad"];
            vec.extend_from_slice(args);
            cli(self.default_user.clone(), self.default_host.clone(), &vec)
        }

        fn remote(&self, args: &[&str]) -> CliTestRemote {
            CliTestRemote {
                matches: self.matches(args).unwrap(),
                remote: GitRemote::init(Some(Verbosity::max())),
            }
        }
    }

    struct CliTestRemote {
        matches: ArgMatches,
        remote: GitRemote,
    }

    impl CliTestRemote {
        fn set_config(&mut self, key: &str, value: &str) -> &mut Self {
            self.remote.git.set_config(key, value).unwrap();
            self
        }

        fn workflow(&mut self) -> Workflow<'_> {
            specified_workflow(&mut self.matches, &self.remote.git).unwrap()
        }
    }

    impl Default for CliTest {
        fn default() -> Self {
            Self {
                default_user: User::from("default_user"),
                default_host: Host::from("default_host"),
            }
        }
    }

    /// Should print help and stop processing if no subcommand is specified.
    #[test]
    fn subcommand_is_required() {
        let cli_test = CliTest::default();
        let matches = cli_test.matches(&[]);
        assert!(matches.is_err());
        assert_eq!(
            match matches {
                Ok(_) => unreachable!(),
                Err(e) => e.kind(),
            },
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        );
    }

    /// `--git` before/after the subcommand.
    #[test]
    fn git_option() {
        for args in &[&["--git", "foo", "ls"], &["ls", "--git", "foo"]] {
            println!("{:?}", args);
            let cli_test = CliTest::default();
            let mut matches = cli_test.matches(*args).unwrap();
            assert_eq!(specified_git(&mut matches), "foo");
        }
    }

    #[test]
    fn quiet_verbosity() {
        for args in &[
            &["--quiet", "ls"],
            &["-q", "ls"],
            &["ls", "--quiet"],
            &["ls", "-q"],
        ] {
            println!("{:?}", args);
            let cli_test = CliTest::default();
            let mut matches = cli_test.matches(*args).unwrap();
            assert_eq!(specified_verbosity(&mut matches), None);
        }
    }

    #[test]
    fn default_verbosity() {
        let cli_test = CliTest::default();
        let mut matches = cli_test.matches(&["ls"]).unwrap();
        assert_eq!(
            specified_verbosity(&mut matches),
            Some(Verbosity::default())
        );
    }

    #[test]
    fn verbose_verbosity() {
        for args in &[
            &["--verbose", "ls"],
            &["-v", "ls"],
            &["ls", "--verbose"],
            &["ls", "-v"],
        ] {
            println!("{:?}", args);
            let cli_test = CliTest::default();
            let mut matches = cli_test.matches(*args).unwrap();
            assert_eq!(
                specified_verbosity(&mut matches),
                Some(Verbosity::verbose())
            );
        }
    }

    #[test]
    fn max_verbosity() {
        for args in &[
            &["--verbose", "--verbose", "ls"] as &[&str],
            &["ls", "-vv"],
            &["ls", "-v", "--verbose"],
            &["ls", "-vv", "-vv"],
        ] {
            println!("{:?}", args);
            let cli_test = CliTest::default();
            let mut matches = cli_test.matches(*args).unwrap();
            assert_eq!(specified_verbosity(&mut matches), Some(Verbosity::max()));
        }
    }

    #[test]
    fn ls() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["ls"]).workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: None,
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::All,
            },
        );
    }

    #[test]
    fn ls_fetch_remote_default() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["ls", "--fetch"]).workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: Some(DEFAULT_REMOTE),
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::All,
            },
        );
    }

    #[test]
    fn ls_fetch_remote_global() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&["--remote", "foo", "ls", "--fetch"])
                .workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: Some(Remote::from("foo")),
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::All,
            },
        );
    }

    #[test]
    fn ls_fetch_remote_local() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&["ls", "--fetch", "--remote", "foo"])
                .workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: Some(Remote::from("foo")),
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::All,
            },
        );
    }

    #[test]
    fn ls_print_grouped() {
        for args in &[
            &["ls", "--print", "grouped"] as &[&str],
            &["ls", "--print=grouped"],
        ] {
            println!("{:?}", args);

            let cli_test = CliTest::default();
            assert_eq!(
                cli_test.remote(args).workflow(),
                Workflow::Ls {
                    printer: LsPrinter::Grouped,
                    user: cli_test.default_user.always_borrow(),
                    fetch_remote: None,
                    host_filter: cli_test.default_host_filter(),
                    branch_filter: Filter::All,
                },
            );
        }
    }

    #[test]
    fn ls_print_ref() {
        for args in &[&["ls", "--print", "ref"] as &[&str], &["ls", "--print=ref"]] {
            println!("{:?}", args);

            let cli_test = CliTest::default();
            assert_eq!(
                cli_test.remote(args).workflow(),
                Workflow::Ls {
                    printer: LsPrinter::Ref,
                    user: cli_test.default_user.always_borrow(),
                    fetch_remote: None,
                    host_filter: cli_test.default_host_filter(),
                    branch_filter: Filter::All,
                },
            );
        }
    }

    #[test]
    fn ls_print_commit() {
        for args in &[
            &["ls", "--print", "commit"] as &[&str],
            &["ls", "--print=commit"],
        ] {
            println!("{:?}", args);

            let cli_test = CliTest::default();
            assert_eq!(
                cli_test.remote(args).workflow(),
                Workflow::Ls {
                    printer: LsPrinter::Commit,
                    user: cli_test.default_user.always_borrow(),
                    fetch_remote: None,
                    host_filter: cli_test.default_host_filter(),
                    branch_filter: Filter::All,
                },
            );
        }
    }

    #[test]
    fn ls_explicit() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["ls", "-U", "explicit_user"]).workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: User::from("explicit_user"),
                fetch_remote: None,
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::All,
            },
        );
    }

    #[test]
    fn ls_config_beats_default() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&["ls"])
                .set_config(CONFIG_USER, "config_user")
                .workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: User::from("config_user"),
                fetch_remote: None,
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::All,
            },
        );
    }

    #[test]
    fn ls_head() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["ls", "--head"]).workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: None,
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::Allow(["master"].map(Branch::from).into()),
            },
        );
    }

    #[test]
    fn ls_branches() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&["ls", "-b", "foo", "--branch", "bar", "--branch=baz"])
                .workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: None,
                host_filter: cli_test.default_host_filter(),
                branch_filter: Filter::Allow(["foo", "bar", "baz"].map(Branch::from).into()),
            },
        );
    }

    #[test]
    fn ls_print_self() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["ls", "--print-self"]).workflow(),
            Workflow::Ls {
                printer: LsPrinter::Grouped,
                user: cli_test.default_user.always_borrow(),
                fetch_remote: None,
                host_filter: Filter::All,
                branch_filter: Filter::All,
            },
        );
    }

    /// Invoke `sync` with explicit `user` and `host`
    #[test]
    fn sync_explicit() {
        for args in &[
            &[
                "--user", "user0", "sync", "--host", "host0", "--remote", "remote",
            ] as &[&str],
            &["sync", "-U", "user0", "-H", "host0", "-R", "remote"],
        ] {
            println!("{:?}", args);
            let cli_test = CliTest::default();
            assert_eq!(
                cli_test.remote(*args).workflow(),
                Workflow::Sync {
                    user: User::from("user0"),
                    host: Host::from("host0"),
                    remote: Remote::from("remote"),
                },
            );
        }
    }

    /// Invoke `sync` with `user` and `host` coming from `git config`.
    #[test]
    fn sync_config() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&["sync"])
                .set_config(CONFIG_USER, "user0")
                .set_config(CONFIG_HOST, "host0")
                .workflow(),
            Workflow::Sync {
                user: User::from("user0"),
                host: Host::from("host0"),
                remote: DEFAULT_REMOTE.clone(),
            }
        );
    }

    /// Invoke `sync` with defaults.
    #[test]
    fn sync_default() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["sync"]).workflow(),
            Workflow::Sync {
                user: cli_test.default_user.always_borrow(),
                host: cli_test.default_host.always_borrow(),
                remote: DEFAULT_REMOTE.clone(),
            }
        );
    }

    #[test]
    fn purge_all() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["purge", "--all"]).workflow(),
            Workflow::Purge {
                user: cli_test.default_user.always_borrow(),
                remote: DEFAULT_REMOTE.clone(),
                host_filter: Filter::All,
            }
        );
    }

    #[test]
    fn purge_hosts() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&["--host=host0", "purge", "-R", "remote"])
                .workflow(),
            Workflow::Purge {
                user: cli_test.default_user.always_borrow(),
                remote: Remote::from("remote"),
                host_filter: Filter::Allow(HashSet::from_iter(["host0"].map(Host::from))),
            }
        );
    }
}
