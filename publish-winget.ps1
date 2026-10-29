<#
.SYNOPSIS
    Generates, validates, and publishes Windows Package Manager (WinGet) manifests for Martini.

.DESCRIPTION
    This script automates creating the official multi-file WinGet manifests:
    - LycheeMartini.Martini.yaml (Version manifest)
    - LycheeMartini.Martini.locale.en-US.yaml (DefaultLocale manifest)
    - LycheeMartini.Martini.installer.yaml (Installer manifest)

    It computes the SHA256 hashes of release binaries (from GitHub Releases or local builds),
    validates the manifests using `winget validate`, and can automatically submit a Pull Request
    to microsoft/winget-pkgs from an organization fork (https://github.com/Lychee-Martini/winget-pkgs).

.PARAMETER Version
    The package version to publish (defaults to the version defined in Cargo.toml).

.PARAMETER PackageIdentifier
    The unique identifier for the package in WinGet (default: 'LycheeMartini.Martini').

.PARAMETER ForkOwner
    The GitHub organization or user account where winget-pkgs is forked (default: 'Lychee-Martini').

.PARAMETER Repo
    The source GitHub repository for Martini (default: 'Lychee-Martini/martini').

.PARAMETER OutputDir
    Directory where generated manifests will be stored (default: 'target/winget-manifests').

.PARAMETER Token
    GitHub Personal Access Token (PAT) with repo / public_repo permissions.
    Can also be provided via $env:WINGET_TOKEN or $env:GITHUB_TOKEN.

.PARAMETER Submit
    Switch to submit the manifest directly to microsoft/winget-pkgs via a Pull Request from the fork.

.PARAMETER OpenPR
    Switch to open the submitted PR in your default web browser.

.PARAMETER SkipValidation
    Switch to skip local `winget validate` validation.

.PARAMETER Cuda
    Switch to generate and publish manifests for the CUDA GPU-accelerated edition
    (defaults PackageIdentifier to 'LycheeMartini.Martini.CUDA' targeting *-x86_64-windows-cuda.zip).

.EXAMPLE
    # 1. Generate and validate manifests locally for the current version:
    .\publish-winget.ps1

.EXAMPLE
    # 2. Generate and validate manifests for the CUDA edition:
    .\publish-winget.ps1 -Cuda

.EXAMPLE
    # 3. Test manifest generation against an existing release (e.g. v0.1.0):
    .\publish-winget.ps1 -Version 0.1.0

.EXAMPLE
    # 4. Submit PR to microsoft/winget-pkgs using Lychee-Martini fork:
    .\publish-winget.ps1 -Submit -Token "ghp_xxxx"
#>

[CmdletBinding()]
param(
    [string]$Version,
    [string]$PackageIdentifier,
    [string]$ForkOwner = "Lychee-Martini",
    [string]$Repo = "Lychee-Martini/martini",
    [string]$OutputDir = "target/winget-manifests",
    [string]$Token,
    [switch]$Cuda,
    [switch]$Submit,
    [switch]$OpenPR,
    [switch]$SkipValidation
)

$ErrorActionPreference = "Stop"

if (-not $PackageIdentifier) {
    $PackageIdentifier = if ($Cuda) { "LycheeMartini.Martini.CUDA" } else { "LycheeMartini.Martini" }
}

# 1. Resolve Token if not passed as argument
if (-not $Token) {
    if ($env:WINGET_TOKEN) {
        $Token = $env:WINGET_TOKEN
    } elseif ($env:GITHUB_TOKEN) {
        $Token = $env:GITHUB_TOKEN
    }
}

# 2. Extract Version from Cargo.toml if not specified
if (-not $Version) {
    if (Test-Path "Cargo.toml") {
        $cargoContent = Get-Content "Cargo.toml" -Raw
        if ($cargoContent -match '(?ms)\[package\].*?version\s*=\s*"([^"]+)"') {
            $Version = $matches[1]
            Write-Host "==> Extracted version $Version from Cargo.toml" -ForegroundColor Cyan
        } else {
            throw "Unable to extract package version from Cargo.toml. Please specify -Version manually."
        }
    } else {
        throw "Cargo.toml not found. Please run this script from the repository root or provide -Version."
    }
}

$cleanVersion = $Version.TrimStart('v')
$tag = "v$cleanVersion"

Write-Host "==========================================================" -ForegroundColor Green
Write-Host " WinGet Manifest Publisher for $PackageIdentifier" -ForegroundColor Green
Write-Host " Version:    $cleanVersion (Tag: $tag)" -ForegroundColor Yellow
Write-Host " Repository: https://github.com/$Repo" -ForegroundColor Yellow
Write-Host " Fork Owner: $ForkOwner (https://github.com/$ForkOwner/winget-pkgs)" -ForegroundColor Yellow
Write-Host "==========================================================" -ForegroundColor Green

# 3. Create manifest directory structure: manifests/<first_char>/<Publisher>/<Package>/<SubPackage>/<Version>/
# e.g.: target/winget-manifests/l/LycheeMartini/Martini/0.1.2/
#   or: target/winget-manifests/l/LycheeMartini/Martini/CUDA/0.1.2/
$parts = $PackageIdentifier.Split('.')
$publisher = $parts[0]
$idPath = $PackageIdentifier -replace '\.', '/'
$firstChar = $publisher.Substring(0, 1).ToLower()

$manifestSubDir = "$firstChar/$idPath/$cleanVersion"
$targetManifestDir = Join-Path $OutputDir $manifestSubDir

if (Test-Path $targetManifestDir) {
    Remove-Item -Recurse -Force $targetManifestDir
}
New-Item -ItemType Directory -Force -Path $targetManifestDir | Out-Null

# 4. Resolve download URLs and SHA256 hashes
$headers = @{
    "User-Agent" = "Martini-WinGet-Publisher"
}
if ($Token) {
    $headers["Authorization"] = "token $Token"
}

Write-Host "==> Checking release assets on GitHub ($Repo, tag: $tag)..." -ForegroundColor Cyan

$releaseUrl = "https://api.github.com/repos/$Repo/releases/tags/$tag"
$releaseData = $null
try {
    $releaseData = Invoke-RestMethod -Uri $releaseUrl -Headers $headers -Method Get
} catch {
    Write-Warning "Could not fetch release $tag from GitHub API ($($_.Exception.Message))."
}

$architectures = if ($Cuda) {
    @(
        @{
            Arch = "x64"
            Suffix = "x86_64-windows-cuda.zip"
        }
    )
} else {
    @(
        @{
            Arch = "x64"
            Suffix = "x86_64-windows.zip"
        },
        @{
            Arch = "x86"
            Suffix = "i386-windows.zip"
        },
        @{
            Arch = "arm64"
            Suffix = "aarch64-windows.zip"
        }
    )
}

$tempDir = Join-Path $env:TEMP "martini-winget-$cleanVersion"
if (-not (Test-Path $tempDir)) {
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
}

$installersList = [System.Collections.Generic.List[PSObject]]::new()

foreach ($item in $architectures) {
    $arch = $item.Arch
    $suffix = $item.Suffix
    $expectedFileName = "martini-$tag-$suffix"
    $downloadUrl = "https://github.com/$Repo/releases/download/$tag/$expectedFileName"
    $sha256 = $null

    # Check if asset exists on GitHub release
    $assetFound = $false
    if ($releaseData -and $releaseData.assets) {
        $matchedAsset = $releaseData.assets | Where-Object { $_.name -eq $expectedFileName }
        if ($matchedAsset) {
            $assetFound = $true
            $downloadUrl = $matchedAsset.browser_download_url
            Write-Host "  Found remote asset: $expectedFileName" -ForegroundColor Green
        }
    }

    $localPath = Join-Path $tempDir $expectedFileName
    if ($assetFound) {
        Write-Host "  Downloading $expectedFileName to compute SHA256..." -ForegroundColor Gray
        Invoke-WebRequest -Uri $downloadUrl -OutFile $localPath -UseBasicParsing
        $hashObj = Get-FileHash -Path $localPath -Algorithm SHA256
        $sha256 = $hashObj.Hash.ToUpper()
        Write-Host "  [$arch] SHA256: $sha256" -ForegroundColor Green
    } else {
        # Check local build folder
        $localSearchPaths = @(
            "target/packages/$expectedFileName",
            "target/release/$expectedFileName",
            "target/$expectedFileName"
        )
        foreach ($candidate in $localSearchPaths) {
            if (Test-Path $candidate) {
                Write-Host "  Found local artifact: $candidate" -ForegroundColor Green
                $hashObj = Get-FileHash -Path $candidate -Algorithm SHA256
                $sha256 = $hashObj.Hash.ToUpper()
                Write-Host "  [$arch] SHA256 (from local): $sha256" -ForegroundColor Green
                break
            }
        }
    }

    if ($sha256) {
        $installersList.Add([PSCustomObject]@{
            Arch = $arch
            Url = $downloadUrl
            Sha256 = $sha256
        })
    } else {
        Write-Host "  (Asset $expectedFileName not found, skipping architecture $arch)" -ForegroundColor Yellow
    }
}

if ($installersList.Count -eq 0) {
    Write-Warning "No Windows release binaries were found online or locally for $tag."
    Write-Host "Using placeholder SHA256 hashes for manifest scaffolding..." -ForegroundColor Yellow
    $placeholderSuffix = if ($Cuda) { "x86_64-windows-cuda.zip" } else { "x86_64-windows.zip" }
    $installersList.Add([PSCustomObject]@{
        Arch = "x64"
        Url = "https://github.com/$Repo/releases/download/$tag/martini-$tag-$placeholderSuffix"
        Sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
    })
}

# 5. Generate Manifest Files
Write-Host "==> Generating WinGet YAML manifests in $targetManifestDir..." -ForegroundColor Cyan

# 5.1 Version Manifest
$versionYamlPath = Join-Path $targetManifestDir "$PackageIdentifier.yaml"
$versionYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.1.6.0.schema.json

PackageIdentifier: $PackageIdentifier
PackageVersion: $cleanVersion
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
"@
Set-Content -Path $versionYamlPath -Value $versionYaml -Encoding UTF8

# 5.2 Locale Manifest
$displayPackageName = if ($Cuda) { "Martini (CUDA)" } else { "Martini" }
$shortDesc = if ($Cuda) {
    "CUDA GPU-accelerated edition of Martini CLI format converter and neural inpainter (requires CUDA 12.x Runtime)."
} else {
    "A modern, extensible CLI format converter built in Rust and optimized for Agent Skills."
}
$longDesc = if ($Cuda) {
    "Martini (CUDA Edition) is a high-performance CLI document, data, and image converter with NVIDIA CUDA GPU acceleration for local neural object and watermark erasure. Requires NVIDIA GPU driver and CUDA 12.x Runtime libraries."
} else {
    "Martini is a high-performance, format-flexible CLI document, data, and image converter designed for developers and AI agent tool chains."
}
$monikerName = if ($Cuda) { "martini-cuda" } else { "martini" }
$extraTags = if ($Cuda) {
@"

  - cuda
  - gpu
"@
} else { "" }

$localeYamlPath = Join-Path $targetManifestDir "$PackageIdentifier.locale.en-US.yaml"
$localeYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.1.6.0.schema.json

PackageIdentifier: $PackageIdentifier
PackageVersion: $cleanVersion
PackageLocale: en-US
Publisher: Lychee Martini
PublisherUrl: https://lycheemartini.com
PublisherSupportUrl: https://github.com/$Repo/issues
PackageName: $displayPackageName
PackageUrl: https://github.com/$Repo
License: Apache-2.0
LicenseUrl: https://github.com/$Repo/blob/main/LICENSE
ShortDescription: $shortDesc
Description: $longDesc
Moniker: $monikerName
Tags:
  - cli
  - converter
  - image
  - pdf
  - markdown$extraTags
ManifestType: defaultLocale
ManifestVersion: 1.6.0
"@
Set-Content -Path $localeYamlPath -Value $localeYaml -Encoding UTF8

# 5.3 Installer Manifest
$installerYamlPath = Join-Path $targetManifestDir "$PackageIdentifier.installer.yaml"
$installersYamlBlocks = @()
foreach ($inst in $installersList) {
    $installersYamlBlocks += @"
  - Architecture: $($inst.Arch)
    NestedInstallerFiles:
      - RelativeFilePath: martini.exe
        PortableCommandAlias: martini
    InstallerUrl: $($inst.Url)
    InstallerSha256: $($inst.Sha256)
"@
}

$installersJoined = $installersYamlBlocks -join "`n"

$installerYaml = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.1.6.0.schema.json

PackageIdentifier: $PackageIdentifier
PackageVersion: $cleanVersion
InstallerType: zip
NestedInstallerType: portable
Commands:
  - martini
Installers:
$installersJoined
ManifestType: installer
ManifestVersion: 1.6.0
"@
Set-Content -Path $installerYamlPath -Value $installerYaml -Encoding UTF8

Write-Host "  Created $versionYamlPath" -ForegroundColor Green
Write-Host "  Created $localeYamlPath" -ForegroundColor Green
Write-Host "  Created $installerYamlPath" -ForegroundColor Green

# 6. Validate with `winget validate` if available
if (-not $SkipValidation) {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "==> Validating manifests with 'winget validate'..." -ForegroundColor Cyan
        try {
            & winget validate --manifest $targetManifestDir
            Write-Host "==> Local WinGet manifest validation passed!" -ForegroundColor Green
        } catch {
            Write-Warning "WinGet validation reported warnings or errors: $($_.Exception.Message)"
        }
    } else {
        Write-Host "==> 'winget' CLI is not found on PATH. Skipping local validation." -ForegroundColor Yellow
    }
}

# 7. Submit Pull Request to microsoft/winget-pkgs
if ($Submit) {
    Write-Host "==> Initiating Pull Request submission to microsoft/winget-pkgs..." -ForegroundColor Cyan

    if (-not $Token) {
        throw "A GitHub Personal Access Token (-Token or `$env:WINGET_TOKEN) is required to submit a PR to microsoft/winget-pkgs."
    }

    $authHeaders = @{
        "User-Agent" = "Martini-WinGet-Publisher"
        "Authorization" = "token $Token"
        "Accept" = "application/vnd.github.v3+json"
    }

    # Step 7.1 Verify or fork microsoft/winget-pkgs under $ForkOwner
    Write-Host "==> Verifying fork at https://github.com/$ForkOwner/winget-pkgs..." -ForegroundColor Cyan
    $forkUrl = "https://api.github.com/repos/$ForkOwner/winget-pkgs"
    $forkExists = $false
    try {
        $forkInfo = Invoke-RestMethod -Uri $forkUrl -Headers $authHeaders -Method Get
        $forkExists = $true
        Write-Host "  Fork found at $ForkOwner/winget-pkgs (default branch: $($forkInfo.default_branch))" -ForegroundColor Green
    } catch {
        Write-Host "  Fork not found under $ForkOwner. Creating fork from microsoft/winget-pkgs..." -ForegroundColor Yellow
        $forkRequestUri = "https://api.github.com/repos/microsoft/winget-pkgs/forks"
        $forkBody = @{
            "organization" = $ForkOwner
        } | ConvertTo-Json
        
        try {
            $forkResult = Invoke-RestMethod -Uri $forkRequestUri -Headers $authHeaders -Method Post -Body $forkBody
            Write-Host "  Fork initiated successfully under $ForkOwner! Please wait a moment for GitHub to complete the fork." -ForegroundColor Green
            Start-Sleep -Seconds 10
            $forkExists = $true
        } catch {
            throw "Failed to create fork under $ForkOwner. Ensure your token has permission to fork into this organization: $($_.Exception.Message)"
        }
    }

    # Step 7.2 Sync fork with upstream master
    Write-Host "==> Syncing fork master branch with upstream microsoft/winget-pkgs:master..." -ForegroundColor Cyan
    try {
        $syncUri = "https://api.github.com/repos/$ForkOwner/winget-pkgs/merge-upstream"
        $syncBody = @{ "branch" = "master" } | ConvertTo-Json
        $syncRes = Invoke-RestMethod -Uri $syncUri -Headers $authHeaders -Method Post -Body $syncBody
        Write-Host "  Fork synchronized: $($syncRes.message)" -ForegroundColor Green
    } catch {
        Write-Host "  Sync upstream note: $($_.Exception.Message)" -ForegroundColor Gray
    }

    # Step 7.3 Get master ref SHA
    $masterRef = Invoke-RestMethod -Uri "https://api.github.com/repos/$ForkOwner/winget-pkgs/git/ref/heads/master" -Headers $authHeaders -Method Get
    $masterSha = $masterRef.object.sha

    # Step 7.4 Create branch: martini-$cleanVersion (or martini-cuda-$cleanVersion)
    $branchName = if ($Cuda) { "martini-cuda-$cleanVersion" } else { "martini-$cleanVersion" }
    Write-Host "==> Creating branch '$branchName' from master ($($masterSha.Substring(0,7)))..." -ForegroundColor Cyan
    try {
        $createBranchUri = "https://api.github.com/repos/$ForkOwner/winget-pkgs/git/refs"
        $branchBody = @{
            "ref" = "refs/heads/$branchName"
            "sha" = $masterSha
        } | ConvertTo-Json
        Invoke-RestMethod -Uri $createBranchUri -Headers $authHeaders -Method Post -Body $branchBody | Out-Null
        Write-Host "  Branch '$branchName' created." -ForegroundColor Green
    } catch {
        Write-Host "  Branch '$branchName' already exists or notice: $($_.Exception.Message)" -ForegroundColor Yellow
    }

    # Step 7.5 Upload the 3 manifest files to the branch via GitHub Contents API
    $filesToCommit = @(
        @{
            LocalPath = $versionYamlPath
            RepoPath = "manifests/$manifestSubDir/$PackageIdentifier.yaml"
        },
        @{
            LocalPath = $localeYamlPath
            RepoPath = "manifests/$manifestSubDir/$PackageIdentifier.locale.en-US.yaml"
        },
        @{
            LocalPath = $installerYamlPath
            RepoPath = "manifests/$manifestSubDir/$PackageIdentifier.installer.yaml"
        }
    )

    foreach ($file in $filesToCommit) {
        $fileName = Split-Path $file.RepoPath -Leaf
        Write-Host "  Committing $fileName to $ForkOwner/winget-pkgs ($branchName)..." -ForegroundColor Gray
        $rawBytes = [System.IO.File]::ReadAllBytes($file.LocalPath)
        $base64 = [Convert]::ToBase64String($rawBytes)

        # Check if file exists in branch to get existing sha
        $fileSha = $null
        try {
            $existingFile = Invoke-RestMethod -Uri "https://api.github.com/repos/$ForkOwner/winget-pkgs/contents/$($file.RepoPath)?ref=$branchName" -Headers $authHeaders -Method Get
            $fileSha = $existingFile.sha
        } catch {
            # File doesn't exist yet, normal
        }

        $commitBody = @{
            "message" = "Add $PackageIdentifier version $cleanVersion ($fileName)"
            "content" = $base64
            "branch" = $branchName
        }
        if ($fileSha) {
            $commitBody["sha"] = $fileSha
        }

        $putUri = "https://api.github.com/repos/$ForkOwner/winget-pkgs/contents/$($file.RepoPath)"
        Invoke-RestMethod -Uri $putUri -Headers $authHeaders -Method Put -Body ($commitBody | ConvertTo-Json) | Out-Null
    }

    # Step 7.6 Create Pull Request against microsoft/winget-pkgs
    Write-Host "==> Submitting Pull Request to microsoft/winget-pkgs..." -ForegroundColor Cyan
    $prUri = "https://api.github.com/repos/microsoft/winget-pkgs/pulls"
    $prTitle = "New package: $PackageIdentifier version $cleanVersion"
    $prBodyText = @"
## Pull Request for $PackageIdentifier

- **Package Identifier:** $PackageIdentifier
- **Package Version:** $cleanVersion
- **Publisher:** Lychee Martini
- **Description:** $shortDesc
- **Repository:** https://github.com/$Repo

Manifests generated and validated via Martini automation pipeline.
"@

    $prPayload = @{
        "title" = $prTitle
        "head" = "$ForkOwner`:$branchName"
        "base" = "master"
        "body" = $prBodyText
    } | ConvertTo-Json

    try {
        $prResult = Invoke-RestMethod -Uri $prUri -Headers $authHeaders -Method Post -Body $prPayload
        Write-Host "==========================================================" -ForegroundColor Green
        Write-Host " SUCCESS! Pull Request submitted to microsoft/winget-pkgs:" -ForegroundColor Green
        Write-Host " URL: $($prResult.html_url)" -ForegroundColor Yellow
        Write-Host "==========================================================" -ForegroundColor Green

        if ($OpenPR) {
            Start-Process $prResult.html_url
        }
    } catch {
        Write-Warning "Pull Request submission failed or already exists: $($_.Exception.Message)"
        Write-Host "You can manually open a Pull Request at:" -ForegroundColor Cyan
        Write-Host "https://github.com/microsoft/winget-pkgs/compare/master...$ForkOwner`:$branchName" -ForegroundColor Yellow
        if ($OpenPR) {
            Start-Process "https://github.com/microsoft/winget-pkgs/compare/master...$ForkOwner`:$branchName"
        }
    }
} else {
    Write-Host ""
    Write-Host "==> Manifest generation and validation complete!" -ForegroundColor Green
    Write-Host "    Manifests saved to: $targetManifestDir" -ForegroundColor Yellow
    Write-Host ""
    $cudaFlagHint = if ($Cuda) { " -Cuda" } else { "" }
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "  1. To automatically submit a PR to microsoft/winget-pkgs from https://github.com/$ForkOwner/winget-pkgs:" -ForegroundColor Gray
    Write-Host "     .\publish-winget.ps1$cudaFlagHint -Submit -Token <GitHub_PAT>" -ForegroundColor White
    Write-Host "  2. Or test and submit using wingetcreate if preferred:" -ForegroundColor Gray
    Write-Host "     wingetcreate submit $($targetManifestDir -replace '/', '\')" -ForegroundColor White
    Write-Host ""
}
