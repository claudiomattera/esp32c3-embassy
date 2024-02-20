Contribution Guidelines
====

Repository
----

Code is organized in a [git] repository.
It adheres to [Semantic Versioning], makes use of [Conventional Commits], and changes are recorded in file [`CHANGELOG.md`](./CHANGELOG.md) according to the format [Keep a Changelog].

[git]: https://git-scm.com/
[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Conventional Commits]: https://www.conventionalcommits.org/en/v1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html


Build
----

This project is implemented in Rust and uses [just] as a task runner.
Install it with the following command.

~~~~shell
cargo install just
~~~~

All available tasks are defined inside file [`justfile`](./justfile), and their names and descriptions should be self-explanatory.

~~~~shell
# List available tasks
just

# Check source code format
just check-format

# Check source code best practices
just lint

# Build the project
just build

# Build and run tests
just test

# Build the project in release mode
just build-release

# Flash the device in release mode and attach a serial monitor
just run-release

# Generate source code documentation
just build-documentation

# Audit dependencies
just audit
~~~~

Two environment variables `WIFI_SSID` and `WIFI_PASSWORD` containing the credentials for connecting to WiFi must be defined and exported before building.

~~~~shell
export WIFI_SSID="..."
export WIFI_PASSWORD="..."

# or, better

read WIFI_SSID
> [ENTER VALUE]
read -s WIFI_PASSWORD
> [ENTER VALUE]
export WIFI_SSID
export WIFI_PASSWORD

just build
~~~~

[just]: https://just.systems/
