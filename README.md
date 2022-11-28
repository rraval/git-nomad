# git-nomad

[![Coverage](https://img.shields.io/coveralls/github/rraval/git-nomad)](https://coveralls.io/github/rraval/git-nomad)
[![Documentation](https://img.shields.io/github/workflow/status/rraval/git-nomad/Doc?label=docs)](https://rraval.github.io/git-nomad/git_nomad/)
[![Issues](https://img.shields.io/github/issues/rraval/git-nomad)](https://github.com/rraval/git-nomad/issues)
[![Latest Release](https://img.shields.io/github/v/release/rraval/git-nomad)](https://github.com/rraval/git-nomad/releases/latest)
[![Commits since latest release](https://img.shields.io/github/commits-since/rraval/git-nomad/latest)](https://github.com/rraval/git-nomad/commits/master)
[![License](https://img.shields.io/github/license/rraval/git-nomad)](https://github.com/rraval/git-nomad/blob/master/LICENSE)

Synchronize work-in-progress git branches in a light weight fashion. Motivation:

- You frequently work on the same repository from multiple machines, like a laptop and a desktop.
- You frequently rewrite history and use short understandable-by-only-you branch names, making pushing regular branches to git remotes cumbersome.
- You want all the efficiency and synchronization benefits of having a git clone available, meaning external syncing tools like Dropbox or network mounts are right out.
- You want this synchronization to work out-of-the-box with popular third party remote hosts like GitHub.

[![asciicast](https://asciinema.org/a/462028.svg)](https://asciinema.org/a/462028?autoplay=1)

## Usage

[Install `git-nomad`](#installation) to make it available on your `$PATH`.
Assume you're hacking away with your usual git workflow:

```console
rraval@desktop:~/git-nomad$ git checkout -b feature
rraval@desktop:~/git-nomad$ touch new_file
rraval@desktop:~/git-nomad$ git add .
rraval@desktop:~/git-nomad$ git commit -m "new file"
```

Whenever you like, you can push the state of your local branches with:

```console
# Synchronizes with a remote called `origin` by default.
# See `--help` for overriding this explicitly.
rraval@desktop:~/git-nomad$ git nomad sync
Pushing local branches to origin... 3s
Fetching branches from origin... 0s
Listing branches at origin... 3s

desktop
  refs/nomad/desktop/feature -> c340cd55853339e4d039746495cdb80cd9e46123
  refs/nomad/desktop/master -> 267719fb8448cc1cbef2c35a638610573779f2ac
```

---

At some future point, you wish to pick up development on a different machine:

```console
rraval@laptop:~/git-nomad$ git nomad sync
Pushing local branches to origin... 2s
Fetching branches from origin... 1s
Listing branches at origin... 2s

desktop
  refs/nomad/desktop/feature -> 1a101799507ba67d822b97105aafa0ac91ce5183
  refs/nomad/desktop/master -> 267719fb8448cc1cbef2c35a638610573779f2ac
laptop
  refs/nomad/laptop/master -> 267719fb8448cc1cbef2c35a638610573779f2ac
```

Which prints out refs to use to pick up where you left off:

```console
rraval@laptop:~/git-nomad$ git checkout -b feature refs/nomad/desktop/feature
# Hack away where you left off on desktop
```

---

Let's say that the `laptop` machine is where development is happening now, so
you go back to `desktop` to throw away the now outdated branch:

```console
rraval@desktop:~/git-nomad$ git checkout master
rraval@desktop:~/git-nomad$ git branch -D feature
Deleted branch feature (was 1a10179).

rraval@desktop:~/git-nomad$ git nomad sync
Pushing local branches to origin... 2s
Fetching branches from origin... 1s
Listing branches at origin... 0s
Pruning branches at origin... 0s
  Delete refs/nomad/desktop/feature (was 1a101799507ba67d822b97105aafa0ac91ce5183)... 0s

desktop
  refs/nomad/desktop/master -> 267719fb8448cc1cbef2c35a638610573779f2ac
laptop
  refs/nomad/laptop/feature -> dedf3f9d3ad279a401877b351c3ec13aa47cbbd4
  refs/nomad/laptop/master -> 267719fb8448cc1cbef2c35a638610573779f2ac
```

---

If you'd like to stop using `git-nomad` and clean up all the refs it has created:

```console
# See also the `purge --host` option.
rraval@desktop:~/git-nomad$ git nomad purge --all
Fetching branches from origin... 1s
Listing branches at origin... 0s
Pruning branches at origin... 2s
  Delete refs/nomad/desktop/master (was 267719fb8448cc1cbef2c35a638610573779f2ac)... 0s
  Delete refs/nomad/laptop/feature (was dedf3f9d3ad279a401877b351c3ec13aa47cbbd4)... 0s
  Delete refs/nomad/laptop/master (was 267719fb8448cc1cbef2c35a638610573779f2ac)... 0s
```

## How it works

Git is unabashedly a [content-addressed filesystem][git-cafs] that manipulates `blob`, `tree`, and `commit` objects. Layered on top of this is a half decent version control system, though this claim is contentious at best.

Git branches are implemented on top of [a more general scheme called `refs`][git-refs], where the local branch `master` is simply the commit pointed to by `refs/heads/master`. Git reserves a few hierarchies for its own use:

- `refs/heads/*` represent local branches.
- `refs/tags/*` represent tags.
- `refs/remotes/*` represent remote branches.

`git-nomad` works directly with refs to implement its own light weight synchronization scheme:

1. Push local `refs/heads/*` to remote `refs/nomad/{user}/{host}/*`. This allows multiple users on multiple hosts to all use `git-nomad` on the same remote without overwriting data.
2. Fetch remote `refs/nomad/{user}/*` to local `refs/nomad/*`. This makes all the host refs for a given user available in a local clone.
3. Prune local `refs/nomad/*` refs where the corresponding branch has been deleted.

Using refs like this has advantages:

- You only pay the storage cost for the content unique to the branch. The bulk of repository history is shared!
- As refs get cleaned up, `git`s automatic garbage collection should reclaim space.
- Since these refs are under a separate `refs/nomad` hierarchy, they are not subject to the usual fast-forward only rules.

## Installation

### On Linux or Mac OS X

Releases on GitHub have prebuilt binary assets: https://github.com/rraval/git-nomad/releases

1. Download the latest version for your OS.
2. `gunzip` the downloaded file.
3. Place the binary somewhere in your `$PATH`.
4. Check that things work with `git nomad --version`.

### On NixOS / via Nix

There is a [prototype Nix package available][nixpkg] but it has not been integrated into Nixpkgs yet.

### From source

If you have [`cargo`][cargo] available:

```
cargo install git-nomad
```

## Contributing

There are a few ways to make this project better:

1. Try it and [file issues][new-issue] when things break. Use the `-vv` flag to capture all information about the commands that were run.
2. Build packages for various operating systems.

[cargo]: https://www.rust-lang.org/tools/install
[git-cafs]: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
[git-refs]: https://git-scm.com/book/en/v2/Git-Internals-Git-References
[new-issue]: https://github.com/rraval/git-nomad/issues/new
[nixpkg]: https://github.com/rraval/nix/blob/master/box/packages/git-nomad.nix
