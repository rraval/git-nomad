//! See [`Progress`] for the primary entry point.

use std::process::{Command, Output};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

/// Significance of the command being run to the overall workflow.
#[derive(Clone, Copy, PartialOrd, PartialEq, Eq, Ord)]
pub enum Run {
    /// An insignificant command that requires additional `--verbose` flags to be visible.
    Trivial,
    /// A slow or otherwise important command to communicate to the user.
    Notable,
}

/// How verbose should `--verbose` be?
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    /// Prints the command and arguments as it executes them.
    CommandOnly,
    /// Prints what [`Self::CommandOnly`] would print and also any `stdout`/`stderr` produced.
    CommandAndOutput,
}

/// Responsible for timely communication of program state to the user.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// No progress messages whatsoever, the user only cares about the exit code.
    Silent,
    /// Every day usage of this program.
    ///
    /// When run interactively, this presents pretty spinners with durations powered by
    /// [`indicatif`].
    Standard {
        /// Skips printing commands with [`Run`] values below what is specified.
        significance_at_least: Run,
    },
    /// Debug usage of this program. Prints increasing amounts of information about subcommands.
    Verbose(Verbosity),
}

impl Progress {
    /// Should the workflow print completion messages?
    pub fn is_output_allowed(&self) -> bool {
        match self {
            Self::Silent => false,
            Self::Standard { .. } => true,
            Self::Verbose(_) => true,
        }
    }

    /// Run a [`Command`] for its [`Output`], while reporting progress to the user appropriately.
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
                let spinner = if significance >= *significance_at_least {
                    let spinner = ProgressBar::new_spinner();
                    spinner.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&[" ..", ". .", ".. ", "..."])
                            .template("{msg}{spinner} {elapsed}"),
                    );
                    spinner.set_message(description.as_ref().to_owned());
                    spinner.enable_steady_tick(150);
                    Some(spinner)
                } else {
                    None
                };

                let output = check_run(description, command);

                if let Some(spinner) = spinner {
                    spinner.finish();
                }

                output
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

/// Extract the printed `stdout` from the [`Output`] of a [`Command`].
///
/// Best used in an `and_then` chain.
pub fn output_stdout(output: Output) -> Result<String> {
    Ok(String::from_utf8(output.stdout)?)
}

/// Invoke a [`Command`] and check its exit code for success.
///
/// Makes effort to build a decent error message on failure.
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
