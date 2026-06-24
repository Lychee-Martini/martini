# Design Specification: Martini Refactoring and Bugfixes

A comprehensive design to resolve CLI coupling, PDFium concurrency risks, favicon output path bug, code duplication, and batch conversion pipeline complexity.

## Goal Description

Improve code quality, maintainability, and concurrency stability of the Martini format converter CLI/library.

---

## Proposed Changes

We will restructure the codebase across library modules, CLI formatting wrappers, and core conversion routines.

### 1. Format Registry (`src/converter/mod.rs` & `src/lib.rs`)

Move the capabilities schema and description details out of `src/main.rs` into the library.

- Define a `FormatCapability` struct in `src/converter/mod.rs` to store source/target formats, description text, and extra flags/options.
- Implement `pub fn get_supported_formats() -> Vec<FormatCapability>` in `src/converter/mod.rs`.
- Re-export these in `src/lib.rs` for consumer visibility.

### 2. PDFium Thread Safety (`src/converter/pdf_conv.rs`)

Avoid repeated concurrent dynamic library binds.

- Introduce a static `std::sync::OnceLock<Result<pdfium_render::prelude::Pdfium, String>>` instance in `src/converter/pdf_conv.rs`.
- Create a function `get_pdfium() -> Result<&'static pdfium_render::prelude::Pdfium, MartiniError>` to safely acquire a shared reference to the bound library.
- Refactor both single and batch PDF conversion routines to use `get_pdfium()`.

### 3. CLI UI Isolation (`src/cli/ui.rs` [NEW] & `src/cli/mod.rs`)

De-couple output presentation from standard CLI run flow.

- **[NEW]** [src/cli/ui.rs](file:///e:/Source/martini/src/cli/ui.rs): Define table formatting, progress tracker setups, byte-to-human size translations, string truncation helper, and setup panels.
- Update `src/cli/mod.rs` to expose the new `ui` module.
- Modify `src/main.rs` to import from `martini::cli::ui::*` instead of holding local helpers.

### 4. Smart Favicon Package Output Resolver (`src/converter/image_conv.rs`)

Fix the directory creation bug in package mode:

- Parse `options.output_path` inside `generate_favicon`.
- If the output path has a file extension or is not an existing directory, treat it as a file:
  - Output folder (parent) = directory of the output path.
  - ICO output filename = file name of the output path.
- Else, default to using the directory directly, outputting to `favicon.ico` within it.

### 5. Simplifying Batch Conversion with Rayon (`src/converter/batch.rs`)

Replace the custom thread channels with Rayon's direct task scheduler.

- Refactor `batch_convert` to map tasks using Rayon parallel iterators (`into_par_iter()`).
- Each task reads the input, performs conversions, and writes the output directly.
- Group the resulting list of `TaskResult` items and return them.

---

## Verification Plan

### Automated Tests
- Run `cargo test` to verify no regressions in current tests.
- Add test coverage for PDFium concurrent rendering and smart favicon directory logic.

### Manual Verification
- Run `cargo run -- list-formats` and `cargo run -- list-formats --json` to inspect CLI capabilities schemas.
- Run `cargo run -- convert -i tests/fixtures/sample.svg -o target/output_favicon.ico --package` to ensure target folder output behaves correctly.
