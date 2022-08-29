# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

As an application (not a library!), backwards compatibility is defined as:

- New versions of the application should keep working with configuration created for and by older versions.
- New versions of the application should keep working in environments where the old version used to work.
- New versions should maintain the command line interface of older versions.

However, the output of the application is designed for humans, not machines, and is thus exempt from these backwards compatibility promises. [File an issue](https://github.com/rraval/git-nomad/issues/new) if you have a use case for machine readable output.

## [Unreleased]

### Added

- `purge` now takes the `<remote>` to delete refs from.
- `ls` gained a `--fetch` option to fetch refs from the `<remote>` before listing.

### Changed

- `<remote>` is now a global option instead of a positional argument. It can be specified via `-R`, `--remote`, or the `GIT_NOMAD_REMOTE` environment variable. It influences the git remote that `ls`, `sync`, `purge` operate on.
- The `--silent` option has been renamed to `--quiet`.
- Cosmetic tweaks to how command failures are reported.

## [0.5.0] - 2022-01-15

`git-nomad` is no longer considered a prototype and is approaching its (hopefully final) 1.0 release.

### Changed

- Update to the 2021 edition of Rust.
- Update to clap v3 for the command line interface.
- Help messages are no longer colorized, matching the lack of color throughout the rest of the implementation.
- Assorted minor version updates to various dependencies.

### Fixed

- Adjust help message to only suggest `-vv` for max verbosity.

## [0.4.0] - 2021-12-26

- An internal rewrite of the implementation to prevent entire categories of bugs like [#1][i1], [#2][i2], and [#3][i3] from sneaking in again.
- End-to-end tests that validate the entire workflow.
- Performance and memory optimizations.

### Changed

- [#4][i4]: The `prune` subcommand has been renamed to `purge`.
- [#5][i5]: The `init` subcommand no longer exists. Other subcommands like `sync` and `purge` take new options and read from the git configuration directly. Starting to use `git-nomad` now only requires one command (`sync`) instead of two (`init` then `sync`).

### Fixed

- [#2][i2]: A severe bug related to purging refs from other hosts, i.e. after you wish to stop using git-nomad.

## [0.3.2] - 2021-12-18

- Patch release to test the automated release workflow.

## [0.3.1] - 2021-12-18

- Updates some minor dependencies.
- Patch release to test the automated release workflow.

## [0.3.0] - 2021-12-18

### Changed

- Skip running pre-push hooks for git-nomad operations. This allows pushing work-in-progress branches with lint or compile errors (if the repo has pre-push hooks that check that).

## [0.2.1] - 2021-11-06

### Fixed

- Allow non-git based builds to work again, with several fallbacks to compute the `--version`.

## [0.2.0] - 2021-11-06

### Changed

- The `--version` information now leverages `git describe` to properly capture the precise revision the binary was built from.

### Fixed

- [#1][i1]: Clean up deleted branches from other hosts.

## [0.1.1] - 2021-05-30

### Added

- Support for a `--version` flag that reports the crate version.

## [0.1.0] - 2021-05-30

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
