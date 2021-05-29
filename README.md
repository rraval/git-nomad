# git-nomad

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
Pushing local branches to origin... 3s
Fetching branches from origin... 1s
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

FIXME: needs a built in way to list nomad managed refs.

FIXME: needs a way to purge nomad refs.

## How it works

## Installation
