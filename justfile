crate := "spel-katalog"

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
test  *EXTRA:
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
sanity: autoinherit fmt test
