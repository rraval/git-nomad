//! See [`GitRef`] for the primary entry point.

use std::{error::Error, fmt};

/// Information about a specific ref in a git repository, analogous to the information
/// that `git show-ref` produces.
///
/// Callers should leverage all the information here for additional safety (for example, using
/// `git update-ref -d <name> <commit_id>` to only delete the reference if it matches the expected
/// commit ID).
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct GitRef {
    /// The hash representing the git commit ID that the ref points to.
    pub commit_id: String,
    /// The full ref name, like `refs/heads/master`.
    pub name: String,
}

/// All the ways a `git show-ref` line can fail to parse.
#[derive(Debug, Eq, PartialEq)]
pub enum GitRefParseError {
    MissingName(String),
    MissingCommitId(String),
    TooManyParts(String),
}

impl fmt::Display for GitRefParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (tag, line) = match self {
            Self::MissingName(line) => ("Missing name", line),
            Self::MissingCommitId(line) => ("Missing commit ID", line),
            Self::TooManyParts(line) => ("Too many parts", line),
        };

        write!(f, "{}: {}", tag, line)
    }
}

impl Error for GitRefParseError {}

// Use an `&S` to avoid compiler quirks: https://stackoverflow.com/a/63917951
fn is_not_empty<S: AsRef<str>>(str: &S) -> bool {
    !str.as_ref().is_empty()
}

impl GitRef {
    /// Utility to parse a `<ref_name><delimiter><commit_id>` line that git likes to output
    /// for various commands.
    fn parse_char_delimited_line(line: &str, delimiter: char) -> Result<GitRef, GitRefParseError> {
        let mut parts = line.split(delimiter).map(String::from).collect::<Vec<_>>();
        let name = parts
            .pop()
            .filter(is_not_empty)
            .ok_or_else(|| GitRefParseError::MissingName(line.to_string()))?;
        let commit_id = parts
            .pop()
            .filter(is_not_empty)
            .ok_or_else(|| GitRefParseError::MissingCommitId(line.to_string()))?;

        if !parts.is_empty() {
            return Err(GitRefParseError::TooManyParts(line.to_string()));
        }

        Ok(GitRef { commit_id, name })
    }

    /// Parse a single line from `git show-ref` as a [`GitRef`].
    pub fn parse_show_ref_line(line: &str) -> Result<GitRef, GitRefParseError> {
        Self::parse_char_delimited_line(line, ' ')
    }

    /// Parse a single line from `git ls-remote` as a [`GitRef`].
    pub fn parse_ls_remote_line(line: &str) -> Result<GitRef, GitRefParseError> {
        Self::parse_char_delimited_line(line, '\t')
    }
}

#[cfg(test)]
mod tests {
    use super::{GitRef, GitRefParseError};

    #[test]
    fn parse() {
        assert_eq!(
            GitRef::parse_show_ref_line("commit_id refs/heads/master"),
            Ok(GitRef {
                commit_id: "commit_id".to_string(),
                name: "refs/heads/master".to_string(),
            })
        );
    }

    fn parse_error<ErrFactory>(line: &str, err: ErrFactory)
    where
        ErrFactory: Fn(String) -> GitRefParseError,
    {
        assert_eq!(
            GitRef::parse_show_ref_line(line),
            Err(err(line.to_string()))
        );
    }

    #[test]
    fn parse_missing_name() {
        parse_error("", GitRefParseError::MissingName);
    }

    #[test]
    fn parse_missing_commit1() {
        parse_error("refs/heads/master", GitRefParseError::MissingCommitId);
    }

    #[test]
    fn parse_missing_commit2() {
        parse_error(" refs/heads/master", GitRefParseError::MissingCommitId);
    }

    #[test]
    fn parse_too_many() {
        parse_error(
            "extra commit_id refs/heads/master",
            GitRefParseError::TooManyParts,
        );
    }

    /// Checks that displaying any [`GitRefParseError`] always includes the string passed in.
    fn assert_display_contains_str(func: impl Fn(String) -> GitRefParseError) {
        let displayed = format!("{}", func("foo".to_string()));
        assert!(displayed.contains("foo"));
    }

    #[test]
    fn display_missing_name() {
        assert_display_contains_str(GitRefParseError::MissingName);
    }

    #[test]
    fn display_missing_commit_id() {
        assert_display_contains_str(GitRefParseError::MissingCommitId);
    }

    #[test]
    fn display_too_many_parts() {
        assert_display_contains_str(GitRefParseError::TooManyParts);
    }
}
