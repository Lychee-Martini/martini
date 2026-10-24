# Multi-Channel Distribution & Installation Guide

Martini is distributed through multiple package managers and binary formats to provide a seamless installation experience across macOS, Linux, and Windows.

---

## 🚀 User Installation Guide

### 1. Homebrew (macOS & Linux)

For macOS (Apple Silicon and Intel) as well as Linux:

```bash
# Add the Lychee-Martini tap and install
brew tap Lychee-Martini/tap
brew install martini

# Or in a single command
brew install Lychee-Martini/tap/martini
```

### 2. Windows Package Manager (WinGet)

For Windows 10/11:

```powershell
winget install LycheeMartini.Martini
```

### 3. Cargo (crates.io)

Ensure you have Rust and Cargo installed:

```bash
cargo install martini-cli
```

> **Note:** The package name on crates.io is `martini-cli`, and the installed binary is `martini`.

### 4. Debian / Ubuntu (APT `.deb`)

Download the latest `.deb` package from [GitHub Releases](https://github.com/Lychee-Martini/martini/releases):

```bash
# Example for x86_64
sudo apt install ./martini_0.1.1-1_amd64.deb
```

### 5. Fedora / RHEL / CentOS (YUM / DNF `.rpm`)

Download the latest `.rpm` package from [GitHub Releases](https://github.com/Lychee-Martini/martini/releases):

```bash
# Example for x86_64
sudo dnf install ./martini-0.1.1-1.x86_64.rpm
```

### 6. Pre-compiled Archives (GitHub Releases)

Pre-compiled standalone binaries with zero system dependencies are available for:
- **Linux**: `x86_64-linux-gnu`, `x86_64-linux-musl`, `aarch64-linux-gnu`, `aarch64-linux-musl`
- **macOS**: `x86_64-macos` (Intel), `aarch64-macos` (Apple Silicon)
- **Windows**: `x86_64-windows`, `i386-windows`, `aarch64-windows`

Extract the archive and move the `martini` (or `martini.exe`) binary to any directory within your system `PATH`.

---

## 🛠️ Maintainer Configuration & Secrets Guide

Automated releases are triggered whenever a version tag matching `v*` (e.g. `v0.1.1`) is pushed to GitHub. The GitHub Actions release workflow handles compiling, packaging, and publishing.

### Required Repository Secrets

Configure the following secrets in **Settings > Secrets and variables > Actions**:

| Secret Name | Purpose | How to Generate |
|-------------|---------|-----------------|
| `CARGO_REGISTRY_TOKEN` | Publishes `martini-cli` to [crates.io](https://crates.io/) | [crates.io Account Settings > API Tokens](https://crates.io/settings/tokens). Generate a token with `publish-update` or `publish-new` permissions. |
| `HOMEBREW_TAP_TOKEN` | Bumps `Formula/martini.rb` in `Lychee-Martini/homebrew-tap` | [GitHub Personal Access Token](https://github.com/settings/tokens) with `repo` or fine-grained `contents:write` permission on the `homebrew-tap` repository. |
| `WINGET_TOKEN` | Submits automated manifest PRs to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) | [GitHub Personal Access Token](https://github.com/settings/tokens) with `public_repo` permission to fork and submit PRs. |

### Release Flow & Fallback Behavior

1. **Compilation & Archives**: `upload-assets` cross-compiles all 9 platform targets and attaches `.tar.gz` and `.zip` archives.
2. **Native Packages**: `package-linux` builds `.deb` and `.rpm` packages statically linked with musl and attaches them to the release.
3. **Draft Release Publication**: `publish-release` turns the draft release into an active public release.
4. **Channel Distribution**:
   - `publish-cargo` runs `cargo publish`.
   - `publish-homebrew` computes SHA256 hashes of the published tarballs and pushes the updated formula.
   - `publish-winget` opens a manifest update PR against `microsoft/winget-pkgs`.
5. **Graceful Fallbacks**: If any of `CARGO_REGISTRY_TOKEN`, `HOMEBREW_TAP_TOKEN`, or `WINGET_TOKEN` are not yet configured, the job emits an informative warning instead of failing the workflow.
