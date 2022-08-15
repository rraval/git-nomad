use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    iter::FromIterator,
};

/// Convenient [`From`] implementations for `Cow<'_, str>` based newtypes.
macro_rules! impl_str_from {
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

/// Additional utility methods for `Cow<'_, str>` based newtypes.
macro_rules! impl_str_ownership {
    ($typename:ident) => {
        /// Takes ownership of non-`'static` borrowed data, possibly allocating a
        /// `String` to do so.
        ///
        /// Convenient representation for types that want to stake ownership of the newtype without
        /// exposing a generic lifetime of their own.
        impl $typename<'_> {
            pub fn possibly_clone(self) -> $typename<'static> {
                let owned = self.0.into_owned();
                $typename(Cow::Owned(owned))
            }
        }

        /// Returns a copy of itself while guaranteeing zero allocations.
        ///
        /// Useful for standard containers that use the `Borrow + Hash + Eq` sleight of hand to
        /// permit zero allocation lookups while still owning the underlying data.
        impl<'a> $typename<'a> {
            pub fn always_borrow(&'a self) -> Self {
                let y: &str = self.0.borrow();
                Self::from(y)
            }
        }
    };
}

/// A remote git repository identified by name, like `origin`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote<'a>(pub Cow<'a, str>);
impl_str_from!(Remote);

/// The branch name part of a ref. `refs/head/master` would be `Branch::from("master")`.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Branch<'a>(pub Cow<'a, str>);
impl_str_from!(Branch);
impl_str_ownership!(Branch);

/// Represents "who" a given branch belongs to. This value should be shared by multiple git
/// clones that belong to the same user.
///
/// This string is used when pushing branches to the remote so that multiple users can use
/// nomad on that remote without overwriting each others refs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct User<'a>(pub Cow<'a, str>);
impl_str_from!(User);
impl_str_ownership!(User);

/// Represents "where" a given branch comes from. This value should be unique for every git
/// clone belonging to a specific user.
///
/// This string is used when pushing branches to the remote so that multiple hosts belonging to
/// the same user can co-exist (i.e. the whole point of nomad).
///
/// This string is also used when pulling branches for all hosts of the current user
/// and for detecting when branches have been deleted.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Host<'a>(pub Cow<'a, str>);
impl_str_from!(Host);
impl_str_ownership!(Host);

/// A ref representing a branch managed by nomad.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct NomadRef<'a, Ref> {
    /// The user this branch belongs to.
    pub user: User<'a>,
    /// The host this branch comes from.
    pub host: Host<'a>,
    /// The branch name.
    pub branch: Branch<'a>,
    /// Any additional internal data representing the underlying git ref.
    pub ref_: Ref,
}

/// A specialized container to represent nomad managed refs that a remote knows about.
pub struct RemoteNomadRefSet {
    set: HashSet<(User<'static>, Host<'static>, Branch<'static>)>,
}

impl RemoteNomadRefSet {
    /// Check whether the remote knows about a given [`NomadRef`].
    ///
    /// Note that the `Ref` part of `NomadRef<Ref>` is completely ignored, since we don't care
    /// about the intrinsic git ref being pointed to, merely that the remote is still tracking a
    /// nomad ref with the given user/host/branch.
    pub fn contains<Ref>(&self, nomad_ref: &NomadRef<Ref>) -> bool {
        // Performs a lookup without allocating.
        //
        // https://users.rust-lang.org/t/using-hashset-contains-with-tuple-types-without-takeing-ownership-of-the-values/65455
        // https://stackoverflow.com/questions/45786717/how-to-implement-hashmap-with-two-keys/45795699#45795699
        self.set.contains(&(
            nomad_ref.user.always_borrow(),
            nomad_ref.host.always_borrow(),
            nomad_ref.branch.always_borrow(),
        ))
    }
}

impl<'a> FromIterator<(User<'a>, Host<'a>, Branch<'a>)> for RemoteNomadRefSet {
    fn from_iter<T: IntoIterator<Item = (User<'a>, Host<'a>, Branch<'a>)>>(iter: T) -> Self {
        let set = HashSet::from_iter(iter.into_iter().map(|(user, host, branch)| {
            (
                user.possibly_clone(),
                host.possibly_clone(),
                branch.possibly_clone(),
            )
        }));
        RemoteNomadRefSet { set }
    }
}

impl<'a, Ref> FromIterator<NomadRef<'a, Ref>> for RemoteNomadRefSet {
    fn from_iter<T: IntoIterator<Item = NomadRef<'a, Ref>>>(iter: T) -> Self {
        Self::from_iter(iter.into_iter().map(|nomad_ref| {
            let NomadRef {
                user, host, branch, ..
            } = nomad_ref;
            (user, host, branch)
        }))
    }
}
