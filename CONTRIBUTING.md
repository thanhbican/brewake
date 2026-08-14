# Contributing to Brewake

Thanks for helping improve Brewake.

## Development

Use stable Rust on Windows with the MSVC target, then run:

```powershell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

The tray regression needs an interactive Windows desktop and is intentionally ignored by the
default test run. Execute it explicitly after changing the Win32 message loop or menus:

```powershell
cargo test --test windows_tray_smoke -- --ignored --nocapture
```

It opens the real native context menu, lets timer and control messages run inside the menu's nested
message loop, dismisses the menu, and verifies a clean process exit.

Keep changes focused. New runtime dependencies should have a clear user-facing benefit; this project deliberately avoids a UI framework, networking, telemetry, and elevation.

## Pull requests

- Explain the behavior change and why it belongs in Brewake.
- Add or update tests for state transitions and CLI parsing.
- Manually verify the tray menu, tooltip, suspend/resume, and clean exit for Win32 changes.
- Do not include generated `target` files or user configuration.

By contributing, you agree that your contribution is licensed under the MIT License.
