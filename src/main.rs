use std::env::current_dir;

use anyhow::{bail, Context, Result};
use clap::{
    crate_authors, crate_description, crate_name, App, AppSettings, Arg, ArgMatches, SubCommand,
};

use crate::{
    backend::{Backend, Config, Remote},
    git_binary::GitBinary,
    progress::{Progress, Run, Verbosity},
};

mod backend;
mod command;
mod git_binary;
mod git_ref;
mod progress;

fn string_value(matches: &ArgMatches, name: &'static str) -> Result<String> {
    matches.value_of(name).context(name).map(String::from)
}

fn main() -> Result<()> {
    let default_user = whoami::username();
    let default_host = whoami::hostname();

    let matches = App::new("git nomad")
        .settings(&[AppSettings::SubcommandRequiredElseHelp])
        .name(crate_name!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("git")
                .long("git")
                .default_value("git")
                .help("Git binary to use"),
        )
        .arg(
            Arg::with_name("silent")
                .short("s")
                .long("silent")
                .help("Silence all output"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Verbose output, repeat up to 3 times for increasing verbosity"),
        )
        .subcommand(
            SubCommand::with_name("init")
                .about("One time initialization for nomad in this repository")
                .arg(
                    Arg::with_name("user")
                        .short("U")
                        .long("user")
                        .default_value(&default_user)
                        .help("User name to sync with (shared by multiple clones)"),
                )
                .arg(
                    Arg::with_name("host")
                        .short("H")
                        .long("host")
                        .default_value(&default_host)
                        .help("Host name to sync with (unique per clone)"),
                ),
        )
        .subcommand(
            SubCommand::with_name("sync")
                .about("Sync local branches to remote")
                .arg(
                    Arg::with_name("remote")
                        .default_value("origin")
                        .help("Git remote to sync against"),
                ),
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

    if let Some(matches) = matches.subcommand_matches("init") {
        let user = string_value(matches, "user")?;
        let host = string_value(matches, "host")?;

        let config = Config { user, host };
        command::init(progress, git, &config)?;
        return Ok(());
    }

    if let Some(matches) = matches.subcommand_matches("sync") {
        return match git.read_config()? {
            None => bail!("No configuration found, try `init` first"),
            Some(config) => {
                let remote = Remote(string_value(matches, "remote")?);
                command::sync(git, &config, &remote)
            }
        };
    }

    Ok(())
}
