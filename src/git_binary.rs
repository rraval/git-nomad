use anyhow::{bail, Context, Result};
use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};

use crate::backend::{Backend, Config};

pub struct GitBinary<'a> {
    name: &'a OsStr,
    git_dir: String,
}

fn config_key(key: &str) -> String {
    format!("nomad.{}", key)
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

    fn get_config(&self, key: &str) -> Result<Option<String>> {
        check_output(Command::new(self.name).args(&[
            "--git-dir",
            &self.git_dir,
            "config",
            "--local",
            // Use a default to prevent git from returning a non-zero exit code when the value does
            // not exist.
            "--default",
            "",
            "--get",
            &config_key(key),
        ]))
        .map(LineArity::of)
        .and_then(LineArity::zero_or_one)
    }

    fn set_config(&self, key: &str, value: &str) -> Result<()> {
        check_run(Command::new(self.name).args(&[
            "--git-dir",
            &self.git_dir,
            "config",
            "--local",
            "--replace-all",
            &config_key(key),
            value,
        ]))?;
        Ok(())
    }
}

impl<'a> Backend for GitBinary<'a> {
    fn read_config(&self) -> Result<Option<Config>> {
        let remote = self.get_config("remote")?;
        let user = self.get_config("user")?;
        let host = self.get_config("host")?;

        match (remote, user, host) {
            (Some(remote), Some(user), Some(host)) => Ok(Some(Config { remote, user, host })),
            (None, None, None) => Ok(None),
            (remote, user, host) => {
                bail!("Partial configuration {:?} {:?} {:?}", remote, user, host)
            }
        }
    }

    fn write_config(&self, config: &Config) -> Result<()> {
        self.set_config("remote", &config.remote)?;
        self.set_config("user", &config.user)?;
        self.set_config("host", &config.host)?;
        Ok(())
    }
}

fn check_run(command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("Running {:?}", command))?;

    if !output.status.success() {
        bail!("command failure {:?}", output);
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

        let got = git.get_config("testkey")?;
        assert_eq!(got, None);

        Ok(())
    }

    #[test]
    fn write_then_read_config() -> Result<()> {
        let (name, tmpdir) = GitBinary::init()?;
        let git = GitBinary::new(&name, tmpdir.path())?;

        git.set_config("testkey", "testvalue")?;
        let got = git.get_config("testkey")?;

        assert_eq!(got, Some("testvalue".to_string()));

        Ok(())
    }
}
