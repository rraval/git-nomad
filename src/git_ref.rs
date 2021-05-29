use std::{error::Error, fmt};

/// Information about a specific ref in the local repository, analogous to the information
/// that `git show-ref` produces.
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct GitRef {
    pub commit_id: String,
    pub name: String,
}

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
    pub fn parse_show_ref_line(line: &str) -> Result<GitRef, GitRefParseError> {
        let mut parts = line.split(' ').map(String::from).collect::<Vec<_>>();
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
}
