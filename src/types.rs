use std::{borrow::Cow, collections::HashSet, iter::FromIterator};

macro_rules! impl_from_str {
    ($typename:ident) => {
        impl<'a> From<&'a str> for $typename<'a> {
            fn from(s: &'a str) -> Self {
                Self(Cow::from(s))
            }
        }

        impl<'a> From<String> for $typename<'a> {
            fn from(s: String) -> Self {
                Self(Cow::from(s))
            }
        }
    };
}

macro_rules! impl_possibly_clone {
    ($typename:ident) => {
        impl<'a> $typename<'a> {
            pub fn possibly_clone(self) -> $typename<'static> {
                let owned = self.0.into_owned();
                $typename(Cow::Owned(owned))
            }
        }
    };
}

/// A remote git repository identified by name, like `origin`.
pub struct Remote<'a>(pub Cow<'a, str>);
impl_from_str!(Remote);

/// The branch name part of a ref. `refs/head/master` would be `Branch("master".to_string())`.
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Branch<'a>(pub Cow<'a, str>);
impl_from_str!(Branch);
impl_possibly_clone!(Branch);

/// Represents "who" a given branch belongs to. This value should be shared by multiple git
/// clones that belong to the same user.
///
/// This string is used when pushing branches to the remote so that multiple users can use
/// nomad on that remote without stepping on each other.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct User(pub String);

impl User {
    pub fn str<S: AsRef<str>>(s: S) -> Self {
        Self(s.as_ref().to_string())
    }
}

/// Represents "where" a given branch comes from. This value should be unique for every git
/// clone belonging to a specific user.
///
/// This string is used when pushing branches to the remote so that multiple hosts belonging to
/// the same user can co-exist (i.e. the whole point of nomad).
///
/// This string is also used when pulling branches for all hosts of the current user
/// and for detecting when branches have been deleted.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Host(pub String);

impl Host {
    pub fn str<S: AsRef<str>>(s: S) -> Self {
        Self(s.as_ref().to_string())
    }
}

/// A ref representing a branch managed by nomad.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct NomadRef<'branch, Ref> {
    /// The user this branch belongs to.
    pub user: User,
    /// The host this branch comes from.
    pub host: Host,
    /// The branch name.
    pub branch: Branch<'branch>,
    /// Any additional data the [`Backend`] would like to carry around.
    pub ref_: Ref,
}

pub struct RemoteNomadRefSet {
    set: HashSet<(User, Host, Branch<'static>)>,
}

impl RemoteNomadRefSet {
    pub fn contains<Ref>(&self, nomad_ref: &NomadRef<Ref>) -> bool {
        // FIXME: Doing this efficiently is a bit of a Rust puzzle
        // https://users.rust-lang.org/t/using-hashset-contains-with-tuple-types-without-takeing-ownership-of-the-values/65455
        // https://stackoverflow.com/questions/45786717/how-to-implement-hashmap-with-two-keys/45795699#45795699
        self.set.contains(&(
            nomad_ref.user.clone(),
            nomad_ref.host.clone(),
            nomad_ref.branch.clone(),
        ))
    }
}

impl<'branch> FromIterator<(User, Host, Branch<'branch>)> for RemoteNomadRefSet {
    fn from_iter<T: IntoIterator<Item = (User, Host, Branch<'branch>)>>(iter: T) -> Self {
        let set = HashSet::from_iter(
            iter.into_iter()
                .map(|(user, host, branch)| (user, host, branch.possibly_clone())),
        );
        RemoteNomadRefSet { set }
    }
}

impl<'branch, Ref> FromIterator<NomadRef<'branch, Ref>> for RemoteNomadRefSet {
    fn from_iter<T: IntoIterator<Item = NomadRef<'branch, Ref>>>(iter: T) -> Self {
        Self::from_iter(iter.into_iter().map(|nomad_ref| {
            let NomadRef {
                user, host, branch, ..
            } = nomad_ref;
            (user, host, branch)
        }))
    }
}
