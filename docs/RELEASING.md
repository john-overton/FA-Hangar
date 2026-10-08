# Releasing

CI does not run on push or pull request. It runs on demand (Actions, Run
workflow) and when a GitHub release is published.

1. Bump `version` under `[workspace.package]` in the root `Cargo.toml`, run
   `cargo build --locked` once so `Cargo.lock` follows, and bump the version
   badge in `README.md`.
2. Move `docs/CHANGELOG.md`'s "Unreleased" entries under the new version
   heading, and record what was verified in `docs/VALIDATION.md`.
3. Run the checks in `AGENTS.md` (fmt, clippy, tests, smoke test, both Windows
   builds and `tools/check_pe.py`), then commit and push to `main`.
4. Publish a GitHub release on that commit with the tag `v<version>`, for
   example `v0.9.0`. The tag must equal `v` plus the `Cargo.toml` version; the
   workflow fails before building otherwise. Delete the release and tag, fix
   the version and publish again.
5. The workflow checks, builds and audits both executables, then attaches
   `fa-hangar-<version>-win98-me-pentium4.zip` and
   `fa-hangar-<version>-win64.zip` to the release. Each ZIP unpacks into a
   folder holding `fa-hangar-<version>.exe`, `SHA256.txt` (sha256sum format)
   and the documentation.

A manual run (no release) builds the same packages as workflow artifacts named
`fa-hangar-<version>-<variant>` with the current `Cargo.toml` version and
uploads nothing to a release.
