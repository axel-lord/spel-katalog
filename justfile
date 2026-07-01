crate := "spel-katalog"
installer_crate := "spel-katalog-install"
daemon_crate := "spel-katalog-daemon"

default:
	just --list

# Generate documentation for default feature set.
docs *EXTRA:
	cargo doc -p {{crate}} {{EXTRA}}

# Generate documentation for default feature set.
docs-nightly *EXTRA:
	RUSTDOCFLAGS='--cfg=docsrs' cargo +nightly doc -p {{crate}} {{EXTRA}}

# Generate documentation for all features.
docs-nightly-all *EXTRA:
	RUSTDOCFLAGS='--cfg=docsrs' cargo +nightly doc --all-features -p {{crate}} {{EXTRA}}

# Generate documentation for minimal feature set.
docs-min *EXTRA:
	cargo doc --no-default-features -p {{crate}} {{EXTRA}}

# Run all tests with all features.
test-all *EXTRA:
	cargo test --all --all-features {{EXTRA}}

# Run tests with all features.
test *EXTRA:
	cargo test --all-features {{EXTRA}}

# Run tests using miri
test-miri *EXTRA:
	cargo miri test {{EXTRA}}

# Format crates.
fmt:
	cargo fmt --all

# Check all features and targets
check:
	cargo clippy --all --all-features --all-targets --workspace

# Run autoinherit
autoinherit:
	cargo autoinherit --prefer-simple-dotted

# Sanity and format check
sanity: autoinherit fmt test-all

# Install a crate by path.
install-crate CRATE:
	cargo +nightly install --path {{CRATE}} -Z build-std=std,panic_abort -Z build-std-features="optimize_for_size"

# Build crate by name.
build-crate CRATE *EXTRA:
	cargo +nightly build --release -p {{CRATE}} -Z build-std=std,panic_abort -Z build-std-features="optimize_for_size" {{EXTRA}}

# Run a crate by name.
run-crate CRATE *EXTRA:
	cargo +nightly run --release -p {{CRATE}} -Z build-std=std,panic_abort -Z build-std-features="optimize_for_size" {{EXTRA}}

install-daemon: (build-crate daemon_crate)
	mkdir -p $XDG_DATA_HOME/spel-katalog
	cp target/release/spel-katalog-daemon $XDG_DATA_HOME/spel-katalog/

# Install project.
install: autoinherit fmt (install-crate crate) (install-crate installer_crate) install-daemon

# Build project.
build *EXTRA: autoinherit fmt (build-crate crate EXTRA)

# Run project.
run *EXTRA: autoinherit fmt (run-crate crate EXTRA)

# Run project with profiling.
profile *EXTRA:
	cargo run -p {{crate}} -F profiling

# Build match num test.
build-match-num *EXTRA:
	cargo build --release -p spel-katalog-test --bin match-num

# Build match num test as an exe.
build-match-num-exe *EXTRA:
	cargo build --release -p spel-katalog-test --bin match-num --target=x86_64-pc-windows-gnu
