## Summary

Describe the user-visible change and why it belongs in Brewake.

## Verification

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`
- [ ] `cargo test --all-targets --locked`
- [ ] I manually tested affected tray or power behavior on Windows, if applicable.

## Project boundaries

- [ ] No unrelated changes or generated `target` files are included.
- [ ] No network access, telemetry, elevation, or power-plan mutation was introduced.
- [ ] Documentation and changelog were updated when user behavior changed.
