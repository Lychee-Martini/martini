# Martini 🍸

A modern, extensible, and high-performance CLI format converter written in Rust. Designed specifically to be robust for human developers and seamlessly integrable with AI Agent workflows (Agent Skills).

## Features

- 🎨 **Pure Rust SVG rendering engine**: Powered by `resvg` and `tiny-skia`. Self-contained and fully portable across Windows, macOS, and Linux without native system dependencies (like Cairo or Glib).
- 🖼️ **Unified Image Conversion**: Supports converting between SVG, PNG, JPG/JPEG, WebP, AVIF, BMP, TIFF, and QOI in any combination.
- 📦 **Favicon generation from any source**: Converts **both** SVG vector files (rendered dynamically at multiple resolutions) and standard raster images (downscaled using Lanczos3 filtering) to a favicon:
  - **Default**: Generates a standard multi-resolution `favicon.ico` containing `16x16`, `32x32`, and `48x48` dimensions.
  - **Package Mode (`--package`)**: Generates a comprehensive modern suite of assets including `favicon.ico`, PNGs (16x16, 32x32, 180x180, 192x192, 512x512), `site.webmanifest`, and copy-pasteable HTML header tags.
- 📊 **Structured Data Formats Transcoding (`DataEngine`)**: Fast, pure-Rust bidirectional conversion between JSON, YAML, TOML, and tabular CSV / TSV formats with automatic column schema union.
- 🗜️ **Archive Compression & Extraction (`ArchiveEngine`)**: Convert between ZIP, TAR, and TAR.GZ directly without external tools; extract archives into folders or compress directories into archive formats with path traversal sanitization.
- ✨ **Smart Object & Watermark Erasure (`martini erase`)**: Erase unwanted objects, text, watermarks, or persons from images using natural language prompts (`--target text`) or custom binary masks. Runs entirely locally and offline on CPU with adaptive edge dilation and feathering.
- ⚙️ **Conversion Presets System**: Store and reuse conversion preferences in `martini-presets.toml` or user global configuration. Inspect available presets with `martini preset list` and `martini preset show <name>`.
- 👁️ **Directory Watching Daemon (`martini watch`)**: Automated headless inbox/outbox file monitoring daemon with settle debouncing, anti-loop output protection, backlog initial scanning, and customizable original file lifecycle (`keep`, `delete`, `move_originals`).
- 🏎️ **Concurrently Optimized Batch Processing**: Process a single file or a whole directory concurrently using a `rayon` thread pool. Includes customizable worker threads and directory recursion (`-r`).
- 📝 **Markdown to PDF & Word (DOCX)**:
  - **Pure Rust Markdown to PDF**: Powered by an embedded `typst` typesetting engine with zero external dependencies. Supports auto-detection of CJK system fonts (Microsoft YaHei, PingFang SC, Noto Sans CJK), customizable themes (`github`, `academic`, `modern`, `minimal`), paper sizes (A4, Letter, A3, A5), margins, headers/footers, page numbers, automatic table of contents (`--toc`), syntax-highlighted code blocks, GFM tables, and LaTeX math formulas.
  - **Markdown to DOCX**: Convert Markdown files directly to Microsoft Word (`.docx`) files natively.
- ⚡ **Developer-Friendly & Auto-Detecting CLI**:
  - Make `--from` optional (defaults to `"auto"` detecting from source extensions).
  - Make `--to` optional (detects target format from `--output` extension, or defaults to `pdf` for Markdown, `favicon` for SVGs, and `webp` for other images).
  - Make `--output` optional (omitted outputs are generated in-place next to source files).
  - Support deleting source files after successful conversion (`-d` / `--delete-original`).
- 🤖 **Agent Skill Ready**:
  - **JSON Mode (`--json`)**: Command results and errors are output in structured, machine-readable JSON.
  - **Self-Discovery (`list-formats`)**: A structured command returning the capabilities schema so agents can learn available format pairs dynamically.
  - **Structured Exit Codes**: Granular, typed exit codes map to distinct failure profiles.
  - **Level-controlled Tracing**: Logs are separated to `stderr` and can be adjusted with `--quiet` or `--verbose`.

---

## Installation

### ⚡ Quick Install (Recommended)

Single-command standalone installation powered by `cargo-dist` (no dependencies required):

#### macOS & Linux (Shell)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Lychee-Martini/martini/releases/latest/download/martini-installer.sh | sh
```

#### Windows (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Lychee-Martini/martini/releases/latest/download/martini-installer.ps1 | iex"
```

---

### Package Managers

#### Homebrew (macOS & Linux)

```bash
brew install Lychee-Martini/tap/martini
```

#### Windows (WinGet)

```powershell
winget install LycheeMartini.Martini
```

#### Cargo (crates.io)

```bash
cargo install martini-cli
```

#### Linux Packages (Debian / Ubuntu & Fedora / RHEL)

Download `.deb` (Debian/Ubuntu) or `.rpm` (Fedora/RHEL) from [GitHub Releases](https://github.com/Lychee-Martini/martini/releases):

```bash
# Debian / Ubuntu (APT)
sudo apt install ./martini_*.deb

# Fedora / RHEL / CentOS (YUM / DNF)
sudo dnf install ./martini-*.rpm
```

### Pre-compiled Standalone Binaries

Download portable single-file binaries from [GitHub Releases](https://github.com/Lychee-Martini/martini/releases) (supports Linux musl/gnu, macOS Apple Silicon/Intel, Windows x64/ARM64).

### Build from Source

Ensure you have Rust installed (1.85+):

```bash
cargo install --path .
# Or manual release build
cargo build --release
```

For more details on package managers and secrets, see [Distribution Guide](docs/distribution.md).

---

## CLI Usage

### 1. View Supported Formats

Retrieve supported format pairs and metadata:

```bash
martini list-formats
```

Output:

```text
Supported Conversions:
- [any] -> favicon: Convert an SVG or raster image to a Chrome favicon (.ico or full favicon package). Options: --package
- [any] -> png: Convert images to PNG format
- [any] -> jpg: Convert images to JPEG format. Options: --quality
- [any] -> webp: Convert images to WebP format. Options: --quality, --lossless
- [any] -> avif: Convert images to AVIF format. Options: --quality, --lossless
- [any] -> both: Convert images to both WebP and AVIF formats. Options: --quality, --lossless
- [any] -> bmp: Convert images to BMP format
- [any] -> tiff: Convert images to TIFF format
- [any] -> qoi: Convert images to QOI format
- pdf -> [png/jpg/webp/avif]: Convert PDF pages to images. Options: --pages, --dpi, --quality, --lossless
- md -> docx: Convert Markdown documents to Microsoft Word (DOCX) files. Options: --overwrite
- md -> pdf: Convert Markdown documents to PDF files with Pure Rust engine. Options: --theme, --paper-size, --margin, --font-family, --header, --footer, --no-page-numbers, --toc, --overwrite
- [json/yaml/toml/csv/tsv] -> [json/yaml/toml/csv/tsv]: Convert structured data between JSON, YAML, TOML, CSV, and TSV formats. Options: --overwrite
- [zip/tar/gz/tar.gz] -> [zip/tar/gz/tar.gz]: Convert compressed archives between ZIP, TAR, and TAR.GZ formats. Options: --overwrite
```

Or structured JSON for Agents:

```bash
martini list-formats --json
```

### 2. Convert Single Files (Simplified Syntax)

Martini automatically detects source and target formats from file extensions. Both concise positional arguments (`martini convert <input> [output]`) and named flags (`-i`, `-o`) are supported. Furthermore, `convert` is the default subcommand:

```bash
# Convert PDF to PNG images (default 150 DPI, outputs test_page_1.png, ...)
martini convert test.pdf
# Or even shorter (convert is the default command):
martini test.pdf

# Convert SVG to PNG using positional arguments
martini convert logo.svg logo.png
# Or directly:
martini logo.svg logo.png

# Convert JPG to WebP in-place (defaults to webp target and in-place output)
martini photo.jpg

# Named flags syntax (-i / -o) is also 100% supported
martini convert -i logo.svg -o logo.png
martini convert -i logo.png -o ./icons_dir --package
```

### 3. Batch Convert a Directory

Concurrently convert all images or documents in a folder:

```bash
# Convert all PNG/JPG images in the directory to WebP
martini convert -i ./images --to webp

# Convert all Markdown files in a folder to PDF
martini convert -i ./docs --from md --to pdf -o ./output_pdfs

# Recursively convert all images in a folder to WebP & AVIF, and delete the original files upon success
martini convert -i ./images --to both -r --delete-original
```

### 4. Convert PDF to Images

Extract pages of a PDF document to raster image formats (PNG, JPEG, WebP, AVIF) at custom resolutions:

```bash
# Convert all pages of a PDF to PNG at default 150 DPI
martini convert -i report.pdf -o report.png

# Convert specific pages (e.g. 1, 3 to 5) to JPEG at high resolution (300 DPI)
martini convert -i doc.pdf -o output.jpg --pages "1,3-5" --dpi 300

# Batch convert a folder of PDFs to WebP
martini convert -i ./pdfs -o ./images --from pdf --to webp
```

*Note: Output files are named with page suffixes, e.g., `output_page_1.jpg`, `output_page_2.jpg`.*

### 5. Image Resizing

Resize standard images, SVG vector images, or PDF pages on conversion:

```bash
# Resize PNG to width 400 (aspect ratio is maintained automatically)
martini convert -i input.png -o output.png --width 400

# Resize SVG to fit within 500x500 bounding box (preserves aspect ratio without distortion)
martini convert -i logo.svg -o output.png --width 500 --height 500

# Downscale JPEG to height 300, and prevent upscaling if input is smaller than 300px
martini convert -i small.jpg -o output.jpg --height 300 --no-upscale
```

### 6. Convert Markdown to PDF

Convert Markdown files (`.md`, `.markdown`) directly to high-fidelity PDF documents with the embedded Pure Rust Typst engine:

```bash
# Convert Markdown to PDF (defaults to pdf when input is .md)
martini convert -i document.md

# Convert with modern styling theme, US Letter size, and Table of Contents
martini convert -i README.md -o output.pdf --theme modern --paper-size letter --toc

# Convert with custom margins, header, and footer
martini convert -i report.md -o report.pdf --margin 15mm --header "Internal Report" --footer "Confidential"

# Available themes: github (default), academic, modern, minimal
martini convert -i paper.md -o paper.pdf --theme academic
```

### 7. Convert Markdown to Word (DOCX)

Convert Markdown files directly to Microsoft Word (`.docx`) files:

```bash
# Convert markdown to docx explicitly
martini convert -i document.md -o document.docx --to docx
```

LaTeX math equations (enclosed in `$` for inline math or `$$` for display math) are parsed natively and rendered styled in Word:

```markdown
$$DCF = \sum_{t=1}^{n} \frac{FCF_t}{(1+WACC)^t} + \frac{FCF_n \times (1+g)}{(WACC - g) \times (1+WACC)^n}$$
```

### 8. Transcode Structured Data (JSON / YAML / TOML / CSV / TSV)

Convert between popular configuration and tabular data formats with automatic schema inference and column union:

```bash

# Convert JSON to YAML (concise positional syntax or convert subcommand)
martini config.json config.yaml
# or
martini convert -i config.json -o config.yaml

# Convert YAML to TOML
martini config.yaml config.toml

# Convert TOML configuration to JSON
martini settings.toml settings.json

# Convert JSON array of objects to CSV or TSV
martini records.json records.csv

# Convert CSV to TSV or JSON
martini data.csv data.json

# Convert in-place or specify target format with --to
martini convert data.json --to yaml
```

### 9. Archive Transcoding & Extraction (ZIP / TAR / TAR.GZ)

Transcode between archive formats without intermediate disk clutter, or extract with path traversal sanitization:

```bash

# Transcode ZIP directly to compressed TAR.GZ (positional or flag syntax)
martini bundle.zip bundle.tar.gz
# or
martini convert -i bundle.zip -o bundle.tar.gz

# Transcode TAR.GZ directly to ZIP
martini bundle.tar.gz bundle.zip

# Transcode TAR to ZIP
martini backup.tar backup.zip

# Transcode using --to flag
martini convert bundle.zip --to tar.gz

# Extract ZIP or TAR archive to a directory
martini convert -i release.zip -o ./dist

# Compress a directory into a ZIP or TAR archive
martini convert -i ./dist -o release.zip
```

### 10. Extended Image Formats (BMP, TIFF, QOI)

Full support for legacy and modern lightweight raster image formats:

```bash
# Convert PNG to Quite OK Image format (.qoi)
martini convert -i asset.png -o asset.qoi

# Convert TIFF or BMP image to WebP
martini convert -i scan.tiff -o scan.webp
```

### 11. Presets Management (`martini preset`)

Manage reusable conversion presets across personal workflows and team projects:

```bash
# List all active presets (from user config and ./martini-presets.toml)
martini preset list

# Inspect configuration of a specific preset
martini preset show web-optimize

# Apply a preset during conversion
martini convert -i banner.png --preset web-optimize
```

Example `martini-presets.toml`:

```toml
[presets.web-optimize]
to = "webp"
quality = 82
lossless = false

[presets.doc-pdf]
to = "pdf"
theme = "academic"
paper_size = "a4"
toc = true
```

### 12. Automated Directory Watching (`martini watch`)

Run Martini as a headless background daemon to monitor incoming files and auto-convert them to an outbox directory:

```bash
# Watch inbox directory and automatically convert incoming JSON files to YAML
martini watch -i ./inbox -o ./outbox --to yaml

# Watch with custom settle debounce (e.g. 500ms) and initial backlog scanning
martini watch -i ./inbox -o ./outbox --preset web-optimize --settle-ms 500 --initial

# Automatically delete source files after conversion
martini watch -i ./inbox -o ./outbox --to webp --delete-original

# Or move source files to an archive folder after successful processing
martini watch -i ./inbox -o ./outbox --to webp --move-originals ./processed
```

### 13. Erase Objects & Watermarks (`martini erase`)

Erase unwanted objects, text, watermarks, or persons from images using natural language prompts or custom binary masks:

```bash

# Erase text overlay from an image using natural language
martini erase photo.jpg -t text

# Erase a watermark with custom output path
martini erase document.png --target watermark -o document_clean.png

# Erase an object using a pre-existing binary mask image
martini erase input.jpg --mask mask.png

# Fine-tune mask sensitivity threshold and boundary dilation radius (in pixels)
martini erase image.jpg -t text --threshold 0.35 --dilate 5

# Save the intermediate generated binary mask for verification
martini erase image.jpg -t watermark --save-mask

# Output execution metadata as structured JSON
martini erase photo.jpg -t text --json
```

All inpainting runs locally and offline on CPU with zero cloud API dependencies, complete privacy, and memory consumption under 2 GB.

---

## Agent Integration (Agent Skills)

Martini is designed to be easily wrapped as a tool or skill for LLM agents.

### Example Tool Invocation

An agent can discover capabilities first:

```bash
martini list-formats --json
```

Output:

```json
[
  {
    "description": "Convert an SVG or raster image to a Chrome favicon (.ico or full favicon package)",
    "from": "svg, png, jpg, jpeg, webp, avif",
    "to": "favicon",
    "parameters": {
      "package": "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)"
    }
  },
  {
    "description": "Convert images to WebP format",
    "from": "svg, png, jpg, jpeg, webp, avif",
    "to": "webp",
    "parameters": {
      "quality": "integer (1-100, default 80)",
      "lossless": "boolean (default false)",
      "overwrite": "boolean (default false)",
      "delete_original": "boolean (default false)",
      "recursive": "boolean (default false)",
      "workers": "integer (optional)"
    }
  }
]
```

Then perform the conversion in JSON mode:

```bash
martini convert -i test.svg -o out.ico --json
```

Output on `stdout`:

```json
{
  "from": "svg",
  "to": "favicon",
  "output_files": [
    {
      "path": "out.ico",
      "size_bytes": 15086,
      "description": "Multi-resolution Windows favicon (16x16, 32x32, 48x48)"
    }
  ]
}
```

### Exit Codes

Programmatic automation can rely on the following exit codes:

| Code | Label | Description |
| --- | --- | --- |
| **0** | `SUCCESS` | Conversion or command completed successfully. |
| **1** | `INVALID_ARGUMENTS` | Command line arguments parsing failed. |
| **2** | `INPUT_FILE_NOT_FOUND` | The specified input path does not exist. |
| **3** | `INVALID_INPUT_DATA` | Input data is empty or invalid SVG. |
| **4** | `PROCESSING_ERROR` | Internal rendering, resizing, or encoding failure. |
| **5** | `OUTPUT_WRITE_ERROR` | Permission denied or target directory unwritable. |
| **6** | `UNSUPPORTED_CONVERSION` | Invalid from/to combination. |

---

## License

This project is licensed under the Apache 2.0 License - see the `LICENSE` file for details.
