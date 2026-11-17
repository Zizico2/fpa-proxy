verify:
    rtk cargo test --workspace --locked
    rtk cargo fmt --check
    rtk cargo clippy --workspace --all-targets --locked -- -D warnings

run:
    rtk cargo run --locked
