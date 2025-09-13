# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

As an application (not a library!), backwards compatibility is defined as:

- New versions of the application should keep working with configuration created for and by older versions.
- New versions of the application should keep working in environments where the old version used to work.
- New versions should maintain the command line interface of older versions.

However, the output of the application is designed for humans, not machines, and is thus exempt from these backwards compatibility promises. [File an issue](https://github.com/rraval/git-nomad/issues/new) if you have a use case for machine readable output.

## [vX.X.X] - Unreleased

## [0.9.0] - 2025-09-13

### Changed

- Update to the 2024 edition of Rust.

## [0.8.0] - 2024-12-14

### Added

- [#170][i170]: A new `completions` subcommand that generates shell completions (thanks @shanesveller).

### Fixed

- Glitchy output where progress bars would sometimes overwrite normal text.
- [#180][i180]: `git nomad ls` now properly displays branches with `/` in their name (thanks @shanesveller).

## [v0.7.1] - 2023-10-22

### Security

- Upgraded `rustix` dependency to mitigate [GHSA-c827-hfw6-qwvm](https://github.com/advisories/GHSA-c827-hfw6-qwvm). Cursory review indicates that `git-nomad` only uses this dependency via `tempfile`, which is only used by testing. Still, users are advised to upgrade out of an abundance of caution.

## [v0.7.0] - 2023-09-22

### Changed

- `git` invocations will now read system and global configuration, so things like credential helpers will will be respected in the underlying `git push` that `git nomad sync` makes. Fixes [#125][i125].

## [v0.6.0] - 2022-10-10

### Added

- `ls` has gained a number of new options:
  - `--fetch` to fetch refs from the `<remote>` before listing.
  - `--print` to choose how output should be printed.
  - `--head` and `--branch` to filter the output to specific branches.
  - `--print-self` for current host refs to be printed.
- `purge` now takes the `<remote>` to delete refs from.

### Changed

- `ls` no longer implicitly prints refs for the current host (see `--print-self`).
- `<remote>` is now a global option instead of a positional argument. It can be specified via `-R`, `--remote`, or the `GIT_NOMAD_REMOTE` environment variable. It influences the git remote that `ls`, `sync`, `purge` operate on.
- `--host` is now a global option instead of subcommand specific.
- The `--silent` option has been renamed to `--quiet`.
- Update to clap v4 for the command line interface.
- Cosmetic tweaks to how command failures are reported.

## Removed

- Support for multiple hosts in the `purge` subcommand. Do it one at a time with the `--host` global option or via `--all`.

## [v0.5.0] - 2022-01-15

`git-nomad` is no longer considered a prototype and is approaching its (hopefully final) 1.0 release.

### Changed

- Update to the 2021 edition of Rust.
- Update to clap v3 for the command line interface.
- Help messages are no longer colorized, matching the lack of color throughout the rest of the implementation.
- Assorted minor version updates to various dependencies.

### Fixed

- Adjust help message to only suggest `-vv` for max verbosity.

## [v0.4.0] - 2021-12-26

- An internal rewrite of the implementation to prevent entire categories of bugs like [#1][i1], [#2][i2], and [#3][i3] from sneaking in again.
- End-to-end tests that validate the entire workflow.
- Performance and memory optimizations.

### Changed

- [#4][i4]: The `prune` subcommand has been renamed to `purge`.
- [#5][i5]: The `init` subcommand no longer exists. Other subcommands like `sync` and `purge` take new options and read from the git configuration directly. Starting to use `git-nomad` now only requires one command (`sync`) instead of two (`init` then `sync`).

### Fixed

- [#2][i2]: A severe bug related to purging refs from other hosts, i.e. after you wish to stop using git-nomad.

## [v0.3.2] - 2021-12-18

- Patch release to test the automated release workflow.

## [v0.3.1] - 2021-12-18

- Updates some minor dependencies.
- Patch release to test the automated release workflow.

## [v0.3.0] - 2021-12-18

### Changed

- Skip running pre-push hooks for git-nomad operations. This allows pushing work-in-progress branches with lint or compile errors (if the repo has pre-push hooks that check that).

## [v0.2.1] - 2021-11-06

### Fixed

- Allow non-git based builds to work again, with several fallbacks to compute the `--version`.

## [v0.2.0] - 2021-11-06

### Changed

- The `--version` information now leverages `git describe` to properly capture the precise revision the binary was built from.

### Fixed

- [#1][i1]: Clean up deleted branches from other hosts.

## [v0.1.1] - 2021-05-30

### Added

- Support for a `--version` flag that reports the crate version.

## [v0.1.0] - 2021-05-30

An initial release with a reasonable complete implementation.

### Added

- `init` to configure user and host names to use
- `ls` to display all nomad managed refs
- `prune` to remove local and remote nomad managed refs
- `sync` to reconcile local and remote state

[i1]: https://github.com/rraval/git-nomad/issues/1
[i2]: https://github.com/rraval/git-nomad/issues/2
[i3]: https://github.com/rraval/git-nomad/issues/3
[i4]: https://github.com/rraval/git-nomad/issues/4
[i5]: https://github.com/rraval/git-nomad/issues/5
[i125]: https://github.com/rraval/git-nomad/issues/125
[i170]: https://github.com/rraval/git-nomad/issues/170
[i180]: https://github.com/rraval/git-nomad/issues/180
