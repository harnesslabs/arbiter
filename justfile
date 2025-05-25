default:
    @just --list

[private]
warn := "\\033[33m"
error := "\\033[31m"
info := "\\033[34m"
success := "\\033[32m"
reset := "\\033[0m"
bold := "\\033[1m"

# Print formatted headers without shell scripts
[private]
header msg:
    @printf "{{info}}{{bold}}==> {{msg}}{{reset}}\n"

# Install cargo tools
install-cargo-tools:
    @just header "Installing Cargo tools"
    # cargo-udeps
    if ! command -v cargo-udeps > /dev/null; then \
        printf "{{info}}Installing cargo-udeps...{{reset}}\n" && \
        cargo install cargo-udeps --locked; \
    else \
        printf "{{success}}✓ cargo-udeps already installed{{reset}}\n"; \
    fi
    # cargo-semver-checks
    if ! command -v cargo-semver-checks > /dev/null; then \
        printf "{{info}}Installing cargo-semver-checks...{{reset}}\n" && \
        cargo install cargo-semver-checks; \
    else \
        printf "{{success}}✓ cargo-semver-checks already installed{{reset}}\n"; \
    fi
    # taplo
    if ! command -v taplo > /dev/null; then \
        printf "{{info}}Installing taplo...{{reset}}\n" && \
        cargo install taplo-cli; \
    else \
        printf "{{success}}✓ taplo already installed{{reset}}\n"; \
    fi

# Install mdbook and plugins
install-mdbook-tools:
    @just header "Installing mdbook and plugins"
    if ! command -v mdbook > /dev/null; then \
        printf "{{info}}Installing mdbook...{{reset}}\n" && \
        cargo install mdbook; \
    else \
        printf "{{success}}✓ mdbook already installed{{reset}}\n"; \
    fi
    if ! command -v mdbook-linkcheck > /dev/null; then \
        printf "{{info}}Installing mdbook-linkcheck...{{reset}}\n" && \
        cargo install mdbook-linkcheck; \
    else \
        printf "{{success}}✓ mdbook-linkcheck already installed{{reset}}\n"; \
    fi
    if ! command -v mdbook-katex > /dev/null; then \
        printf "{{info}}Installing mdbook-katex...{{reset}}\n" && \
        cargo install mdbook-katex; \
    else \
        printf "{{success}}✓ mdbook-katex already installed{{reset}}\n"; \
    fi

# Install nightly rust
install-rust-nightly:
    @just header "Installing Rust nightly"
    rustup install nightly

# Setup complete development environment
setup: install-cargo-tools install-rust-nightly install-mdbook-tools
    @printf "{{success}}{{bold}}Development environment setup complete!{{reset}}\n"

# Check the with local OS target
check:
    @just header "Building workspace"
    cargo build --workspace --all-targets

# Build with local OS target
build:
    @just header "Building workspace"
    cargo build --workspace --all-targets

# Build with local OS target
build-wasm:
    @just header "Building workspace"
    cargo build --workspace --all-targets --target wasm32-unknown-unknown

# Run the tests on your local OS
test:
    @just header "Running main test suite"
    cargo test --workspace --all-targets --all-features
    @just header "Running doc tests"
    cargo test --workspace --doc

# Run clippy for the workspace on your local OS
lint:
    @just header "Running clippy"
    cargo clippy --workspace --all-targets --all-features

# Run clippy for the workspace on WASM
lint-wasm:
    @just header "Running clippy"
    cargo clippy --workspace --all-targets --all-features --target wasm32-unknown-unknown

# Check for semantic versioning for workspace crates
semver:
    @just header "Checking semver compatibility"
    cargo semver-checks check-release --workspace

# Run format for the workspace
fmt:
    @just header "Formatting code"
    cargo fmt --all
    taplo fmt

# Check for unused dependencies in the workspace
udeps:
    @just header "Checking unused dependencies"
    cargo +nightly udeps --workspace

# Run cargo clean to remove build artifacts
clean:
    @just header "Cleaning build artifacts"
    cargo clean

# Serve the mdbook documentation (with live reload)
book:
    @just header "Serving mdbook documentation"
    mdbook serve

book-check:
    @just header "Checking mdbook documentation"
    mdbook build

# Open cargo docs in browser
docs:
    @just header "Building and opening cargo docs"
    cargo doc --workspace --no-deps --open

doc-check:
    @just header "Checking cargo docs"
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Show your relevant environment information
info:
    @just header "Environment Information"
    @printf "{{info}}OS:{{reset}} %s\n" "$(uname -s)"
    @printf "{{info}}Rust:{{reset}} %s\n" "$(rustc --version)"
    @printf "{{info}}Cargo:{{reset}} %s\n" "$(cargo --version)"
    @printf "{{info}}Installed targets:{{reset}}\n"
    @rustup target list --installed | sed 's/^/  /'

# Run all possible CI checks (cannot test a non-local OS target!)
ci:
    @printf "{{bold}}Starting CI checks{{reset}}\n\n"
    @ERROR=0; \
    just run-single-check "Rust formatting" "cargo fmt --all -- --check" || ERROR=1; \
    just run-single-check "TOML formatting" "taplo fmt --check" || ERROR=1; \
    just run-single-check "Check" "cargo check --workspace" || ERROR=1; \
    just run-single-check "Clippy" "cargo clippy --workspace --all-targets --all-features -- --deny warnings" || ERROR=1; \
    just run-single-check "Test suite" "cargo test --verbose --workspace" || ERROR=1; \
    just run-single-check "Doc check" "RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps --all-features" || ERROR=1; \
    just run-single-check "Unused dependencies" "cargo +nightly udeps --workspace" || ERROR=1; \
    just run-single-check "Semver compatibility" "cargo semver-checks check-release --workspace" || ERROR=1; \
    printf "\n{{bold}}CI Summary:{{reset}}\n"; \
    if [ $ERROR -eq 0 ]; then \
        printf "{{success}}{{bold}}All checks passed successfully!{{reset}}\n"; \
    else \
        printf "{{error}}{{bold}}Some checks failed. See output above for details.{{reset}}\n"; \
        exit 1; \
    fi

# Run a single check and return status (0 = pass, 1 = fail)
[private]
run-single-check name command:
    #!/usr/bin/env sh
    printf "{{info}}{{bold}}Running{{reset}} {{info}}%s{{reset}}...\n" "{{name}}"
    if {{command}} > /tmp/check-output 2>&1; then
        printf "  {{success}}{{bold}}PASSED{{reset}}\n"
        exit 0
    else
        printf "  {{error}}{{bold}}FAILED{{reset}}\n"
        printf "{{error}}----------------------------------------\n"
        while IFS= read -r line; do
            printf "{{error}}%s{{reset}}\n" "$line"
        done < /tmp/check-output
        printf "{{error}}----------------------------------------{{reset}}\n"
        exit 1
    fi

# Success summary (called if all checks pass)
[private]
_ci-summary-success:
    @printf "\n{{bold}}CI Summary:{{reset}}\n"
    @printf "{{success}}{{bold}}All checks passed successfully!{{reset}}\n"

# Failure summary (called if any check fails)
[private]
_ci-summary-failure:
    @printf "\n{{bold}}CI Summary:{{reset}}\n"
    @printf "{{error}}{{bold}}Some checks failed. See output above for details.{{reset}}\n"
    @exit 1


