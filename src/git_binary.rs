use anyhow::{bail, Result};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub struct GitBinary<'a> {
    name: &'a OsStr,
    git_dir: PathBuf,
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
        .and_then(LineArity::one)
        .map(PathBuf::from)?;

        Ok(GitBinary { name, git_dir })
    }
}

fn check_run(command: &mut Command) -> Result<Output> {
    let output = command.output()?;

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
                    LineArity::One(last)
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

        let git_binary = GitBinary::new(&name, tmpdir.path())?;
        assert_eq!(git_binary.git_dir, tmpdir.path().join(".git"));

        Ok(())
    }

    #[test]
    fn toplevel_in_subdir() -> Result<()> {
        let (name, tmpdir) = GitBinary::init()?;
        let subdir = tmpdir.path().join("subdir");
        create_dir(&subdir)?;

        let git_binary = GitBinary::new(&name, subdir.as_path())?;
        assert_eq!(git_binary.git_dir, tmpdir.path().join(".git"));

        Ok(())
    }
}
