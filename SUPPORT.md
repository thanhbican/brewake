# Brewake support

## Before opening an issue

1. Install the latest release from the official GitHub Releases page.
2. Confirm Windows 10/11 x64 and note the exact version from `winver`.
3. Run `brewakectl status --json` and save the output.
4. Try exiting Brewake cleanly and starting it again.
5. Check existing issues for the same behavior.

## Bug reports

Use the bug report template and include:

- Brewake version and download source.
- Windows edition, version, and architecture.
- Exact steps that reproduce the issue.
- Expected and actual behavior.
- `brewakectl status --json` output, with any personal paths removed.
- Relevant power-management or organization policies.

Do not post passwords, access tokens, private paths, or other sensitive data. For vulnerabilities, follow `SECURITY.md` instead of opening a public issue.

## Feature requests

Describe the user problem first, then the proposed behavior. Brewake intentionally stays native, small, local-only, non-elevated, and free of telemetry, so proposals should preserve those boundaries.
