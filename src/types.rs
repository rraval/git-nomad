/// The primary configuration that nomad works off.
#[derive(Debug)]
pub struct Config {
    /// Represents "who" a given branch belongs to. This value should be shared by multiple git
    /// clones that belong to the same user.
    ///
    /// This string is used when pushing branches to the remote so that multiple users can use
    /// nomad on that remote without stepping on each other.
    pub user: String,

    /// Represents "where" a given branch comes from. This value should be unique for every git
    /// clone belonging to a specific user.
    ///
    /// This string is used when pushing branches to the remote so that multiple hosts belonging to
    /// the same user can co-exist (i.e. the whole point of nomad).
    ///
    /// This string is also used when pulling branches for all hosts of the current user
    /// and for detecting when branches have been deleted.
    pub host: String,
}

/// A remote git repository identified by name, like `origin`.
pub struct Remote(pub String);

/// The branch name part of a ref. `refs/head/master` would be `Branch("master".to_string())`.
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Branch(pub String);

impl Branch {
    pub fn str<S: AsRef<str>>(s: S) -> Branch {
        Branch(s.as_ref().to_string())
    }
}

/// A ref representing a branch managed by nomad.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct NomadRef<Ref> {
    /// The user this branch belongs to.
    pub user: String,
    /// The host this branch comes from.
    pub host: String,
    /// The branch name.
    pub branch: Branch,
    /// Any additional data the [`Backend`] would like to carry around.
    pub ref_: Ref,
}
