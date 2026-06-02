# Martini 🍸

A modern, extensible, and high-performance CLI format converter written in Rust. Designed specifically to be robust for human developers and seamlessly integrable with AI Agent workflows (Agent Skills).

## Features

- 🎨 **Pure Rust SVG rendering engine**: Powered by `resvg` and `tiny-skia`. Self-contained and fully portable across Windows, macOS, and Linux without native system dependencies (like Cairo or Glib).
- 📦 **SVG to Favicon conversion**:
  - **Default**: Generates a standard multi-resolution `favicon.ico` containing `16x16`, `32x32`, and `48x48` dimensions.
  - **Package Mode (`--package`)**: Generates a comprehensive modern suite of assets including:
    - `favicon.ico` (multi-resolution)
    - `favicon-16x16.png` & `favicon-32x32.png`
    - `apple-touch-icon.png` (180x180)
    - `android-chrome-192x192.png` & `android-chrome-512x512.png`
    - `site.webmanifest`
    - `favicon-tags.html` (copy-pasteable `<link>` tags)
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

### 2. Convert SVG to favicon.ico (Default)
Generates a single multi-resolution ICO file at the target path:
```bash
martini convert --from svg --to favicon -i logo.svg -o favicon.ico
```

### 3. Convert SVG to Favicon Package
Generates all sizes, manifests, and HTML snippet under the target directory:
```bash
martini convert --from svg --to favicon -i logo.svg -o ./icons_dir --package
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
    "description": "Convert an SVG vector image to a Chrome favicon (.ico or full favicon package)",
    "from": "svg",
    "parameters": {
      "package": "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)"
    },
    "to": "favicon"
  }
]
```

Then perform the conversion in JSON mode:
```bash
martini convert --from svg --to favicon -i test.svg -o out.ico --json
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