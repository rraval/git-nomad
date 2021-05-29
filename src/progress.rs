use std::process::{Command, Output};

use anyhow::{bail, Context, Result};

#[derive(PartialOrd, PartialEq, Eq, Ord)]
pub enum Run {
    Trivial,
    Notable,
}

pub enum Verbosity {
    CommandOnly,
    CommandAndOutput,
}

pub enum Progress {
    Silent,
    Standard { significance_at_least: Run },
    Verbose(Verbosity),
}

impl Progress {
    pub fn is_output_allowed(&self) -> bool {
        match self {
            Self::Silent => false,
            Self::Standard { .. } => true,
            Self::Verbose(_) => true,
        }
    }

    pub fn run<S: AsRef<str>>(
        &self,
        significance: Run,
        description: S,
        command: &mut Command,
    ) -> Result<Output> {
        match self {
            Self::Silent => check_run(description, command),
            Self::Standard {
                significance_at_least,
            } => {
                if significance >= *significance_at_least {
                    eprintln!("{}...", description.as_ref());
                }
                check_run(description, command)
            }
            Self::Verbose(verbosity) => {
                eprintln!();
                eprintln!("# {}", description.as_ref());
                eprintln!("$ {:#?}", command);
                let output = check_run(description, command)?;

                match verbosity {
                    Verbosity::CommandOnly => (),
                    Verbosity::CommandAndOutput => {
                        let forward = |name: &str, stream: &[u8]| {
                            if !stream.is_empty() {
                                // Ideally this would use `stderr.write_all` to simply forward the raw bytes
                                // onward, but that does not play nice with `cargo test`s output capturing.
                                //
                                // In practice, we only wrap `git` which produces UTF8, so a conversion here is
                                // okay.
                                eprintln!("{}", String::from_utf8_lossy(stream));
                                eprintln!("# ---- END {} ----", name);
                            }
                        };

                        forward("STDOUT", &output.stdout);
                        forward("STDERR", &output.stderr);
                    }
                }

                Ok(output)
            }
        }
    }
}

pub fn output_stdout(output: Output) -> Result<String> {
    Ok(String::from_utf8(output.stdout)?)
}

fn check_run<S: AsRef<str>>(description: S, command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("{}: {:?}", description.as_ref(), command))?;

    if !output.status.success() {
        let forward = |name: &str, stream: &[u8]| {
            if stream.is_empty() {
                String::new()
            } else {
                format!(
                    "\n# ---- {} ----\n{}",
                    name,
                    String::from_utf8_lossy(stream)
                )
            }
        };

        bail!(
            "command failure\n$ {:?}\n# Exit code: {:?}{}{}",
            command,
            output.status.code(),
            forward("STDOUT", &output.stdout),
            forward("STDERR", &output.stderr)
        );
    }

    Ok(output)
}
