# Martini 🍸

A modern, extensible, and high-performance CLI format converter written in Rust. Designed specifically to be robust for human developers and seamlessly integrable with AI Agent workflows (Agent Skills).

## Features

- 🎨 **Pure Rust SVG rendering engine**: Powered by `resvg` and `tiny-skia`. Self-contained and fully portable across Windows, macOS, and Linux without native system dependencies (like Cairo or Glib).
- 🖼️ **Unified Image Conversion**: Supports converting between SVG, PNG, JPG/JPEG, WebP, and AVIF in any combination.
- 📦 **Favicon generation from any source**: Converts **both** SVG vector files (rendered dynamically at multiple resolutions) and standard raster images (downscaled using Lanczos3 filtering) to a favicon:
  - **Default**: Generates a standard multi-resolution `favicon.ico` containing `16x16`, `32x32`, and `48x48` dimensions.
  - **Package Mode (`--package`)**: Generates a comprehensive modern suite of assets including `favicon.ico`, PNGs (16x16, 32x32, 180x180, 192x192, 512x512), `site.webmanifest`, and copy-pasteable HTML header tags.
- 🏎️ **Concurrently Optimized Batch Processing**: Process a single file or a whole directory concurrently using a `rayon` thread pool. Includes customizable worker threads and directory recursion (`-r`).
- 📝 **Markdown to Word (DOCX)**: Convert Markdown documents containing rich elements (headings, bold/italic/strikethrough/inline code, bulleted/numbered lists, blockquotes, tables, hyperlinks, and LaTeX equations) directly to standard Word documents (`.docx`) natively.
- ⚡ **Developer-Friendly & Auto-Detecting CLI**:
  - Make `--from` optional (defaults to `"auto"` detecting from source extensions).
  - Make `--to` optional (detects target format from `--output` extension, or defaults to `favicon` for SVGs and `webp` for other images).
  - Make `--output` optional (omitted outputs are generated in-place next to source files).
  - Support deleting source files after successful conversion (`-d` / `--delete-original`).
- 🤖 **Agent Skill Ready**:
  - **JSON Mode (`--json`)**: Command results and errors are output in structured, machine-readable JSON.
  - **Self-Discovery (`list-formats`)**: A structured command returning the capabilities schema so agents can learn available format pairs dynamically.
  - **Structured Exit Codes**: Granular, typed exit codes map to distinct failure profiles.
  - **Level-controlled Tracing**: Logs are separated to `stderr` and can be adjusted with `--quiet` or `--verbose`.

---

## Installation

Ensure you have Rust installed (1.75+), then clone the repository and build:

```bash
cargo build --release
```

The compiled binary will be located at `target/release/martini`.

---

## CLI Usage

### 1. View Supported Formats
Retrieve supported format pairs and metadata (default outputs JSON structure):
```bash
martini list-formats
```

Or structured JSON:
```bash
martini list-formats --json
```

### 2. Convert Single Files (Simplified Syntax)
Martini automatically detects source and target formats from file extensions:
```bash
# Convert SVG to PNG (auto-detects 'svg' and 'png')
martini convert -i logo.svg -o logo.png

# Convert PNG to favicon package (auto-detects 'png' and 'favicon')
martini convert -i logo.png -o ./icons_dir --package

# Convert JPG to WebP in-place (defaults to webp target and in-place output)
martini convert -i photo.jpg
```

### 3. Batch Convert a Directory
Concurrently convert all images in a folder:
```bash
# Convert all PNG/JPG images in the directory to WebP
martini convert -i ./images --to webp

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

### 6. Convert Markdown to Word (DOCX)
Convert Markdown files (`.md`, `.markdown`) directly to Microsoft Word (`.docx`) files:
```bash
# Convert markdown to docx (auto-detects 'md' and 'docx')
martini convert -i document.md -o document.docx

# Auto-detection default target (defaults output extension to .docx)
martini convert -i document.md
```
LaTeX math equations (enclosed in `$` for inline math or `$$` for display math) are parsed natively and rendered styled in Word:
```markdown
$$DCF = \sum_{t=1}^{n} \frac{FCF_t}{(1+WACC)^t} + \frac{FCF_n \times (1+g)}{(WACC - g) \times (1+WACC)^n}$$
```

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
|---|---|---|
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