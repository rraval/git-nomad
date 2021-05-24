use std::env::current_dir;

use anyhow::{bail, Context, Result};
use clap::{
    crate_authors, crate_description, crate_name, App, AppSettings, Arg, ArgMatches, SubCommand,
};

use crate::{
    backend::{Backend, Config},
    git_binary::GitBinary,
};

mod backend;
mod command;
mod git_binary;

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
        .subcommand(
            SubCommand::with_name("init")
                .about("One time initialization for nomad in this repository")
                .arg(
                    Arg::with_name("remote")
                        .long("remote")
                        .default_value("origin")
                        .help("Git remote to sync against"),
                )
                .arg(
                    Arg::with_name("user")
                        .long("user")
                        .default_value(&default_user)
                        .help("User name to sync with (shared by multiple clones)"),
                )
                .arg(
                    Arg::with_name("host")
                        .long("host")
                        .default_value(&default_host)
                        .help("Host name to sync with (unique per clone)"),
                ),
        )
        .subcommand(SubCommand::with_name("sync").about("Sync local branches to remote"))
        .get_matches();

    let git = GitBinary::new(
        matches.value_of("git").context("git")?,
        current_dir()?.as_path(),
    )?;

    if let Some(matches) = matches.subcommand_matches("init") {
        let remote = string_value(matches, "remote")?;
        let user = string_value(matches, "user")?;
        let host = string_value(matches, "host")?;

        let config = Config { remote, user, host };
        command::init(git, &config)?;
        return Ok(());
    }

    if matches.subcommand_matches("sync").is_some() {
        return match git.read_config()? {
            None => bail!("No configuration found, try `init` first"),
            Some(config) => command::sync(git, &config),
        };
    }

    Ok(())
}
