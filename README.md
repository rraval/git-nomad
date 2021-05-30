# git-nomad

> ⚠ This application is still in its prototype phase. Really bad things like data loss should not be possible and there's decent test coverage of the core workflows. That said, backwards incompatible iterations in response to public feedback are still on the table. [Help make things better...](#contributing)

Synchronize work-in-progress git branches in a light weight fashion. Motivation:

- You frequently work on the same repository from multiple machines, like a laptop and a desktop.
- You frequently rewrite history and use short understandable-by-only-you branch names, making pushing regular branches to git remotes cumbersome.
- You want all the efficiency and synchronization benefits of having a git clone available, meaning external syncing tools like Dropbox or network mounts are right out.
- You want this synchronization to work out-of-the-box with popular third party remote hosts like GitHub.

## Usage

[Install `git-nomad`](#installation) to make it available on your `$PATH`.

Configure it for a specific git clone:

```console
# Needs to be run once for each clone on each machine.
# Note that this defaults to your username and hostname.
# See `--help` for overriding this explicitly.
rraval@apollo:~/git-nomad$ git nomad init
Wrote Config {
    user: "rraval",
    host: "apollo",
}
```

Now hack away with your usual git workflow:

```console
rraval@apollo:~/git-nomad$ git checkout -b feature
rraval@apollo:~/git-nomad$ touch new_file
rraval@apollo:~/git-nomad$ git add .
rraval@apollo:~/git-nomad$ git commit -m "new file"
```

Whenever you like, you can push the state of your local branches with:

```console
# Synchronizes with a remote called `origin` by default.
# See `--help` for overriding this explicitly.
rraval@apollo:~/git-nomad$ git nomad sync
Pushing local branches to origin... 2s
Fetching branches from origin... 1s

apollo
  refs/nomad/apollo/feature -> e02800d10b11ae03a93e43b8f7fc17b70dfe7acf
  refs/nomad/apollo/master -> fe8bf41bbaf201c0506b60677f03a23da2873fdc
```

---

At some future point, you wish to pick up development on a different machine:

```console
# Only needs to be run once per clone on each machine.
rraval@boreas:~/git-nomad$ git nomad init
Wrote Config {
    user: "rraval",
    host: "boreas",
}
```

You can now run `sync` on this new machine as well:

```console
rraval@boreas:~/git-nomad$ git nomad sync
Pushing local branches to origin... 1s
Fetching branches from origin... 1s

apollo
  refs/nomad/apollo/feature -> e02800d10b11ae03a93e43b8f7fc17b70dfe7acf
  refs/nomad/apollo/master -> fe8bf41bbaf201c0506b60677f03a23da2873fdc
boreas
  refs/nomad/boreas/master -> fe8bf41bbaf201c0506b60677f03a23da2873fdc
```

Which prints out refs to use to pick up where you left off:

```console
rraval@boreas:~/git-nomad$ git checkout -b feature refs/nomad/apollo/feature
# Hack away where you left off on apollo
```

---

Let's say that the `boreas` machine is where development is happening now, so
you go back to `apollo` to throw away the now outdated branch:

```console
rraval@apollo:~/git-nomad$ git checkout master
rraval@apollo:~/git-nomad$ git branch -D feature
Deleted branch feature (was e02800d).

rraval@apollo:~/git-nomad$ git nomad sync
Pushing local branches to origin... 1s
Fetching branches from origin... 1s
Pruning branches at origin... 2s
  Delete refs/nomad/apollo/feature (was e02800d10b11ae03a93e43b8f7fc17b70dfe7acf)... 0s

apollo
  refs/nomad/apollo/master -> fe8bf41bbaf201c0506b60677f03a23da2873fdc
boreas
  refs/nomad/boreas/feature -> 3187d762ca557bfa741bc07d47e0b7f8c1777400
  refs/nomad/boreas/master -> fe8bf41bbaf201c0506b60677f03a23da2873fdc
```

---

If you'd like to stop using `git-nomad` and clean up all the refs it has created:

```console
# Needs to be run once per clone where `git nomad init` has been run.
# See also the `prune --host` option.
rraval@apollo:~/git-nomad$ git nomad prune --all
Fetching branches from origin... 1s
Pruning branches at origin... 2s
  Delete refs/nomad/apollo/master (was fe8bf41bbaf201c0506b60677f03a23da2873fdc)... 0s
  Delete refs/nomad/boreas/feature (was 3187d762ca557bfa741bc07d47e0b7f8c1777400)... 0s
  Delete refs/nomad/boreas/master (was fe8bf41bbaf201c0506b60677f03a23da2873fdc)... 0s
```

## How it works

Git is unabashedly a [content-addressed filesystem][git-cafs] that manipulates `blob`, `tree`, and `commit` objects. Layered on top of this is a half decent version control system, though this claim is contentious at best.

Git branches are implemented on top of [a more general scheme called `refs`][git-refs], where the local branch `master` is simply the commit pointed to by `refs/heads/master`. Git reserves a few hierarchies for its own use:

- `refs/heads/*` represent local branches.
- `refs/tags/*` represent tags.
- `refs/remotes/*` represent remote branches.

`git-nomad` works directly with refs to implement [its own light weight synchronization scheme][sync]:

1. Push local `refs/heads/*` to remote `refs/nomad/{user}/{host}/*`. This allows multiple users on multiple hosts to all use `git-nomad` on the same remote without overwriting data.
2. Fetch remote `refs/nomad/{user}/*` to local `refs/nomad/*`. This makes all the host refs for a given user available in a local clone.
3. Prune local `refs/nomad/*` refs where the corresponding branch has been deleted.

Using refs like this has advantages:

- You only pay the storage cost for the content unique to the branch. The bulk of repository history is shared!
- As refs get cleaned up, `git`s automatic garbage collection should reclaim space.
- Since these refs are under a separate `refs/nomad` hierarchy, they are not subject to the usual fast-forward only rules.

## Installation

### On NixOS / via Nix

There is a [prototype Nix package available][nixpkg] but it has not been integrated into Nixpkgs yet.

### From source

If you have [`cargo`][cargo] available:

```
cargo install git-nomad
```

## Contributing

There are a few ways to make this project better:

1. Try it and [file issues][new-issue] when things break. Use the `-vvv` flag to capture all information about the commands that were run.
2. Build packages for various operating systems.

[cargo]: https://www.rust-lang.org/tools/install
[git-cafs]: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
[git-refs]: https://git-scm.com/book/en/v2/Git-Internals-Git-References
[new-issue]: https://github.com/rraval/git-nomad/issues/new
[nixpkg]: https://github.com/rraval/nix/blob/master/git-nomad.nix
[sync]: https://github.com/rraval/git-nomad/blob/master/src/command.rs
