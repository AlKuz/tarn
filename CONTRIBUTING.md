# Contributing

## Commit Messages

This project uses **Conventional Commits** to automate versioning and changelog generation.

### Format

```
<type>: <description>

[optional body]
```

### Types

| Type | When to use | Version bump |
|------|-------------|--------------|
| `fix` | Bug fixes | 0.1.0 → 0.1.1 (patch) |
| `feat` | New features | 0.1.0 → 0.2.0 (minor) |
| `feat!` | Breaking changes | 0.1.0 → 1.0.0 (major) |
| `docs` | Documentation only | No release |
| `chore` | Maintenance (deps, CI) | No release |
| `refactor` | Code changes without fixing bugs or adding features | No release |
| `test` | Adding or fixing tests | No release |

### Examples

```bash
# Bug fix (patch release)
git commit -m "fix: handle empty file paths"

# New feature (minor release)
git commit -m "feat: add HTTP transport support"

# Breaking change (major release)
git commit -m "feat!: rename TarnBuilder to Builder"

# With detailed description
git commit -m "feat: add file watching

Watches for changes in tracked directories and
automatically updates the index.

Closes #123"

# Non-release commits
git commit -m "docs: update installation instructions"
git commit -m "chore: update dependencies"
git commit -m "test: add integration tests for MCP"
```

## Release Process

Releases are fully automated:

1. **Write code** and commit using conventional commits
2. **Push to main** — release-please analyzes your commits
3. **Release PR appears** — it shows the new version and changelog
4. **Merge the PR** — this triggers:
   - Version bump in `Cargo.toml`
   - Git tag creation (e.g., `v0.2.0`)
   - GitHub Release with binaries for macOS, Linux, Windows
   - Auto-generated changelog

You don't need to manually edit versions or create tags.

## Development

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy
```

CI runs these checks on every pull request.
