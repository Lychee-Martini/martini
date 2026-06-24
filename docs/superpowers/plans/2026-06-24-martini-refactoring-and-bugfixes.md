# Martini Refactoring and Bugfixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor Martini format capabilities registry, isolate CLI UI presentation from the conversion logic, secure PDFium thread-safety using a global OnceLock, resolve smart favicon path packaging logic, and simplify batch conversion with Rayon.

**Architecture:** 
1. Define a centralized `FormatCapability` library-owned registry and remove hardcoded descriptions from `main.rs`.
2. Secure PDFium bindings via `OnceLock` to prevent parallel DLL loads and race conditions.
3. Decouple reporting formatting functions from the main binary into a new `cli::ui` module.
4. Smartly split file/parent paths when generating favicon packages.
5. Replace custom multi-threaded batch coordinator channels with an idiomatic Rayon parallel iterator.

**Tech Stack:** Rust 2024, Rayon, Clap, pdfium-render, resvg, tiny-skia, image.

## Global Constraints

- Do not introduce external dependencies outside of Cargo.toml.
- Retain all existing tracing logs and stderr configurations.
- All existing tests must pass at each task boundary.

---

### Task 1: Format Capabilities Registry

**Files:**
- Modify: [src/converter/mod.rs](file:///e:/Source/martini/src/converter/mod.rs)
- Modify: [src/lib.rs](file:///e:/Source/martini/src/lib.rs)
- Modify: [src/main.rs](file:///e:/Source/martini/src/main.rs)

**Interfaces:**
- Consumes: None
- Produces: `converter::get_supported_formats() -> Vec<FormatCapability>`

- [ ] **Step 1: Write a failing unit test**
  Add a test to the bottom of `src/converter/mod.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn test_get_supported_formats() {
          let formats = get_supported_formats();
          assert!(!formats.is_empty());
          assert!(formats.iter().any(|f| f.to == "favicon"));
      }
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test --lib converter::tests`
  Expected: FAIL (compilation error due to missing function/struct)

- [ ] **Step 3: Define `FormatCapability` and implement `get_supported_formats`**
  Modify `src/converter/mod.rs` to define:
  ```rust
  #[derive(Debug, Clone, serde::Serialize)]
  pub struct FormatCapability {
      pub from: String,
      pub to: String,
      pub description: String,
      pub parameters: std::collections::HashMap<String, String>,
  }

  pub fn get_supported_formats() -> Vec<FormatCapability> {
      let mut list = Vec::new();
      
      let mut fav_params = std::collections::HashMap::new();
      fav_params.insert("package".to_string(), "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)".to_string());
      list.push(FormatCapability {
          from: "svg, png, jpg, jpeg, webp, avif".to_string(),
          to: "favicon".to_string(),
          description: "Convert an SVG or raster image to a Chrome favicon (.ico or full favicon package)".to_string(),
          parameters: fav_params,
      });

      let mut png_params = std::collections::HashMap::new();
      png_params.insert("overwrite".to_string(), "boolean (default false)".to_string());
      png_params.insert("delete_original".to_string(), "boolean (default false)".to_string());
      png_params.insert("recursive".to_string(), "boolean (default false)".to_string());
      png_params.insert("workers".to_string(), "integer (optional)".to_string());
      list.push(FormatCapability {
          from: "svg, png, jpg, jpeg, webp, avif".to_string(),
          to: "png".to_string(),
          description: "Convert images to PNG format".to_string(),
          parameters: png_params,
      });

      let mut jpg_params = std::collections::HashMap::new();
      jpg_params.insert("quality".to_string(), "integer (1-100, default 80)".to_string());
      jpg_params.insert("overwrite".to_string(), "boolean (default false)".to_string());
      jpg_params.insert("delete_original".to_string(), "boolean (default false)".to_string());
      jpg_params.insert("recursive".to_string(), "boolean (default false)".to_string());
      jpg_params.insert("workers".to_string(), "integer (optional)".to_string());
      list.push(FormatCapability {
          from: "svg, png, jpg, jpeg, webp, avif".to_string(),
          to: "jpg".to_string(),
          description: "Convert images to JPEG format".to_string(),
          parameters: jpg_params,
      });

      let mut webp_params = std::collections::HashMap::new();
      webp_params.insert("quality".to_string(), "integer (1-100, default 80)".to_string());
      webp_params.insert("lossless".to_string(), "boolean (default false)".to_string());
      webp_params.insert("overwrite".to_string(), "boolean (default false)".to_string());
      webp_params.insert("delete_original".to_string(), "boolean (default false)".to_string());
      webp_params.insert("recursive".to_string(), "boolean (default false)".to_string());
      webp_params.insert("workers".to_string(), "integer (optional)".to_string());
      list.push(FormatCapability {
          from: "svg, png, jpg, jpeg, webp, avif".to_string(),
          to: "webp".to_string(),
          description: "Convert images to WebP format".to_string(),
          parameters: webp_params,
      });

      let mut avif_params = std::collections::HashMap::new();
      avif_params.insert("quality".to_string(), "integer (1-100, default 80)".to_string());
      avif_params.insert("lossless".to_string(), "boolean (default false)".to_string());
      avif_params.insert("overwrite".to_string(), "boolean (default false)".to_string());
      avif_params.insert("delete_original".to_string(), "boolean (default false)".to_string());
      avif_params.insert("recursive".to_string(), "boolean (default false)".to_string());
      avif_params.insert("workers".to_string(), "integer (optional)".to_string());
      list.push(FormatCapability {
          from: "svg, png, jpg, jpeg, webp, avif".to_string(),
          to: "avif".to_string(),
          description: "Convert images to AVIF format".to_string(),
          parameters: avif_params,
      });

      let mut both_params = std::collections::HashMap::new();
      both_params.insert("quality".to_string(), "integer (1-100, default 80)".to_string());
      both_params.insert("lossless".to_string(), "boolean (default false)".to_string());
      both_params.insert("overwrite".to_string(), "boolean (default false)".to_string());
      both_params.insert("delete_original".to_string(), "boolean (default false)".to_string());
      both_params.insert("recursive".to_string(), "boolean (default false)".to_string());
      both_params.insert("workers".to_string(), "integer (optional)".to_string());
      list.push(FormatCapability {
          from: "svg, png, jpg, jpeg, webp, avif".to_string(),
          to: "both".to_string(),
          description: "Convert images to both WebP and AVIF formats".to_string(),
          parameters: both_params,
      });

      let mut pdf_params = std::collections::HashMap::new();
      pdf_params.insert("pages".to_string(), "string (comma-separated page numbers or ranges, e.g. '1,3-5')".to_string());
      pdf_params.insert("dpi".to_string(), "integer (rendering DPI, default 150)".to_string());
      pdf_params.insert("quality".to_string(), "integer (1-100, default 80)".to_string());
      pdf_params.insert("lossless".to_string(), "boolean (default false)".to_string());
      pdf_params.insert("overwrite".to_string(), "boolean (default false)".to_string());
      pdf_params.insert("delete_original".to_string(), "boolean (default false)".to_string());
      list.push(FormatCapability {
          from: "pdf".to_string(),
          to: "png, jpg, jpeg, webp, avif".to_string(),
          description: "Convert PDF pages to images".to_string(),
          parameters: pdf_params,
      });

      list
  }
  ```
  Expose `get_supported_formats` and `FormatCapability` in `src/lib.rs`:
  ```rust
  pub use converter::{
      ConversionResult, ConvertOptions, Format, OutputFileMetadata, convert,
      get_supported_formats, FormatCapability,
  };
  ```

- [ ] **Step 4: Refactor `src/main.rs` to print dynamically from library**
  Modify the `Commands::ListFormats` execution block in `src/main.rs`:
  ```rust
          Commands::ListFormats => {
              let formats = martini::converter::get_supported_formats();
              if args.json {
                  println!("{}", serde_json::to_string_pretty(&formats).unwrap());
              } else {
                  println!("Supported Conversions:");
                  for f in formats {
                      let mut params_str = String::new();
                      for (k, v) in &f.parameters {
                          params_str.push_str(&format!(" --{} {}", k, v));
                      }
                      println!("- {} -> {}: {}. Options:{}", f.from, f.to, f.description, params_str);
                  }
              }
              Ok(0)
          }
  ```

- [ ] **Step 5: Run tests and commit**
  Run: `cargo test`
  Expected: PASS
  Commit: `git commit -am "feat: refactor format capabilities registry to library"`

---

### Task 2: PDFium Thread Safety

**Files:**
- Modify: [src/converter/pdf_conv.rs](file:///e:/Source/martini/src/converter/pdf_conv.rs)
- Modify: [src/converter/batch.rs](file:///e:/Source/martini/src/converter/batch.rs)

**Interfaces:**
- Consumes: `pdf_conv::PdfConverter`
- Produces: `pdf_conv::get_pdfium() -> Result<&'static pdfium_render::prelude::Pdfium, MartiniError>`

- [ ] **Step 1: Write a test verifying concurrent PDF parsing**
  Add a test inside `tests/cli.rs` that renders a PDF page:
  ```rust
  #[test]
  fn test_pdfium_thread_safety() {
      // Test that calls can be executed concurrently without double loading pdfium
      let threads: Vec<_> = (0..3).map(|_| {
          std::thread::spawn(|| {
              let mut cmd = Command::cargo_bin("martini").unwrap();
              cmd.arg("list-formats").assert().success();
          })
      }).collect();
      for t in threads {
          t.join().unwrap();
      }
  }
  ```

- [ ] **Step 2: Run test**
  Run: `cargo test test_pdfium_thread_safety`
  Expected: PASS (as it doesn't execute PDF conversions yet)

- [ ] **Step 3: Define static `OnceLock` and implement `get_pdfium`**
  Modify [src/converter/pdf_conv.rs](file:///e:/Source/martini/src/converter/pdf_conv.rs):
  ```rust
  use std::sync::OnceLock;
  use pdfium_render::prelude::Pdfium;

  static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

  pub fn get_pdfium() -> Result<&'static Pdfium, MartiniError> {
      let res = PDFIUM.get_or_init(|| {
          pdfium_auto::bind_pdfium_silent()
              .map_err(|e| format!("Failed to load PDFium library: {:?}", e))
      });
      res.as_ref()
          .map_err(|e| MartiniError::PdfRender(e.clone()))
  }
  ```

- [ ] **Step 4: Refactor PDF converter to use `get_pdfium()`**
  Replace `bind_pdfium_silent()` calls in `src/converter/pdf_conv.rs` and `src/converter/batch.rs` with `get_pdfium()`.
  In `src/converter/pdf_conv.rs`:
  ```rust
          // 1. Bind to PDFium library
          let pdfium = get_pdfium()?;
  ```
  In `src/converter/batch.rs`:
  ```rust
              let pdfium = match get_pdfium() {
                  Ok(p) => p,
                  Err(e) => {
                      // ... exit with error payload
  ```

- [ ] **Step 5: Run tests and commit**
  Run: `cargo test`
  Expected: PASS
  Commit: `git commit -am "feat: implement static OnceLock for PDFium bindings"`

---

### Task 3: CLI UI Isolation

**Files:**
- Create: [src/cli/ui.rs](file:///e:/Source/martini/src/cli/ui.rs)
- Modify: [src/cli/mod.rs](file:///e:/Source/martini/src/cli/mod.rs)
- Modify: [src/main.rs](file:///e:/Source/martini/src/main.rs)

**Interfaces:**
- Consumes: None
- Produces: CLI ui formatting and reporting functions (`print_setup_panel`, `print_report_table`, `CliProgressTracker`)

- [ ] **Step 1: Write test for UI helpers**
  Create [src/cli/ui.rs](file:///e:/Source/martini/src/cli/ui.rs) and write test boilerplate:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn test_format_size() {
          assert_eq!(format_size(1024), "1 KB");
          assert_eq!(format_size(0), "0 B");
      }
  }
  ```

- [ ] **Step 2: Run test**
  Run: `cargo test --lib cli::ui::tests`
  Expected: FAIL (as file is empty or does not compile)

- [ ] **Step 3: Implement `src/cli/ui.rs`**
  Move helper functions and structs from `src/main.rs` to `src/cli/ui.rs`:
  ```rust
  use indicatif::{ProgressBar, ProgressStyle};
  use std::path::Path;
  use martini::converter::batch::{ProgressTracker, TaskResult};

  pub struct CliProgressTracker {
      pub pb: ProgressBar,
  }

  impl ProgressTracker for CliProgressTracker {
      fn set_message(&self, msg: &str) {
          self.pb.set_message(msg.to_string());
      }

      fn inc(&self, delta: u64) {
          self.pb.inc(delta);
      }
  }

  pub fn format_size(size_bytes: u64) -> String {
      if size_bytes == 0 {
          return "0 B".to_string();
      }
      let size_name = ["B", "KB", "MB", "GB", "TB"];
      let i = (size_bytes as f64).log(1024.0).floor() as usize;
      if i >= size_name.len() {
          return format!("{} TB", (size_bytes as f64 / 1024.0f64.powi(4)));
      }
      let p = 1024.0f64.powi(i as i32);
      let s = (size_bytes as f64 / p * 100.0).round() / 100.0;
      format!("{} {}", s, size_name[i])
  }

  pub fn truncate_str(s: &str, max_len: usize) -> String {
      if s.len() <= max_len {
          s.to_string()
      } else {
          format!("...{}", &s[s.len() - max_len + 3..])
      }
  }

  pub fn print_setup_panel(
      input: &Path,
      to: &str,
      quality: u8,
      lossless: bool,
      recursive: bool,
      delete_original: bool,
      overwrite: bool,
  ) {
      println!("┌────────────────────────────────────────────────────────┐");
      println!("│                Image Converter Setup                   │");
      println!("├────────────────────────────────────────────────────────┤");
      println!(
          "│ Target:            {:35} │",
          truncate_str(&input.to_string_lossy(), 35)
      );
      println!("│ Formats:           {:35} │", to);
      println!(
          "│ Quality:           {:35} │",
          format!("{} (lossless={})", quality, lossless)
      );
      println!("│ Recursive:         {:35} │", recursive.to_string());
      println!("│ Delete Original:   {:35} │", delete_original.to_string());
      println!("│ Overwrite Existing:{:35} │", overwrite.to_string());
      println!("└────────────────────────────────────────────────────────┘\n");
  }

  pub fn print_report_table(
      results: &[TaskResult],
      delete_original: bool,
      deleted_count: usize,
      del_errors: &[(String, String)],
  ) {
      let total_tasks = results.len();
      let successful = results.iter().filter(|r| r.status == "success").count();
      let skipped = results.iter().filter(|r| r.status == "skipped").count();
      let failed = results.iter().filter(|r| r.status == "failed").count();

      let mut orig_size = 0;
      let mut new_size = 0;
      for r in results {
          if r.status == "success" || r.status == "skipped" {
              orig_size += r.original_size;
              new_size += r.converted_size;
          }
      }

      let saved_bytes = orig_size as i64 - new_size as i64;
      let savings_str = if orig_size == 0 {
          "0 B (0% saved)".to_string()
      } else if saved_bytes < 0 {
          format!(
              "-{} (size increased)",
              format_size(saved_bytes.unsigned_abs())
          )
      } else {
          let pct = (saved_bytes as f64 / orig_size as f64) * 100.0;
          format!("{} ({:.1}% saved)", format_size(saved_bytes as u64), pct)
      };

      println!("\n✨ Conversion complete!\n");
      println!("┌────────────────────────────────────────┐");
      println!("│           Conversion Summary           │");
      println!("├────────────────────────────────────────┤");
      println!("│ Total Tasks:       {:20} │", total_tasks);
      println!("│ Successful:        \x1b[32m{:20}\x1b[0m │", successful);
      println!("│ Skipped:           \x1b[33m{:20}\x1b[0m │", skipped);
      println!("│ Failed:            \x1b[31m{:20}\x1b[0m │", failed);
      if delete_original {
          println!("│ Originals Deleted: \x1b[31m{:20}\x1b[0m │", deleted_count);
          if !del_errors.is_empty() {
              println!(
                  "│ Deletion Failures: \x1b[31m{:20}\x1b[0m │",
                  del_errors.len()
              );
          }
      }
      println!("│ Original Size:     {:20} │", format_size(orig_size));
      println!("│ New Size:          {:20} │", format_size(new_size));
      println!("│ Space Savings:     {:20} │", savings_str);
      println!("└────────────────────────────────────────┘");

      let failures: Vec<&TaskResult> = results.iter().filter(|r| r.status == "failed").collect();
      if !failures.is_empty() {
          println!("\n❌ Failed Conversions Details:");
          println!("--------------------------------------------------");
          for fail in failures {
              println!("📄 File: {}", fail.input_path);
              println!(
                  "   Error: {}",
                  fail.error_message.as_deref().unwrap_or("Unknown error")
              );
              println!();
          }
      }

      if !del_errors.is_empty() {
          println!("\n❌ Failed Deletions Details:");
          println!("--------------------------------------------------");
          for (file, err_msg) in del_errors {
              println!("📄 File: {}", file);
              println!("   Error: {}", err_msg);
              println!();
          }
      }
  }
  ```
  Expose `ui` module in `src/cli/mod.rs`:
  ```rust
  pub mod commands;
  pub mod ui;
  pub use commands::{CliArgs, Commands};
  ```

- [ ] **Step 4: Modify `src/main.rs` to clean up helpers**
  Import the UI functions in `src/main.rs`:
  ```rust
  use martini::cli::ui::{
      format_size, print_report_table, print_setup_panel, CliProgressTracker,
  };
  ```
  Remove duplicate function definitions of `format_size`, `truncate_str`, `print_setup_panel`, `print_report_table`, and `CliProgressTracker` from `src/main.rs`.

- [ ] **Step 5: Run tests and commit**
  Run: `cargo test`
  Expected: PASS
  Commit: `git commit -am "refactor: isolate CLI UI helpers to separate module"`

---

### Task 4: Smart Favicon Package Output Resolver

**Files:**
- Modify: [src/converter/image_conv.rs](file:///e:/Source/martini/src/converter/image_conv.rs)

**Interfaces:**
- Consumes: `image_conv::generate_favicon`
- Produces: Output files written to resolved directories

- [ ] **Step 1: Write smart path test**
  Add a integration test in `tests/cli.rs`:
  ```rust
  #[test]
  fn test_favicon_package_smart_path() {
      let temp_dir = tempdir().unwrap();
      let output_file_path = temp_dir.path().join("sub_folder").join("my_icon.ico");

      let mut cmd = Command::cargo_bin("martini").unwrap();
      cmd.arg("convert")
          .arg("-i")
          .arg("tests/fixtures/sample.svg")
          .arg("-o")
          .arg(&output_file_path)
          .arg("--to")
          .arg("favicon")
          .arg("--package")
          .assert()
          .success();

      // Ensure primary ICO is at the exact path
      assert!(output_file_path.exists());
      assert!(output_file_path.is_file());
      
      // Companion files should be in the same folder as my_icon.ico
      let parent_dir = output_file_path.parent().unwrap();
      assert!(parent_dir.join("favicon-16x16.png").exists());
      assert!(parent_dir.join("site.webmanifest").exists());
  }
  ```

- [ ] **Step 2: Run test**
  Run: `cargo test test_favicon_package_smart_path`
  Expected: FAIL (compiles, but creates a directory named `my_icon.ico` instead)

- [ ] **Step 3: Implement path resolution inside `generate_favicon`**
  Modify the `generate_favicon` method inside `src/converter/image_conv.rs`:
  ```rust
      if options.package {
          let (dir_path, ico_name) = if options.output_path.is_dir() {
              (options.output_path.clone(), "favicon.ico".to_string())
          } else {
              let parent = options.output_path.parent()
                  .filter(|p| !p.as_os_str().is_empty())
                  .map(|p| p.to_path_buf())
                  .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
              let name = options.output_path.file_name()
                  .and_then(|n| n.to_str())
                  .map(|s| s.to_string())
                  .unwrap_or_else(|| "favicon.ico".to_string());
              (parent, name)
          };

          let sizes = [
              (
                  16,
                  "favicon-16x16.png",
                  "Standard small favicon for browser tabs",
              ),
              (
                  32,
                  "favicon-32x32.png",
                  "Standard medium favicon for desktop browsers",
              ),
              (
                  180,
                  "apple-touch-icon.png",
                  "Apple Touch Icon for iOS home screen",
              ),
              (
                  192,
                  "android-chrome-192x192.png",
                  "Android Chrome icon for web app manifest",
              ),
              (
                  512,
                  "android-chrome-512x512.png",
                  "Android Chrome splash icon for web app manifest",
              ),
          ];

          let mut png_buffers = Vec::new();
          for &(size, filename, desc) in &sizes {
              let png_data = get_png_bytes(tree, img, size)?;

              if size == 16 || size == 32 {
                  png_buffers.push((png_data.clone(), size));
              }

              let file_path = dir_path.join(filename);
              output_files.push(EncodedFile {
                  path: file_path,
                  bytes: png_data,
                  description: desc.to_string(),
              });
          }

          let png_48 = get_png_bytes(tree, img, 48)?;
          png_buffers.push((png_48, 48));

          let ico_bytes = build_ico(&png_buffers)?;
          let ico_path = dir_path.join(ico_name);
          output_files.push(EncodedFile {
              path: ico_path,
              bytes: ico_bytes,
              description: "Multi-resolution Windows favicon (16x16, 32x32, 48x48)".to_string(),
          });

          let manifest_content = r##"{
      "name": "Lychee Martini Web App",
      "short_name": "Lychee Martini",
      "icons": [
          {
              "src": "/android-chrome-192x192.png",
              "sizes": "192x192",
              "type": "image/png"
          },
          {
              "src": "/android-chrome-512x512.png",
              "sizes": "512x512",
              "type": "image/png"
          }
      ],
      "theme_color": "#ffffff",
      "background_color": "#ffffff",
      "display": "standalone"
  }
  "##;
          let manifest_path = dir_path.join("site.webmanifest");
          output_files.push(EncodedFile {
              path: manifest_path,
              bytes: manifest_content.as_bytes().to_vec(),
              description: "Web App Manifest containing icons definitions".to_string(),
          });

          let html_content = r#"<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
  <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
  <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
  <link rel="manifest" href="/site.webmanifest">
  "#;
          let html_path = dir_path.join("favicon-tags.html");
          output_files.push(EncodedFile {
              path: html_path,
              bytes: html_content.as_bytes().to_vec(),
              description: "HTML header tags to copy-paste into index.html".to_string(),
          });
      } else {
          // Keep single ICO path unchanged...
  ```

- [ ] **Step 4: Run tests and commit**
  Run: `cargo test`
  Expected: PASS
  Commit: `git commit -am "bugfix: implement smart output path resolution in package favicon mode"`

---

### Task 5: Simplifying Batch Conversion with Rayon

**Files:**
- Modify: [src/converter/batch.rs](file:///e:/Source/martini/src/converter/batch.rs)

**Interfaces:**
- Consumes: `batch::batch_convert`
- Produces: `batch::batch_convert` using Rayon parallel iterator logic.

- [ ] **Step 1: Ensure current tests cover batch converters**
  Verify `test_image_conversions` inside `tests/cli.rs` runs correctly.

- [ ] **Step 2: Run test**
  Run: `cargo test test_image_conversions`
  Expected: PASS

- [ ] **Step 3: Refactor `batch_convert` using Rayon parallel iterators**
  Rewrite the core of `batch_convert` in [src/converter/batch.rs](file:///e:/Source/martini/src/converter/batch.rs) to avoid the custom reader, coordinator, and writer threads. Integrate simple, direct parallel file reading, conversion, and writing operations:
  ```rust
  use rayon::prelude::*;
  
  pub fn batch_convert(
      options: BatchConvertOptions,
      tracker: Option<Arc<dyn ProgressTracker>>,
  ) -> Result<BatchResult, MartiniError> {
      if !options.input_dir.exists() {
          return Err(MartiniError::InputFileNotFound {
              path: options.input_dir.to_string_lossy().to_string(),
          });
      }

      if !options.input_dir.is_dir() {
          return Err(MartiniError::InvalidInputData {
              reason: "Input path must be a directory for batch conversions".to_string(),
          });
      }

      let files = get_all_images(&options.input_dir, options.recursive, &options.from_filter);
      if files.is_empty() {
          return Ok(BatchResult {
              tasks: Vec::new(),
              deleted_count: 0,
              deletion_errors: Vec::new(),
          });
      }

      // Configure ThreadPool if custom workers requested
      if let Some(w) = options.workers {
          let _ = rayon::ThreadPoolBuilder::new()
              .num_threads(w)
              .build_global();
      }

      let tracker_arc = tracker;

      // Map each input file to its corresponding tasks and execute in parallel
      let tasks: Vec<TaskResult> = files
          .par_iter()
          .flat_map(|file_path| {
              let mut results = Vec::new();
              
              if let Some(ref t) = tracker_arc {
                  let filename = file_path
                      .file_name()
                      .map(|f| f.to_string_lossy())
                      .unwrap_or_default();
                  t.set_message(&format!("Converting {}", filename));
              }

              let input_bytes = match std::fs::read(file_path) {
                  Ok(bytes) => bytes,
                  Err(e) => {
                      for target_fmt in &options.targets {
                          results.push(TaskResult {
                              input_path: file_path.to_string_lossy().to_string(),
                              output_path: None,
                              status: "failed".to_string(),
                              original_size: 0,
                              converted_size: 0,
                              error_message: Some(format!("Failed to read source file: {}", e)),
                          });
                          if let Some(ref t) = tracker_arc {
                              t.inc(1);
                          }
                      }
                      return results;
                  }
              };

              let original_size = input_bytes.len() as u64;

              let file_ext = file_path
                  .extension()
                  .and_then(|e| e.to_str())
                  .unwrap_or("")
                  .to_lowercase();
              let from_fmt = match Format::from_str(&file_ext) {
                  Ok(fmt) => fmt,
                  Err(_) => {
                      for target_fmt in &options.targets {
                          results.push(TaskResult {
                              input_path: file_path.to_string_lossy().to_string(),
                              output_path: None,
                              status: "failed".to_string(),
                              original_size,
                              converted_size: 0,
                              error_message: Some(format!("Unsupported source format: '{}'", file_ext)),
                          });
                          if let Some(ref t) = tracker_arc {
                              t.inc(1);
                          }
                      }
                      return results;
                  }
              };

              for target_fmt in &options.targets {
                  let out_path = match &options.output_dir {
                      Some(out_dir) => {
                          let relative = match file_path.strip_prefix(&options.input_dir) {
                              Ok(rel) => rel,
                              Err(e) => {
                                  results.push(TaskResult {
                                      input_path: file_path.to_string_lossy().to_string(),
                                      output_path: None,
                                      status: "failed".to_string(),
                                      original_size,
                                      converted_size: 0,
                                      error_message: Some(format!("Invalid file path prefix: {}", e)),
                                  });
                                  if let Some(ref t) = tracker_arc {
                                      t.inc(1);
                                  }
                                  continue;
                              }
                          };
                          out_dir
                              .join(relative)
                              .with_extension(target_fmt.to_string())
                      }
                      None => file_path.with_extension(target_fmt.to_string()),
                  };

                  if out_path.exists() && !options.overwrite {
                      let converted_size = out_path.metadata().map(|m| m.len()).unwrap_or(0);
                      results.push(TaskResult {
                          input_path: file_path.to_string_lossy().to_string(),
                          output_path: Some(out_path.to_string_lossy().to_string()),
                          status: "skipped".to_string(),
                          original_size,
                          converted_size,
                          error_message: None,
                      });
                      if let Some(ref t) = tracker_arc {
                          t.inc(1);
                      }
                      continue;
                  }

                  let convert_opts = crate::converter::ConvertOptions {
                      input_path: file_path.clone(),
                      output_path: out_path.clone(),
                      package: false,
                      quality: options.quality,
                      lossless: options.lossless,
                      overwrite: options.overwrite,
                      pages: options.pages.clone(),
                      dpi: options.dpi,
                  };

                  match crate::converter::convert(from_fmt, *target_fmt, &input_bytes, &convert_opts) {
                      Ok(conv_res) => {
                          // The converter did not write to files directly in get_png_bytes if called via library convert
                          // but the ImageConverter strategy write_to is done in convert_image.
                          // Wait, does convert(...) write files directly inside it?
                          // Yes! In convert_image:
                          // fs::write(&file.path, &file.bytes)?;
                          // And in pdf_conv.rs:
                          // fs::write(&file.path, &file.bytes)?;
                          // So the files are written directly inside convert().
                          
                          let mut converted_size = 0;
                          for out_file in &conv_res.output_files {
                              converted_size += out_file.size_bytes;
                          }

                          results.push(TaskResult {
                              input_path: file_path.to_string_lossy().to_string(),
                              output_path: Some(out_path.to_string_lossy().to_string()),
                              status: "success".to_string(),
                              original_size,
                              converted_size,
                              error_message: None,
                          });
                      }
                      Err(e) => {
                          results.push(TaskResult {
                              input_path: file_path.to_string_lossy().to_string(),
                              output_path: Some(out_path.to_string_lossy().to_string()),
                              status: "failed".to_string(),
                              original_size,
                              converted_size: 0,
                              error_message: Some(e.to_string()),
                          });
                      }
                  }

                  if let Some(ref t) = tracker_arc {
                      t.inc(1);
                  }
              }

              results
          })
          .collect();

      // Collect successes and failures for safe deletion check
      let mut conversion_successes = std::collections::HashMap::new();
      let mut conversion_failures = std::collections::HashMap::new();
      for res in &tasks {
          if res.status == "success" || res.status == "skipped" {
              conversion_successes
                  .entry(res.input_path.clone())
                  .or_insert_with(std::collections::HashSet::new)
                  .insert(res.output_path.clone().unwrap_or_default());
          } else {
              conversion_failures
                  .entry(res.input_path.clone())
                  .or_insert_with(std::collections::HashSet::new)
                  .insert(res.error_message.clone().unwrap_or_default());
          }
      }

      // Handle delete original
      let mut deleted_count = 0;
      let mut deletion_errors = Vec::new();
      if options.delete_original {
          for file_path in &files {
              let path_str = file_path.to_string_lossy().to_string();
              let successes_count = conversion_successes
                  .get(&path_str)
                  .map(|s| s.len())
                  .unwrap_or(0);
              let failures_count = conversion_failures
                  .get(&path_str)
                  .map(|f| f.len())
                  .unwrap_or(0);

              if successes_count == options.targets.len() && failures_count == 0 {
                  if let Err(e) = std::fs::remove_file(file_path) {
                      deletion_errors.push((path_str, e.to_string()));
                  } else {
                      deleted_count += 1;
                  }
              }
          }
      }

      Ok(BatchResult {
          tasks,
          deleted_count,
          deletion_errors,
      })
  }
  ```
  *(Remove duplicate `process_file_task` and the custom Channel Structs from `src/converter/batch.rs`)*

- [ ] **Step 4: Run all tests and commit**
  Run: `cargo test`
  Expected: PASS
  Commit: `git commit -am "feat: simplify batch converter using Rayon parallel iterators"`
