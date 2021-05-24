use anyhow::{bail, Context, Result};
use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};

use crate::backend::{Backend, Config, Remote};

pub struct GitBinary<'a> {
    name: &'a OsStr,
    git_dir: String,
}

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
        check_run(self.command().args(&[
            "config",
            "--local",
            "--replace-all",
            key,
            value,
        ]))?;
        Ok(())
    }

    fn fetch(&self, remote: &str, refspec: &str) -> Result<()> {
        check_run(self.command().args(&["fetch", remote, refspec]))?;
        Ok(())
    }
}

impl<'a> Backend for GitBinary<'a> {
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

    fn fetch_remote_refs(&self, config: &Config, remote: &Remote) -> Result<()> {
        self.fetch(&remote.0, &namespace::fetch_refspec(config))
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
mod tests {
    use std::{fs::create_dir, process::Command};

    use tempfile::{tempdir, TempDir};

    use super::{check_output, GitBinary};
    use anyhow::Result;

    impl<'a> GitBinary<'a> {
        fn init() -> Result<(String, TempDir)> {
            let name = "git".to_owned();
            let tmpdir = tempdir()?;

            check_output(
                Command::new(&name)
                    .current_dir(tmpdir.path())
                    .args(&["init"]),
            )?;

            Ok((name, tmpdir))
        }
    }

    #[test]
    fn toplevel_at_root() -> Result<()> {
        let (name, tmpdir) = GitBinary::init()?;

        let git = GitBinary::new(&name, tmpdir.path())?;
        assert_eq!(
            Some(git.git_dir.as_str()),
            tmpdir.path().join(".git").to_str()
        );

        Ok(())
    }

    #[test]
    fn toplevel_in_subdir() -> Result<()> {
        let (name, tmpdir) = GitBinary::init()?;
        let subdir = tmpdir.path().join("subdir");
        create_dir(&subdir)?;

        let git = GitBinary::new(&name, subdir.as_path())?;
        assert_eq!(
            Some(git.git_dir.as_str()),
            tmpdir.path().join(".git").to_str(),
        );

        Ok(())
    }

    #[test]
    fn read_empty_config() -> Result<()> {
        let (name, tmpdir) = GitBinary::init()?;
        let git = GitBinary::new(&name, tmpdir.path())?;

        let got = git.get_config("test.key")?;
        assert_eq!(got, None);

        Ok(())
    }

    #[test]
    fn write_then_read_config() -> Result<()> {
        let (name, tmpdir) = GitBinary::init()?;
        let git = GitBinary::new(&name, tmpdir.path())?;

        git.set_config("test.key", "testvalue")?;
        let got = git.get_config("test.key")?;

        assert_eq!(got, Some("testvalue".to_string()));

        Ok(())
    }
}
