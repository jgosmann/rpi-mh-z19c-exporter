# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2023-02-17

### Changed

* Updated dependencies


## [0.2.1] - 2022-09-24

### Fixed

* Error message strings
* Exit with non-zero exit code if worker process dies

## [0.2.0] - 2022-09-24

### Changed

* Error handling changes:
  * Print the actual error if reading the sensor fails.
  * Exit the exporter if reading the sensor fails.
  * Simplify error handling.


## [0.1.4] - 2021-10-28

### Fixed

* The program will exit when the worker dies


## [0.1.3] - 2021-04-22

### Fixed

* Links in changelog


## [0.1.2] - 2021-04-22

### Added

* Changelog

### Fixed

* Invalid crates.io keyword in `Cargo.toml`


## [0.1.1] - 2021-04-16

### Fixed

* Print correct application name at start of log.
* systemd unit name/description is now shorter


## [0.1.0] - 2021-04-16

Initial release.

[Unreleased]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.1.4...v0.2.0
[0.1.4]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/jgosmann/rpi-mh-z19c-exporter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jgosmann/rpi-mh-z19c-exporter/releases/tag/v0.1.0

