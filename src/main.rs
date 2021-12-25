use std::{collections::HashSet, env::current_dir};

use anyhow::{bail, Context, Result};
use clap::{
    crate_authors, crate_description, crate_name, App, AppSettings, Arg, ArgMatches, SubCommand,
};
// `crate_version!` is only used as a version fallback and thus macro expansion may make the only
// usage disappear.
#[allow(unused_imports)]
use clap::crate_version;
use git_version::git_version;

use crate::{
    git_binary::GitBinary,
    progress::{Progress, Run, Verbosity},
    types::{Host, Remote, User},
};

mod command;
mod git_binary;
mod git_ref;
mod progress;
mod snapshot;
mod types;

#[cfg(test)]
mod git_testing;

fn str_value<'a>(matches: &'a ArgMatches, name: &'static str) -> Result<&'a str> {
    matches.value_of(name).context(name)
}

fn main() -> Result<()> {
    let default_user = User::from(whoami::username());
    let default_host = Host::from(whoami::hostname());

    let remote_arg = || {
        Arg::with_name("remote")
            .default_value("origin")
            .help("Git remote to sync against")
    };

    let host_arg = || {
        Arg::with_name("host")
            .short("H")
            .long("host")
            .takes_value(true)
    };

    let matches =
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
                    .arg(host_arg().multiple(true).help(
                        "Delete refs for only the given host (can be specified multiple times)",
                    ))
                    .arg(remote_arg()),
            )
            .get_matches();

    let progress = &{
        if matches.is_present("silent") {
            Progress::Silent
        } else {
            match matches.occurrences_of("verbose") {
                0 => Progress::Standard {
                    significance_at_least: Run::Notable,
                },
                1 => Progress::Standard {
                    significance_at_least: Run::Trivial,
                },
                2 => Progress::Verbose(Verbosity::CommandOnly),
                _ => Progress::Verbose(Verbosity::CommandAndOutput),
            }
        }
    };

    let git = GitBinary::new(
        progress,
        matches.value_of("git").context("git")?,
        current_dir()?.as_path(),
    )?;

    let user = resolve(
        &matches,
        "user",
        git.get_config("user")?.map(User::from),
        default_user.always_borrow(),
    );

    if let Some(matches) = matches.subcommand_matches("sync") {
        let host = resolve(
            matches,
            "host",
            git.get_config("host")?.map(Host::from),
            default_host.always_borrow(),
        );

        let remote = Remote::from(str_value(matches, "remote")?);
        command::sync(&git, &user, &host, &remote)?
    }

    if matches.subcommand_matches("ls").is_some() {
        command::ls(&git, &user)?
    }

    if let Some(matches) = matches.subcommand_matches("purge") {
        let remote = Remote::from(str_value(matches, "remote")?);
        if matches.is_present("all") {
            command::purge(&git, &user, &remote, |snapshot| snapshot.prune_all())?
        } else if let Some(hosts) = matches.values_of("host") {
            let set = hosts.map(Host::from).collect::<HashSet<_>>();
            command::purge(&git, &user, &remote, |snapshot| {
                snapshot.prune_all_by_hosts(&set)
            })?
        } else {
            bail!("Must specify --all or --host");
        }
    }

    Ok(())
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
fn resolve<'a, T: From<&'a str>>(
    matches: &'a ArgMatches,
    arg_name: &str,
    from_git_config: Option<T>,
    from_os_default: T,
) -> T {
    if matches.is_present(arg_name) {
        T::from(
            matches
                .value_of(arg_name)
                .expect("is_present claimed there was a value"),
        )
    } else if let Some(value) = from_git_config {
        value
    } else {
        from_os_default
    }
}
