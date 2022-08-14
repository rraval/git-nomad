use std::{
    borrow::Cow,
    env::{self, current_dir},
    ffi::OsString,
};

use clap::{
    crate_authors, crate_description, crate_name, crate_version, value_parser, Arg, ArgAction,
    ArgGroup, ArgMatches, Command, ValueHint, ValueSource,
};
use git_version::git_version;
use verbosity::Verbosity;

use crate::{
    git_binary::GitBinary,
    types::{Host, Remote, User},
    workflow::{PurgeFilter, Workflow},
};

mod git_binary;
mod git_ref;
mod snapshot;
mod types;
mod verbosity;
mod workflow;

#[cfg(test)]
mod git_testing;

const DEFAULT_REMOTE: Remote<'static> = Remote(Cow::Borrowed("origin"));
const ENV_USER: &str = "GIT_NOMAD_USER";
const ENV_HOST: &str = "GIT_NOMAD_HOST";
const CONFIG_USER: &str = "user";
const CONFIG_HOST: &str = "host";

fn main() -> anyhow::Result<()> {
    let default_user = User::from(whoami::username());
    let default_host = Host::from(whoami::hostname());

    let mut matches =
        cli(&default_user, &default_host, &mut env::args_os()).unwrap_or_else(|e| e.exit());
    let verbosity = specified_verbosity(&mut matches);
    let git = GitBinary::new(
        verbosity,
        Cow::from(specified_git(&mut matches)),
        current_dir()?.as_path(),
    )?;
    let workflow = specified_workflow(&mut matches, &git)?;

    if let Some(verbosity) = verbosity {
        if verbosity.display_workflow {
            eprintln!();
            eprintln!("Workflow: {:?}", workflow);
        }
    }

    workflow.execute(&git)
}

/// Use [`clap`] to implement the intended command line interface.
fn cli(
    default_user: &User,
    default_host: &Host,
    args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
) -> clap::Result<ArgMatches> {
    let remote_arg = || {
        Arg::new("remote")
            .help("Git remote to sync against")
            .takes_value(true)
            .value_parser(value_parser!(String))
            .value_hint(ValueHint::Other)
            .default_value(&DEFAULT_REMOTE.0)
    };

    let host_arg = || {
        Arg::new("host")
            .short('H')
            .long("host")
            .takes_value(true)
            .value_parser(value_parser!(String))
            .value_hint(ValueHint::Hostname)
    };

    // This value is only conditionally used if `git_version!` cannot find any other version.
    let _fallback_version = crate_version!();

    Command::new(crate_name!())
        .arg_required_else_help(true)
        .version(git_version!(
            prefix = "git:",
            args = ["--tags", "--always", "--dirty=-modified"],
            fallback = _fallback_version,
        ))
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::new("git")
                .global(true)
                .long("git")
                .help("Git binary to use")
                .takes_value(true)
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::CommandName)
                .default_value("git"),
        )
        .arg(
            Arg::new("silent")
                .global(true)
                .short('s')
                .long("silent")
                .help("Silence all output")
                .takes_value(true)
                .value_parser(value_parser!(bool))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .global(true)
                .short('v')
                .long("verbose")
                .help("Verbose output, repeat up to 2 times for increasing verbosity")
                .takes_value(true)
                .value_parser(value_parser!(u8))
                .action(ArgAction::Count),
        )
        .arg(
            Arg::new("user")
                .global(true)
                .short('U')
                .long("user")
                .help("User name, shared by multiple clones, unique per remote")
                .next_line_help(true)
                .takes_value(true)
                .value_parser(value_parser!(String))
                .value_hint(ValueHint::Username)
                .env(ENV_USER)
                .default_value(&default_user.0),
        )
        .subcommand(
            Command::new("sync")
                .about("Sync local branches to remote")
                .arg(
                    host_arg()
                        .env(ENV_HOST)
                        .default_value(&default_host.0)
                        .help("Host name to sync with, unique per clone")
                        .next_line_help(true),
                )
                .arg(remote_arg()),
        )
        .subcommand(Command::new("ls").about("List refs for all hosts"))
        .subcommand(
            Command::new("purge")
                .about("Delete nomad refs locally and on the remote")
                .arg(
                    Arg::new("all")
                        .long("all")
                        .help("Delete refs for all hosts")
                        .takes_value(true)
                        .value_parser(value_parser!(bool))
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    host_arg()
                        .takes_value(true)
                        .value_parser(value_parser!(String))
                        .action(ArgAction::Append)
                        .help(
                            "Delete refs for only the given host (can be specified multiple times)",
                        ),
                )
                .group(
                    ArgGroup::new("host_group")
                        .args(&["all", "host"])
                        .required(true),
                )
                .arg(remote_arg()),
        )
        .try_get_matches_from(args)
}

/// The [`Verbosity`] intended by the user via the CLI.
fn specified_verbosity(matches: &mut ArgMatches) -> Option<Verbosity> {
    if matches.remove_one::<bool>("silent").expect("has default") {
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
fn specified_workflow<'a, 'user: 'a, 'host: 'a>(
    matches: &'a mut ArgMatches,
    git: &GitBinary,
) -> anyhow::Result<Workflow<'a, 'a, 'a>> {
    let user = resolve(matches, "user", || {
        git.get_config(CONFIG_USER).map(|opt| opt.map(User::from))
    })?;

    let (subcommand, matches) = matches
        .remove_subcommand()
        .expect("subcommand is mandatory");

    return match (subcommand.as_str(), matches) {
        ("sync", mut matches) => {
            let host = resolve(&mut matches, "host", || {
                git.get_config(CONFIG_HOST).map(|opt| opt.map(Host::from))
            })?;
            let remote = Remote::from(
                matches
                    .remove_one::<String>("remote")
                    .expect("<remote> is a required argument"),
            );

            Ok(Workflow::Sync { user, host, remote })
        }

        ("ls", _) => Ok(Workflow::Ls { user }),

        ("purge", mut matches) => {
            let remote = Remote::from(
                matches
                    .remove_one::<String>("remote")
                    .expect("<remote> is a required argument"),
            );
            let purge_filter = if matches.remove_one::<bool>("all").expect("default value") {
                PurgeFilter::All
            } else {
                PurgeFilter::Hosts(
                    matches
                        .remove_many::<String>("host")
                        .unwrap_or_default()
                        .map(Host::from)
                        .collect(),
                )
            };

            return Ok(Workflow::Purge {
                user,
                remote,
                purge_filter,
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
        types::Branch,
        workflow::{PurgeFilter, Workflow},
    };

    fn sync_host(clone: &GitClone) {
        Workflow::Sync {
            user: clone.user.always_borrow(),
            host: clone.host.always_borrow(),
            remote: clone.remote(),
        }
        .execute(&clone.git)
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
        let origin = GitRemote::init();
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
        let origin = GitRemote::init();

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
            remote: host1.remote(),
            purge_filter: PurgeFilter::Hosts(HashSet::from_iter([host0.host.always_borrow()])),
        }
        .execute(&host1.git)
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
        let origin = GitRemote::init();

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
            remote: host1.remote(),
            purge_filter: PurgeFilter::All,
        }
        .execute(&host1.git)
        .unwrap();

        // the origin should have no refs
        assert_eq!(origin.nomad_refs(), HashSet::new(),);
    }
}

/// CLI invocation tests
#[cfg(test)]
mod test_cli {
    use std::{collections::HashSet, iter::FromIterator};

    use clap::{ArgMatches, ErrorKind};

    use crate::{
        cli,
        git_testing::GitRemote,
        specified_git, specified_verbosity, specified_workflow,
        types::{Host, Remote, User},
        verbosity::Verbosity,
        workflow::{PurgeFilter, Workflow},
        CONFIG_HOST, CONFIG_USER, DEFAULT_REMOTE,
    };

    struct CliTest {
        default_user: User<'static>,
        default_host: Host<'static>,
    }

    impl CliTest {
        fn matches<'a>(&'a self, args: &[&str]) -> clap::Result<ArgMatches> {
            let mut vec = vec!["git-nomad"];
            vec.extend_from_slice(args);
            cli(&self.default_user, &self.default_host, &vec)
        }

        fn remote(&self, args: &[&str]) -> CliTestRemote {
            CliTestRemote {
                matches: self.matches(args).unwrap(),
                remote: GitRemote::init(),
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

        fn workflow(&mut self) -> Workflow<'_, '_, '_> {
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
    fn silent_verbosity() {
        for args in &[
            &["--silent", "ls"],
            &["-s", "ls"],
            &["ls", "--silent"],
            &["ls", "-s"],
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
                user: cli_test.default_user.always_borrow(),
            },
        );
    }

    #[test]
    fn ls_explicit() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test.remote(&["ls", "-U", "explicit_user"]).workflow(),
            Workflow::Ls {
                user: User::from("explicit_user"),
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
                user: User::from("config_user"),
            },
        );
    }

    /// Invoke `sync` with explicit `user` and `host`
    #[test]
    fn sync_explicit() {
        for args in &[
            &["--user", "user0", "sync", "--host", "host0", "remote"] as &[&str],
            &["sync", "-U", "user0", "-H", "host0", "remote"],
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
                purge_filter: PurgeFilter::All,
            }
        );
    }

    #[test]
    fn purge_hosts() {
        let cli_test = CliTest::default();
        assert_eq!(
            cli_test
                .remote(&[
                    "purge",
                    "--host=host0",
                    "--host",
                    "host1",
                    "-H",
                    "host2",
                    "remote"
                ])
                .workflow(),
            Workflow::Purge {
                user: cli_test.default_user.always_borrow(),
                remote: Remote::from("remote"),
                purge_filter: PurgeFilter::Hosts(HashSet::from_iter(
                    ["host0", "host1", "host2"].map(Host::from)
                )),
            }
        );
    }
}
