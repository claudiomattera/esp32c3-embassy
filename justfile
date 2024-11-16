
set dotenv-load := true

# Print available recipes
default:
    @just --list

[private]
cargo +args:
    cargo {{args}}
#     cargo +nightly {{args}}

# Generate Cargo.lock
generate-lockfile:
    @just cargo generate-lockfile --offline

# Update Cargo.lock
update-lockfile:
    @just cargo update

# Fetch dependencies
fetch:
    @just cargo fetch --locked

# Check source code format
check-format: fetch
    @just cargo fmt --all -- --check

# Enforce source code format
format: fetch
    @just cargo fmt --all

# Type-check source code
check +args='': fetch
    @just cargo check --frozen {{args}}

# Type-check source code for all feature combinations
check-all-feature-combinations: fetch
    @just cargo hack --feature-powerset --no-dev-deps check

# Check lints with Clippy
lint +args='': (check args)
    @just cargo clippy --frozen {{args}}

# Check lints with Clippy for all feature combinations
lint-all-feature-combinations: (check-all-feature-combinations)
    @just cargo hack --feature-powerset --no-dev-deps clippy

# Build debug
build +args='': fetch
    @just cargo build --frozen {{args}}

# Build release
build-release +args='': fetch
    @just cargo build --frozen --release {{args}}

# Build for all feature combinations
build-all-feature-combinations: (check-all-feature-combinations)
    @just cargo hack --feature-powerset --no-dev-deps build

# Build tests
build-tests +args='': fetch
    @just cargo test --target=x86_64-unknown-linux-gnu --frozen {{args}} --no-run

# Build tests for all feature combinations
build-tests-all-feature-combinations: (build-all-feature-combinations)
    @just cargo hack --feature-powerset test --target=x86_64-unknown-linux-gnu --no-run

# Run tests
test +args='': (build-tests args)
    @just cargo test --target=x86_64-unknown-linux-gnu --frozen {{args}}

# Run tests for all feature combinations
test-all-feature-combinations: (build-tests-all-feature-combinations)
    @just cargo hack --feature-powerset test --target=x86_64-unknown-linux-gnu

# Run release
run-release *args: (build-release args)
    @just cargo run --frozen  --release {{ args }}

# Build documentation
build-documentation +args='': fetch
    @just cargo doc --frozen --document-private-items {{args}}

# Clean
clean:
    @just cargo clean

# Audit dependencies
audit:
    @just cargo audit --deny unsound --deny yanked

# Publish to crates.io
publish:
    @just cargo login "${CRATES_IO_TOKEN}"
    @just cargo publish
    @just cargo logout
