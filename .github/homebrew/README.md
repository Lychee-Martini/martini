# Homebrew Tap Setup for Martini

This directory contains the formula template and setup guide for distributing Martini via Homebrew.

## Maintainer One-Time Setup

1. Create a public repository named `homebrew-tap` under the `Lychee-Martini` GitHub organization:
   - Repository URL: `https://github.com/Lychee-Martini/homebrew-tap`
2. In the `homebrew-tap` repository, create the directory structure:
   ```text
   Formula/
     martini.rb
   ```
3. Copy `martini.rb.template` into `Formula/martini.rb` in that repository and populate it with initial release checksums.
4. Create a GitHub Personal Access Token (PAT) with `repo` write permissions (or fine-grained token with contents:write for `homebrew-tap`).
5. In the `martini` repository (`Lychee-Martini/martini`), add the secret:
   - Name: `HOMEBREW_TAP_TOKEN`
   - Value: `<your-pat-token>`

## Automated Updates

When a new version tag (e.g. `v0.1.1`) is published, the `release.yml` GitHub Actions workflow will automatically:
1. Download release archives for macOS (`x86_64`, `aarch64`) and Linux (`x86_64`, `aarch64`).
2. Calculate their SHA256 checksums.
3. Render the updated formula from `martini.rb.template`.
4. Commit and push the updated `Formula/martini.rb` directly to `Lychee-Martini/homebrew-tap`.

## User Installation

Users install Martini via Homebrew with:

```bash
brew tap Lychee-Martini/tap
brew install martini
```

Or in a single command:

```bash
brew install Lychee-Martini/tap/martini
```
