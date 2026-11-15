# Contributing

Use stable Rust, safe Rust by default, typed errors/configuration and small complete changes. Preserve input source and independent test expectations. Add an actual migration fixture before claiming a new language/bridge, and mock provider responses for normal CI. Live paid-model tests must be opt-in.

Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace`. For native end-to-end tests, first fetch `examples/python-native/Cargo.toml` with `--locked`, then run the ignored engine integration tests. Keep the response JSON in sync with the complete example Rust implementation.

Document observable behavior, evidence, limitations and setup requirements. A predicted gain is an estimate; a benchmark result must include workload and variability. Do not remove features/tests to make migrations pass, introduce fake results, commit credentials, or push/publish without authorization.
