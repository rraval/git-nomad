//! See [`Progress`] for the primary entry point.

use std::process::{Command, Output};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

/// What commands to display during workflow execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignificanceVerbosity {
    /// Only slow or otherwise important commands will be displayed.
    OnlyNotable,
    /// All commands will be displayed.
    All,
}

/// How much output to display about invoked commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandVerbosity {
    /// Show a pretty spinner with a description.
    Spinner,
    /// Only print out the command line invocation (binary and arguments).
    Invocation,
    /// Prints what [`Self::Invocation`] would print and also any `stdout`/`stderr` produced.
    InvocationAndOutput,
}

impl CommandVerbosity {
    fn run<S: AsRef<str>>(&self, description: S, command: &mut Command) -> Result<Output> {
        match self {
            Self::Spinner => run_spinner(description, command),
            Self::Invocation => run_with_invocation(description, command),
            Self::InvocationAndOutput => run_with_invocation_and_output(description, command),
        }
    }
}

/// Responsible for timely communication of program state to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verbosity {
    /// Show an internal representation of the workflow about to be invoked.
    pub display_workflow: bool,
    pub significance: SignificanceVerbosity,
    pub command: CommandVerbosity,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self {
            display_workflow: false,
            significance: SignificanceVerbosity::OnlyNotable,
            command: CommandVerbosity::Spinner,
        }
    }
}

impl Verbosity {
    pub const fn verbose() -> Self {
        Self {
            display_workflow: true,
            significance: SignificanceVerbosity::All,
            command: CommandVerbosity::Invocation,
        }
    }

    pub const fn max() -> Self {
        Self {
            display_workflow: true,
            significance: SignificanceVerbosity::All,
            command: CommandVerbosity::InvocationAndOutput,
        }
    }
}

pub fn is_output_allowed(verbosity: Option<Verbosity>) -> bool {
    verbosity.is_some()
}

pub fn run_trivial<S: AsRef<str>>(
    verbosity: Option<Verbosity>,
    description: S,
    command: &mut Command,
) -> Result<Output> {
    match verbosity {
        None => run_silent(description, command),
        Some(verbosity) => match verbosity.significance {
            SignificanceVerbosity::OnlyNotable => run_silent(description, command),
            SignificanceVerbosity::All => verbosity.command.run(description, command),
        },
    }
}

pub fn run_notable<S: AsRef<str>>(
    verbosity: Option<Verbosity>,
    description: S,
    command: &mut Command,
) -> Result<Output> {
    match verbosity {
        None => run_silent(description, command),
        Some(verbosity) => match verbosity.significance {
            SignificanceVerbosity::OnlyNotable | SignificanceVerbosity::All => {
                verbosity.command.run(description, command)
            }
        },
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
fn run_silent<S: AsRef<str>>(description: S, command: &mut Command) -> Result<Output> {
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

fn run_spinner<S: AsRef<str>>(description: S, command: &mut Command) -> Result<Output> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[" ..", ". .", ".. ", "..."])
            .template("{msg}{spinner} {elapsed}"),
    );
    spinner.set_message(description.as_ref().to_owned());
    spinner.enable_steady_tick(150);

    let output = run_silent(description, command);
    spinner.finish();

    output
}

fn run_with_invocation<S: AsRef<str>>(description: S, command: &mut Command) -> Result<Output> {
    eprintln!();
    eprintln!("# {}", description.as_ref());
    eprintln!("$ {:#?}", command);
    run_silent(description, command)
}

fn run_with_invocation_and_output<S: AsRef<str>>(
    description: S,
    command: &mut Command,
) -> Result<Output> {
    let output = run_with_invocation(description, command)?;

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

    Ok(output)
}

#[cfg(test)]
mod test {
    macro_rules! test_run {
        {$name:ident} => {
            mod $name {
                use std::process::Command;

                use crate::verbosity::{output_stdout, $name};

                #[test]
                fn test() {
                    let output = $name(
                        "echo",
                        Command::new("echo").arg("foo"),
                    ).and_then(output_stdout).unwrap();

                    assert_eq!(output, "foo\n");
                }
            }
        };

        {$name:ident, $($rest:ident),+} => {
            test_run! { $name }
            test_run! { $($rest),+ }
        };
    }

    test_run! {
        run_silent,
        run_spinner,
        run_with_invocation,
        run_with_invocation_and_output
    }
}
