# Release Process Guide

This document explains how to create new releases for Open Killer using the automated CI/CD pipeline.

## Automated Release System

The project uses GitHub Actions for **fully automated** building and releasing across Windows, macOS, and Linux platforms. When you merge code to the `main` branch, the system automatically:

1. Detects the type of changes from commit messages
2. Bumps the version accordingly (patch, minor, or major)
3. Builds installers for all platforms
4. Creates a GitHub release with downloadable installers

## How It Works

### Automatic Release on Merge (Recommended)

Every time you merge a Pull Request or push directly to `main`, the CI/CD pipeline automatically:

1. **Analyzes your commit messages** to determine version type:
   - `feat:` commits → **MINOR** version bump (1.0.0 → 1.1.0)
   - `fix:` or other commits → **PATCH** version bump (1.0.0 → 1.0.1)
   - `BREAKING CHANGE`, `feat!`, `fix!` → **MAJOR** version bump (1.0.0 → 2.0.0)

2. **Automatically updates** package.json with the new version

3. **Creates a git tag** (e.g., v1.0.1)

4. **Builds installers** for Windows, macOS, and Linux

5. **Publishes a GitHub release** with:
   - Automatic changelog from commits
   - Downloadable installers for all platforms

### Example Workflow

```bash
# Create a feature branch
git checkout -b feature/add-dark-mode

# Make your changes and commit with conventional commit format
git commit -m "feat: add dark mode toggle"

# Push and create a Pull Request
git push origin feature/add-dark-mode

# After PR is reviewed and merged to main:
# → CI/CD automatically detects "feat:" prefix
# → Bumps MINOR version (1.0.0 → 1.1.0)
# → Builds and releases v1.1.0
```

## Commit Message Format

Use **Conventional Commits** format for automatic version detection:

### New Features (MINOR bump)

```bash
git commit -m "feat: add user authentication"
git commit -m "feat: implement favorites system"
```

### Bug Fixes (PATCH bump)

```bash
git commit -m "fix: resolve memory leak in port scanner"
git commit -m "fix: correct typo in UI"
```

### Breaking Changes (MAJOR bump)

```bash
git commit -m "feat!: redesign API endpoints"
git commit -m "BREAKING CHANGE: remove deprecated features"
```

### Other Commits (PATCH bump)

```bash
git commit -m "chore: update dependencies"
git commit -m "docs: update README"
git commit -m "refactor: improve code structure"
```

## Manual Release Methods

### Method 1: Using GitHub Actions

If you need to create a release manually:

1. Go to the **Actions** tab in your GitHub repository
2. Select the **"Create New Version"** workflow
3. Click **"Run workflow"**
4. Choose the version bump type (patch/minor/major)

### Method 2: Using NPM Scripts

```bash
# For bug fixes (1.0.0 → 1.0.1)
npm run version:patch

# For new features (1.0.0 → 1.1.0)
npm run version:minor

# For breaking changes (1.0.0 → 2.0.0)
npm run version:major
```

## Build Outputs

Each release will include:

- **Windows**: `.exe` installer (NSIS)
- **macOS**: `.dmg` installer (Intel and Apple Silicon)
- **Linux**: `.AppImage` portable application

## Workflow Files

- [`.github/workflows/build-release.yml`](.github/workflows/build-release.yml) - Main build and release workflow
- [`.github/workflows/version-release.yml`](.github/workflows/version-release.yml) - Version bumping workflow

## Best Practices

1. **Write meaningful commit messages** - They appear in the changelog
2. **Test before releasing** - Use `npm run dev` to verify changes
3. **Use semantic versioning**:
   - MAJOR: Breaking changes
   - MINOR: New features (backward compatible)
   - PATCH: Bug fixes
4. **Review the generated release** - Check the changelog and artifacts before sharing

## First Release

To create your first release:

```bash
# Make sure all changes are committed
git add .
git commit -m "feat: initial release"

# Create the first version tag
npm run version:patch
```

Or use the GitHub Actions workflow to create v1.0.1.

## Troubleshooting

- **Build fails**: Check the Actions tab for error logs
- **Missing artifacts**: Ensure electron-builder is properly configured
- **Permission errors**: Verify GitHub token permissions in repository settings

## Changelog Format

The changelog is automatically generated from your git commit history. Use conventional commit format for best results:

- `feat:` - New features
- `fix:` - Bug fixes
- `chore:` - Maintenance tasks
- `docs:` - Documentation updates
- `refactor:` - Code refactoring
- `test:` - Test updates
