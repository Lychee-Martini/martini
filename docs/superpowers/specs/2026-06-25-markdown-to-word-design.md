# Spec: Markdown to Word (DOCX) Conversion in Martini

This document details the design for adding Markdown (`.md`) to Word (`.docx`) file conversion functionality to the Martini CLI format converter.

## 1. Goal
Add robust, self-contained Markdown-to-DOCX conversion capabilities. This will parse Markdown documents (supporting standard elements and GFM tables) and build Microsoft Word documents (`.docx`) natively in Rust without any external system dependencies.

## 2. Requirements & Scope
- **Input Format**: Markdown (`.md`, `.markdown`)
- **Output Format**: Word (`.docx`)
- **Supported Elements**:
  - Headings (H1 to H6) with hierarchy styling
  - Standard paragraphs
  - Bold, Italic, Strikethrough, and Inline Code text styling
  - Lists: ordered (numbered) and unordered (bulleted), including nested lists
  - Blockquotes (indented paragraphs)
  - Tables (GFM standard with headers, borders, alignments)
  - Links (styled as blue underlined text)
  - Code Blocks (styled as monospaced text with background shading)
- **Architecture**: Integrated into the existing `martini::converter` framework.
- **Dependencies**: 
  - `pulldown-cmark` for compliant and fast Markdown event-stream parsing.
  - `docx-rs` for programmatic DOCX generation.

## 3. Proposed Changes

### A. Dependency Configuration
We will add `docx-rs` and `pulldown-cmark` to `Cargo.toml`.

### B. Core Model and Architecture
1. **`Format` Enum (in `src/converter/mod.rs`)**:
   - Add `Format::Md` and `Format::Docx` variants.
   - Update string serialization and deserialization functions.
2. **Capabilities**:
   - Register the capability to convert from `md` to `docx` in `get_supported_formats()`.
3. **Dispatching**:
   - In `convert()`, check if `from == Format::Md` and `to == Format::Docx`. If so, delegate to the new `MarkdownConverter`.

### C. Markdown Conversion Engine (`src/converter/md_conv.rs`)
Implement the conversion logic using an event-driven parser that maps Markdown events directly to `docx-rs` document elements.

1. **Stack-based Styles**:
   Maintain boolean flags for styling (`bold`, `italic`, `strikethrough`, `is_code`) and optionally url strings for `link`.
2. **Accumulators**:
   - Paragraphs are assembled cell by cell, item by item.
   - List nesting is tracked using a list stack to compute correct left indents and prefix symbols/numbers.
   - Tables are accumulated as rows of cells, and then built natively.

### D. CLI CLI integration (`src/main.rs` & `src/cli/commands.rs`)
1. **Auto-detection**: If the source extension is `.md`, resolve the input format as `Format::Md` and set the target format default to `Format::Docx`.
2. **Help Information**: Ensure `ListFormats` outputs the Markdown to Word capability correctly.

## 4. Verification Plan
- **Unit Tests**:
  - Test `Format::from_str` and display for `md` and `docx`.
- **Integration Tests**:
  - Create a test markdown fixture containing paragraphs, headings, lists, inline formatting, a table, links, and code blocks.
  - Run the CLI converter command to verify the conversion finishes successfully.
  - Validate that the output file is a valid `.docx` (checking size, unzip structure, or loading it via `docx-rs` parser if applicable).
