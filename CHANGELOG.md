# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

As an application (not a library!), backwards compatibility is defined as:

- New versions of the application should keep working with configuration created for and by older versions.
- New versions of the application should keep working in environments where the old version used to work.
- New versions should maintain the command line interface of older versions.

However, the output of the application is designed for humans, not machines, and is thus exempt from these backwards compatibility promises. [File an issue](https://github.com/rraval/git-nomad/issues/new) if you have a use case for machine readable output.

## [Unreleased]

### Changed
- The `--version` information now leverages `git describe` to properly capture the precise revision the binary was built from.

### Fixed
- Clean up deleted branches from other hosts. See https://github.com/rraval/git-nomad/issues/1.

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
