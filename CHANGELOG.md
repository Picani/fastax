# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
## Changed
- When outputting a tree in Newick, if the root has only one child,
  then the root is removed from the output.

## [1.3.2] -- 2019-12-31
### Fixed
- Generated Newick trees had bad internal nodes in some situation.

## [1.3.1] -- 2019-12-31
### Changed
- Code reorganization.
- Multiple small internal changes.

## [1.3.0] -- 2019-10-16
### Added
- `-f/--format` option to format nodes in trees.

### Changed
- Update dependencies.

## [1.2.0] -- 2019-09-11
### Added
- `subtree` command that takes a node and makes the tree with this node as
  root.

## [1.1.1] -- 2019-09-10
### Fixed
- Change the index on the `names` table, fixing performance issues.

## [1.1.0] -- 2019-07-24
### Added
- All commands can use scientific name as argument, in addition to NCBI
  taxonomic ID.

## [1.0.1] -- 2019-06-23
### Changed
- Improve the error message when the database file is not present.

## [1.0.0] -- 2019-06-09
### Added
- Initial release of a working application.

[Unreleased]: https://github.com/Picani/fastax
[1.3.2]: https://github.com/Picani/fastax/releases/tag/v1.3.2
[1.3.1]: https://github.com/Picani/fastax/releases/tag/v1.3.1
[1.3.0]: https://github.com/Picani/fastax/releases/tag/v1.3.0
[1.2.0]: https://github.com/Picani/fastax/releases/tag/v1.2.0
[1.1.1]: https://github.com/Picani/fastax/releases/tag/v1.1.1
[1.1.0]: https://github.com/Picani/fastax/tree/d877e5b2d44aed82acc646a9ba4a930e263c1d22
[1.0.1]: https://github.com/Picani/fastax/tree/731468f3b8abdc7cc859bb0e30aa1da84e1a22d3
[1.0.0]: https://github.com/Picani/fastax/tree/9f1a6ba928ab1661b95cd5bfa0e1b799b380debf
