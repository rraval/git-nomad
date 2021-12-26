use std::{
    borrow::Cow,
    env::{self, current_dir},
    ffi::OsString,
};

use clap::{
    crate_authors, crate_description, crate_name, App, AppSettings, Arg, ArgGroup, ArgMatches,
    SubCommand,
};
// `crate_version!` is only used as a version fallback and thus macro expansion may make the only
// usage disappear.
#[allow(unused_imports)]
use clap::crate_version;
use git_version::git_version;
use verbosity::{CommandVerbosity, SignificanceVerbosity, Verbosity};

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

fn main() -> anyhow::Result<()> {
    let default_user = User::from(whoami::username());
    let default_host = Host::from(whoami::hostname());

    let matches =
        cli(&default_user, &default_host, &mut env::args_os()).unwrap_or_else(|e| e.exit());
    let verbosity = specified_verbosity(&matches);
    let git = GitBinary::new(verbosity, specified_git(&matches), current_dir()?.as_path())?;
    let workflow = specified_workflow(&matches, &default_user, &default_host, &git)?;

    if let Some(verbosity) = verbosity {
        if verbosity.display_workflow {
            eprintln!();
            eprintln!("Workflow: {:?}", workflow);
        }
    }

    workflow.execute(&git)?;

    Ok(())
}

/// Use [`clap`] to implement the intended command line interface.
fn cli<'a>(
    default_user: &'a User<'a>,
    default_host: &'a Host<'a>,
    args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
) -> clap::Result<ArgMatches<'a>> {
    let remote_arg = || {
        Arg::with_name("remote")
            .default_value(&DEFAULT_REMOTE.0)
            .help("Git remote to sync against")
    };

    let host_arg = || {
        Arg::with_name("host")
            .short("H")
            .long("host")
            .takes_value(true)
    };

    App::new("git nomad")
        .settings(&[
            AppSettings::SubcommandRequiredElseHelp,
            AppSettings::VersionlessSubcommands,
        ])
        .name(crate_name!())
        .version(git_version!(
            prefix = "git:",
            args = ["--tags", "--always", "--dirty=-modified"],
            fallback = crate_version!(),
        ))
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("git")
                .global(true)
                .long("git")
                .default_value("git")
                .help("Git binary to use"),
        )
        .arg(
            Arg::with_name("silent")
                .global(true)
                .short("s")
                .long("silent")
                .help("Silence all output"),
        )
        .arg(
            Arg::with_name("verbose")
                .global(true)
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Verbose output, repeat up to 3 times for increasing verbosity"),
        )
        .arg(
            Arg::with_name("user")
                .global(true)
                .short("U")
                .long("user")
                .env("GIT_NOMAD_USER")
                .default_value(&default_user.0)
                .next_line_help(true)
                .help("User name, shared by multiple clones, unique per remote"),
        )
        .subcommand(
            SubCommand::with_name("sync")
                .about("Sync local branches to remote")
                .arg(
                    host_arg()
                        .env("GIT_NOMAD_HOST")
                        .next_line_help(true)
                        .default_value(&default_host.0)
                        .help("Host name to sync with, unique per clone"),
                )
                .arg(remote_arg()),
        )
        .subcommand(SubCommand::with_name("ls").about("List refs for all hosts"))
        .subcommand(
            SubCommand::with_name("purge")
                .about("Delete nomad refs locally and on the remote")
                .arg(
                    Arg::with_name("all")
                        .long("all")
                        .help("Delete refs for all hosts"),
                )
                .arg(
                    host_arg().multiple(true).help(
                        "Delete refs for only the given host (can be specified multiple times)",
                    ),
                )
                .group(
                    ArgGroup::with_name("host_group")
                        .args(&["all", "host"])
                        .required(true),
                )
                .arg(remote_arg()),
        )
        .get_matches_from_safe(args)
}

/// The [`Verbosity`] intended by the user via the CLI.
fn specified_verbosity(matches: &ArgMatches) -> Option<Verbosity> {
    if matches.is_present("silent") {
        None
    } else {
        match matches.occurrences_of("verbose") {
            0 => Some(Verbosity {
                display_workflow: false,
                significance: SignificanceVerbosity::OnlyNotable,
                command: CommandVerbosity::Spinner,
            }),
            1 => Some(Verbosity {
                display_workflow: true,
                significance: SignificanceVerbosity::All,
                command: CommandVerbosity::Invocation,
            }),
            _ => Some(Verbosity::max()),
        }
    }
}

/// The [`GitBinary`] intended by the user via the CLI.
///
/// # Panics
///
/// If [`clap`] does not prevent certain assumed invalid states.
fn specified_git<'a>(matches: &'a ArgMatches) -> &'a str {
    matches
        .value_of("git")
        .expect("There should be a default value")
}

/// The nomad workflow the user intends to execute via the CLI.
///
/// # Panics
///
/// If [`clap`] does not prevent certain assumed invalid states.
fn specified_workflow<'a, 'user: 'a, 'host: 'a>(
    matches: &'a ArgMatches,
    default_user: &'user User<'user>,
    default_host: &'host Host<'host>,
    git: &GitBinary,
) -> anyhow::Result<Workflow<'a, 'a, 'a>> {
    let user = resolve(
        matches,
        "user",
        git.get_config("user")?.map(User::from),
        default_user,
    );

    if let Some(matches) = matches.subcommand_matches("sync") {
        let host = resolve(
            matches,
            "host",
            git.get_config("host")?.map(Host::from),
            default_host,
        );
        let remote = Remote::from(
            matches
                .value_of("remote")
                .expect("<remote> is a required argument"),
        );
        return Ok(Workflow::Sync { user, host, remote });
    }

    if matches.subcommand_matches("ls").is_some() {
        return Ok(Workflow::Ls { user });
    }

    if let Some(matches) = matches.subcommand_matches("purge") {
        let remote = Remote::from(
            matches
                .value_of("remote")
                .expect("<remote> is a required argument"),
        );
        let purge_filter = if matches.is_present("all") {
            PurgeFilter::All
        } else if let Some(hosts) = matches.values_of("host") {
            PurgeFilter::Hosts(hosts.map(Host::from).collect())
        } else {
            panic!("ArgGroup should have verified that one of these parameters was present");
        };

        return Ok(Workflow::Purge {
            user,
            remote,
            purge_filter,
        });
    }

    panic!("Subcommand is mandatory");
}

/// Extract user arguments in order of preference:
///
/// 1. Passed in as direct CLI options
/// 2. Specified as an environment variable
/// 3. Specified in `git config`
/// 4. A default from querying the operating system
///
/// [`clap`] supports (1), (2), and (4), but because we need to insert (3), we cannot simply rely
/// on [`ArgMatches::value_of`].
///
/// Instead, we rely on [`ArgMatches::is_present`], which will be true for (1) and (2) and thus
/// [`ArgMatches::value_of`] will do as we want.
///
/// Otherwise, we roll our own logic for the (3) and (4) cases.
fn resolve<'a, T: Clone + From<&'a str>>(
    matches: &'a ArgMatches,
    arg_name: &str,
    from_git_config: Option<T>,
    from_os_default: &'a T,
) -> Cow<'a, T> {
    if matches.is_present(arg_name) {
        Cow::Owned(T::from(
            matches
                .value_of(arg_name)
                .expect("is_present claimed there was a value"),
        ))
    } else if let Some(value) = from_git_config {
        Cow::Owned(value)
    } else {
        Cow::Borrowed(from_os_default)
    }
}

/// End-to-end workflow tests.
#[cfg(test)]
mod test_e2e {
    use std::{borrow::Cow, collections::HashSet, iter::FromIterator};

    use crate::{
        git_testing::{GitClone, GitRemote, INITIAL_BRANCH},
        types::Branch,
        workflow::{PurgeFilter, Workflow},
    };

    fn sync_host(clone: &GitClone) {
        Workflow::Sync {
            user: Cow::Borrowed(&clone.user),
            host: Cow::Borrowed(&clone.host),
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
            user: Cow::Borrowed(&host1.user),
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
            user: Cow::Borrowed(&host1.user),
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
    use std::borrow::Cow;

    use clap::{ArgMatches, ErrorKind};

    use crate::{
        cli,
        git_testing::GitRemote,
        specified_git, specified_workflow,
        types::{Host, User},
        workflow::Workflow,
        DEFAULT_REMOTE,
    };

    struct CliTest {
        default_user: User<'static>,
        default_host: Host<'static>,
    }

    impl CliTest {
        fn matches<'a>(&'a self, args: &[&str]) -> clap::Result<ArgMatches<'a>> {
            let mut vec = vec!["git-nomad"];
            vec.extend_from_slice(args);
            cli(&self.default_user, &self.default_host, &vec)
        }

        fn workflow<'a>(&'a self, matches: &'a ArgMatches<'a>) -> Workflow<'a, 'a, 'a> {
            let remote = GitRemote::init();
            specified_workflow(matches, &self.default_user, &self.default_host, &remote.git)
                .unwrap()
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
                Err(e) => e.kind,
            },
            ErrorKind::MissingArgumentOrSubcommand,
        );
    }

    /// `--git` before/after the subcommand.
    #[test]
    fn git_option() {
        for args in &[&["--git", "foo", "ls"], &["ls", "--git", "foo"]] {
            let cli_test = CliTest::default();
            let matches = cli_test.matches(*args).unwrap();
            assert_eq!(specified_git(&matches), "foo");
        }
    }

    #[test]
    fn sync() {
        let cli_test = CliTest::default();
        let matches = cli_test.matches(&["sync"]).unwrap();

        assert_eq!(
            cli_test.workflow(&matches),
            Workflow::Sync {
                user: Cow::Borrowed(&cli_test.default_user),
                host: Cow::Borrowed(&cli_test.default_host),
                remote: DEFAULT_REMOTE.clone(),
            }
        );
    }
}
