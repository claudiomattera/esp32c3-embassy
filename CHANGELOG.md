# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Update `esp-hal` to version 1.0.0-beta.0
- Update `esp-hal-embassy` to version 0.7
- Update `esp-wifi` to version 0.13
- Update `esp-alloc` to version 0.7
- Update `embedded-hal-bus` to version 0.3
- Disable feature `critical-section` on crate `esp-println`
- Enable feature `builtin-scheduler` on crate `esp-wifi`


## [0.6.0] - 2025-02-07

### Changed

- Use [Adafruit IO](https://io.adafruit.com) API instead of WorldTimeAPI to fetch current time
- Change license to MIT + APACHE-2.0 for main firmware
- Update `embassy-executor` to version 0.7
- Update `embassy-time` to version 0.4
- Update `embassy-net` to version 0.6
- Update `esp-hal` to version 0.23
- Update `esp-hal-embassy` to version 0.6
- Update `esp-backtrace` to version 0.15
- Update `esp-println` to version 0.13
- Update `esp-wifi` to version 0.12
- Enable feature `critical-section` on crate `esp-println`
- Update `rand_core` to version 0.9


## [0.5.0] - 2024-12-22

### Changed

- Update `embassy-net` to version 0.5
- Update `esp-hal` to version 0.22
- Update `esp-hal-embassy` to version 0.5
- Update `esp-wifi` to version 0.11
- Update `reqwless` to version 0.13
- Update `embedded-hal-bus` to version 0.2
- Refactor function `main()` into smaller functions

### Fixed

- Use correct environment variable in log documentation
- Use correct hash for `rust-toolchain.toml` in Nix flake (contributed by [MaxKiv](https://github.com/MaxKiv/))


## [0.4.0] - 2024-11-05

### Added

- Nix flake (contributed by [MaxKiv](https://github.com/MaxKiv/))

### Changed

- Update `embassy-executor` to version 0.6
- Update `esp-hal` to version 0.21
- Update `esp-hal-embassy` to version 0.4
- Update `esp-backtrace` to version 0.14
- Update `esp-println` to version 0.12
- Update `esp-wifi` to version 0.10
- Add feature `esp-alloc` to `esp-wifi`
- Add `esp-alloc` to version 0.5
- Update `bme208-rs` to version 0.3
- Update `uom` to version 0.36


## [0.3.0] - 2024-09-09

### Changed

- Update `esp-hal` to version 0.19
- Update `esp-hal-embassy` to version 0.2
- Update `esp-backtrace` to version 0.13
- Update `esp-println` to version 0.10
- Update `esp-wifi` to version 0.7


## [0.2.0] - 2024-06-23

### Changed

- Update `embassy-sync` to version 0.6
- Update `esp-hal` to version 0.18
- Update `esp-backtrace` to version 0.12
- Add `esp-hal-embassy` version 0.1
- Use item-level granularity for imports
- Use stable Rust toolchain
- Replace `static mut` with `SyncUnsafeCell`
- Update `reqwless` to version 0.12
- Update other dependencies to their latest versions


## [0.1.0] - 2024-03-16

### Added

- Initial implementation

[Unreleased]: https://gitlab.com/claudiomattera/esp32c3-embassy
[0.1.0]: https://gitlab.com/claudiomattera/esp32c3-embassy/-/tags/0.1.0
[0.2.0]: https://gitlab.com/claudiomattera/esp32c3-embassy/-/tags/0.2.0
[0.3.0]: https://gitlab.com/claudiomattera/esp32c3-embassy/-/tags/0.3.0
[0.4.0]: https://gitlab.com/claudiomattera/esp32c3-embassy/-/tags/0.4.0
[0.5.0]: https://gitlab.com/claudiomattera/esp32c3-embassy/-/tags/0.5.0
[0.6.0]: https://gitlab.com/claudiomattera/esp32c3-embassy/-/tags/0.6.0
