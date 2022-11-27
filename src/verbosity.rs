//! Helpers for executing [`Command`]s and parsing their [`Output`].

use std::{
    process::{Command, Output},
    time::Duration,
};

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
    /// Show the version information for debugging.
    pub display_version: bool,

    pub significance: SignificanceVerbosity,
    pub command: CommandVerbosity,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::standard()
    }
}

impl Verbosity {
    // workaround for `Default::default` not being able to be a `const fn`.
    const fn standard() -> Self {
        Self {
            display_workflow: false,
            display_version: false,
            significance: SignificanceVerbosity::OnlyNotable,
            command: CommandVerbosity::Spinner,
        }
    }

    pub const fn verbose() -> Self {
        Self {
            display_workflow: true,
            display_version: false,
            significance: SignificanceVerbosity::All,
            command: CommandVerbosity::Invocation,
        }
    }

    pub const fn max() -> Self {
        Self {
            display_workflow: true,
            display_version: true,
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
fn run_silent<S: AsRef<str>>(description: S, command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("{}: {:?}", description.as_ref(), command))?;

    if !output.status.success() {
        return dump_command_failure(command, &output);
    }

    Ok(output)
}

/// Make some effort to build a decent error message for commands that fail.
fn dump_command_failure<T>(command: &Command, output: &Output) -> Result<T> {
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
        "command failure\n$ {:?}\n# status: {}{}{}",
        command,
        output.status,
        forward("STDOUT", &output.stdout),
        forward("STDERR", &output.stderr)
    );
}

fn run_spinner<S: AsRef<str>>(description: S, command: &mut Command) -> Result<Output> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[" ..", ". .", ".. ", "..."])
            .template("{msg}{spinner} {elapsed}")
            .unwrap(),
    );
    spinner.set_message(description.as_ref().to_owned());
    spinner.enable_steady_tick(Duration::from_millis(150));

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
    use std::{
        os::unix::prelude::ExitStatusExt,
        process::{Command, ExitStatus, Output},
    };

    use crate::verbosity::{run_notable, run_silent};

    use super::{dump_command_failure, output_stdout, run_trivial, Verbosity};

    const ALL_VERBOSITIES: &[Option<Verbosity>] = &[
        None,
        Some(Verbosity::standard()),
        Some(Verbosity::verbose()),
        Some(Verbosity::max()),
    ];

    #[test]
    fn test_trivial_success() {
        for verbosity in ALL_VERBOSITIES {
            println!("{:?}", verbosity);
            let output = run_trivial(*verbosity, "echo", Command::new("echo").arg("foo"))
                .and_then(output_stdout)
                .unwrap();
            assert_eq!(output, "foo\n");
        }
    }

    #[test]
    fn test_notable_success() {
        for verbosity in ALL_VERBOSITIES {
            println!("{:?}", verbosity);
            let output = run_notable(*verbosity, "echo", Command::new("echo").arg("foo"))
                .and_then(output_stdout)
                .unwrap();
            assert_eq!(output, "foo\n");
        }
    }

    #[test]
    fn test_failure() {
        let output = run_silent("failure", &mut Command::new("false"));
        assert!(output.is_err());
        match output {
            Ok(_) => unreachable!(),
            Err(e) => assert!(e.to_string().contains("false")), // the command that was invoked
        }
    }

    /// Ensures that [`dump_command_failure`] prints all available information so the user can
    /// figure out what went wrong.
    #[test]
    fn test_dump_command_failure_stdout_and_stderr() {
        let mut command = Command::new("binary");
        command.arg("arg");

        let output = Output {
            status: ExitStatus::from_raw(123),
            stdout: "some stdout".as_bytes().to_vec(),
            stderr: "some stderr".as_bytes().to_vec(),
        };

        let dump = dump_command_failure::<()>(&command, &output).unwrap_err();
        let displayed_dump = format!("{}", dump);

        assert!(displayed_dump.contains("binary"));
        assert!(displayed_dump.contains("arg"));
        assert!(displayed_dump.contains("123"));
        assert!(displayed_dump.contains("STDOUT"));
        assert!(displayed_dump.contains("some stdout"));
        assert!(displayed_dump.contains("STDERR"));
        assert!(displayed_dump.contains("some stderr"));
    }

    /// [`dump_command_failure`] should elide stderr when it is empty.
    #[test]
    fn test_dump_command_failure_just_stdout() {
        let command = Command::new("binary");

        let output = Output {
            status: ExitStatus::from_raw(123),
            stdout: "some stdout".as_bytes().to_vec(),
            stderr: Vec::new(),
        };

        let dump = dump_command_failure::<()>(&command, &output).unwrap_err();
        let displayed_dump = format!("{}", dump);

        assert!(displayed_dump.contains("binary"));
        assert!(!displayed_dump.contains("arg"));
        assert!(displayed_dump.contains("123"));
        assert!(displayed_dump.contains("STDOUT"));
        assert!(displayed_dump.contains("some stdout"));
        assert!(!displayed_dump.contains("STDERR"));
        assert!(!displayed_dump.contains("some stderr"));
    }
}
