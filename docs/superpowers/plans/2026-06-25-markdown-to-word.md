# Markdown to Word (DOCX) Conversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Markdown-to-DOCX format conversion in Martini CLI using native Rust crates.

**Architecture:** We configure Cargo dependencies, update `Format` enum and CLI parser routing inside the library, build a Markdown event-stream translator in `src/converter/md_conv.rs`, and write automated integration tests to verify correctness.

**Tech Stack:** Rust, clap, docx-rs, pulldown-cmark

## Global Constraints
- Target Rust version: Rust Edition 2024 (as defined in Cargo.toml).
- No external binary dependencies required (self-contained logic).

---

### Task 1: Dependency Configuration

**Files:**
- Modify: [Cargo.toml](file:///e:/Source/martini/Cargo.toml)

**Interfaces:**
- Consumes: None
- Produces: Adds crates `docx-rs` and `pulldown-cmark` for use by the compilation modules.

- [ ] **Step 1: Edit Cargo.toml to add docx-rs and pulldown-cmark**
  Add under `[dependencies]` in `Cargo.toml`:
  ```toml
  docx-rs = "0.4.15"
  pulldown-cmark = "0.12.1"
  ```
- [ ] **Step 2: Run Cargo check to download and build dependencies**
  Run: `cargo check`
  Expected: Success without compiling errors.
- [ ] **Step 3: Commit the dependency changes**
  ```bash
  git add Cargo.toml Cargo.lock
  git commit -m "deps: add docx-rs and pulldown-cmark"
  ```

---

### Task 2: Define formats and register in mod.rs

**Files:**
- Modify: [src/converter/mod.rs](file:///e:/Source/martini/src/converter/mod.rs)

**Interfaces:**
- Consumes: None
- Produces: `Format::Md`, `Format::Docx` enums, capacity list from `get_supported_formats()`.

- [ ] **Step 1: Add Md and Docx enum variants to Format**
  Modify `Format` enum in `src/converter/mod.rs` to include `Md` and `Docx`. Also update its `FromStr` and `Display` implementations:
  ```rust
  // In enum Format:
      Md,
      Docx,
  ```
  ```rust
  // In FromStr:
              "md" | "markdown" => Ok(Format::Md),
              "docx" => Ok(Format::Docx),
  ```
  ```rust
  // In Display:
              Format::Md => write!(f, "md"),
              Format::Docx => write!(f, "docx"),
  ```
- [ ] **Step 2: Register capability in get_supported_formats()**
  Add the following capability registration inside `get_supported_formats()`:
  ```rust
      let mut docx_params = std::collections::HashMap::new();
      docx_params.insert(
          "overwrite".to_string(),
          "boolean (default false)".to_string(),
      );
      list.push(FormatCapability {
          from: "md".to_string(),
          to: "docx".to_string(),
          description: "Convert Markdown documents to Microsoft Word (DOCX) files".to_string(),
          parameters: docx_params,
      });
  ```
- [ ] **Step 3: Verify build**
  Run: `cargo check`
  Expected: Success.
- [ ] **Step 4: Commit**
  ```bash
  git add src/converter/mod.rs
  git commit -m "feat: add Format::Md and Format::Docx variants and capability registration"
  ```

---

### Task 3: Implement Markdown compiler in md_conv.rs

**Files:**
- Create: `src/converter/md_conv.rs`

**Interfaces:**
- Consumes: `pulldown-cmark` parsing elements and `docx-rs` builder structs.
- Produces: `MarkdownConverter::convert_markdown(&self, input_data: &[u8], options: &ConvertOptions) -> Result<ConversionResult, MartiniError>`

- [ ] **Step 1: Create new md_conv.rs file with compilation logic**
  Write the following content to `src/converter/md_conv.rs`:
  ```rust
  use crate::converter::{ConversionResult, ConvertOptions, Format, OutputFileMetadata};
  use crate::error::MartiniError;
  use docx_rs::*;
  use pulldown_cmark::{Event, Parser, Tag, TagEnd, Options};

  pub struct MarkdownConverter;

  #[derive(Debug, Clone)]
  enum ParaElement {
      Run(Run),
      Link(Hyperlink),
  }

  #[derive(Debug, Clone)]
  struct ParagraphState {
      elements: Vec<ParaElement>,
      heading_level: Option<usize>,
      is_blockquote: bool,
      is_code_block: bool,
      is_list_item: bool,
      list_indent: usize,
  }

  #[derive(Debug, Clone)]
  struct ListState {
      is_ordered: bool,
      counter: u32,
  }

  struct TableState {
      rows: Vec<TableRow>,
      current_row_cells: Vec<TableCell>,
      alignments: Vec<pulldown_cmark::Alignment>,
  }

  struct StyleState {
      bold: bool,
      italic: bool,
      strikethrough: bool,
      is_code: bool,
      link: Option<String>,
  }

  impl MarkdownConverter {
      pub fn convert_markdown(
          &self,
          input_data: &[u8],
          options: &ConvertOptions,
      ) -> Result<ConversionResult, MartiniError> {
          let markdown_str = std::str::from_utf8(input_data).map_err(|e| {
              MartiniError::InvalidInputData {
                  reason: format!("Input is not valid UTF-8: {}", e),
              }
          })?;

          let mut parser_options = Options::empty();
          parser_options.insert(Options::ENABLE_TABLES);
          parser_options.insert(Options::ENABLE_STRIKETHROUGH);
          parser_options.insert(Options::ENABLE_TASKLISTS);
          
          let parser = Parser::new_ext(markdown_str, parser_options);
          
          let mut doc = Docx::new();
          
          let mut style = StyleState {
              bold: false,
              italic: false,
              strikethrough: false,
              is_code: false,
              link: None,
          };
          
          let mut current_para: Option<ParagraphState> = None;
          let mut list_stack: Vec<ListState> = Vec::new();
          let mut table_state: Option<TableState> = None;
          let mut in_code_block = false;
          
          for event in parser {
              match event {
                  Event::Start(tag) => match tag {
                      Tag::Paragraph => {
                          current_para = Some(ParagraphState {
                              elements: Vec::new(),
                              heading_level: None,
                              is_blockquote: !list_stack.is_empty() && current_para.as_ref().map_or(false, |p| p.is_blockquote),
                              is_code_block: false,
                              is_list_item: false,
                              list_indent: list_stack.len(),
                          });
                      }
                      Tag::Heading { level, .. } => {
                          let lvl_num = match level {
                              pulldown_cmark::HeadingLevel::H1 => 1,
                              pulldown_cmark::HeadingLevel::H2 => 2,
                              pulldown_cmark::HeadingLevel::H3 => 3,
                              pulldown_cmark::HeadingLevel::H4 => 4,
                              pulldown_cmark::HeadingLevel::H5 => 5,
                              pulldown_cmark::HeadingLevel::H6 => 6,
                          };
                          current_para = Some(ParagraphState {
                              elements: Vec::new(),
                              heading_level: Some(lvl_num),
                              is_blockquote: false,
                              is_code_block: false,
                              is_list_item: false,
                              list_indent: 0,
                          });
                      }
                      Tag::BlockQuote(_kind) => {
                          if let Some(ref mut p) = current_para {
                              p.is_blockquote = true;
                          } else {
                              current_para = Some(ParagraphState {
                                  elements: Vec::new(),
                                  heading_level: None,
                                  is_blockquote: true,
                                  is_code_block: false,
                                  is_list_item: false,
                                  list_indent: 0,
                              });
                          }
                      }
                      Tag::List(start) => {
                          list_stack.push(ListState {
                              is_ordered: start.is_some(),
                              counter: start.unwrap_or(1) as u32,
                          });
                      }
                      Tag::Item => {
                          let indent = list_stack.len();
                          let prefix = if let Some(state) = list_stack.last_mut() {
                              if state.is_ordered {
                                  let val = format!("{}.  ", state.counter);
                                  state.counter += 1;
                                  val
                              } else {
                                  "•  ".to_string()
                              }
                          } else {
                              "•  ".to_string()
                          };
                          let mut elements = Vec::new();
                          elements.push(ParaElement::Run(Run::new().add_text(prefix)));
                          current_para = Some(ParagraphState {
                              elements,
                              heading_level: None,
                              is_blockquote: false,
                              is_code_block: false,
                              is_list_item: true,
                              list_indent: indent,
                          });
                      }
                      Tag::Table(alignments) => {
                          table_state = Some(TableState {
                              rows: Vec::new(),
                              current_row_cells: Vec::new(),
                              alignments,
                          });
                      }
                      Tag::TableHead => {}
                      Tag::TableRow => {}
                      Tag::TableCell => {
                          current_para = Some(ParagraphState {
                              elements: Vec::new(),
                              heading_level: None,
                              is_blockquote: false,
                              is_code_block: false,
                              is_list_item: false,
                              list_indent: 0,
                          });
                      }
                      Tag::Strong => style.bold = true,
                      Tag::Emphasis => style.italic = true,
                      Tag::Strikethrough => style.strikethrough = true,
                      Tag::Link { dest_url, .. } => style.link = Some(dest_url.to_string()),
                      Tag::Image { dest_url, title, .. } => {
                          let text = if title.is_empty() {
                              format!("[Image: {}]", dest_url)
                          } else {
                              format!("[Image: {} - {}]", title, dest_url)
                          };
                          if let Some(ref mut p) = current_para {
                              p.elements.push(ParaElement::Run(Run::new().add_text(text).italic().color("888888")));
                          }
                      }
                      Tag::CodeBlock(_) => {
                          in_code_block = true;
                          current_para = Some(ParagraphState {
                              elements: Vec::new(),
                              heading_level: None,
                              is_blockquote: false,
                              is_code_block: true,
                              list_indent: 0,
                              is_list_item: false,
                          });
                      }
                      _ => {}
                  },
                  Event::End(tag_end) => match tag_end {
                      TagEnd::Paragraph | TagEnd::Heading => {
                          if let Some(p) = current_para.take() {
                              let docx_p = build_paragraph(p);
                              if let Some(ref mut ts) = table_state {
                                  let cell = TableCell::new().add_paragraph(docx_p);
                                  ts.current_row_cells.push(cell);
                              } else {
                                  doc = doc.add_paragraph(docx_p);
                              }
                          }
                      }
                      TagEnd::BlockQuote => {}
                      TagEnd::List => {
                          list_stack.pop();
                      }
                      TagEnd::Item => {
                          if let Some(p) = current_para.take() {
                              let docx_p = build_paragraph(p);
                              doc = doc.add_paragraph(docx_p);
                          }
                      }
                      TagEnd::Table => {
                          if let Some(ts) = table_state.take() {
                              let border = TableBorder::new(BorderType::Single).size(4).color("CCCCCC");
                              let borders = TableBorders::new()
                                  .set(BorderPosition::Top, border.clone())
                                  .set(BorderPosition::Bottom, border.clone())
                                  .set(BorderPosition::Left, border.clone())
                                  .set(BorderPosition::Right, border.clone())
                                  .set(BorderPosition::InsideH, border.clone())
                                  .set(BorderPosition::InsideV, border.clone());
                              let table = Table::new(ts.rows).set_borders(borders);
                              doc = doc.add_table(table);
                          }
                      }
                      TagEnd::TableHead => {
                          if let Some(ref mut ts) = table_state {
                              let cells = std::mem::take(&mut ts.current_row_cells);
                              ts.rows.push(TableRow::new(cells));
                          }
                      }
                      TagEnd::TableRow => {
                          if let Some(ref mut ts) = table_state {
                              let cells = std::mem::take(&mut ts.current_row_cells);
                              ts.rows.push(TableRow::new(cells));
                          }
                      }
                      TagEnd::TableCell => {
                          if let Some(p) = current_para.take() {
                              let docx_p = build_paragraph(p);
                              if let Some(ref mut ts) = table_state {
                                  let cell = TableCell::new().add_paragraph(docx_p);
                                  ts.current_row_cells.push(cell);
                              }
                          } else {
                              if let Some(ref mut ts) = table_state {
                                  ts.current_row_cells.push(TableCell::new());
                              }
                          }
                      }
                      TagEnd::Strong => style.bold = false,
                      TagEnd::Emphasis => style.italic = false,
                      TagEnd::Strikethrough => style.strikethrough = false,
                      TagEnd::Link => style.link = None,
                      TagEnd::CodeBlock => {
                          in_code_block = false;
                          if let Some(p) = current_para.take() {
                              let docx_p = build_paragraph(p);
                              doc = doc.add_paragraph(docx_p);
                          }
                      }
                      _ => {}
                  },
                  Event::Text(text) => {
                      let mut run = Run::new().add_text(text.to_string());
                      if in_code_block {
                          run = run.color("333333");
                      } else {
                          if style.bold { run = run.bold(); }
                          if style.italic { run = run.italic(); }
                          if style.strikethrough { run = run.strike(); }
                          if style.is_code { run = run.color("A71D5D"); }
                      }
                      
                      if let Some(ref mut p) = current_para {
                          if let Some(ref url) = style.link {
                              let hl = Hyperlink::new(url, "External").add_run(run.color("0000FF").underline("single"));
                              p.elements.push(ParaElement::Link(hl));
                          } else {
                              p.elements.push(ParaElement::Run(run));
                          }
                      }
                  }
                  Event::Code(code) => {
                      let run = Run::new().add_text(code.to_string()).color("A71D5D");
                      if let Some(ref mut p) = current_para {
                          if let Some(ref url) = style.link {
                              let hl = Hyperlink::new(url, "External").add_run(run.underline("single"));
                              p.elements.push(ParaElement::Link(hl));
                          } else {
                              p.elements.push(ParaElement::Run(run));
                          }
                      }
                  }
                  Event::SoftBreak | Event::HardBreak => {
                      if let Some(ref mut p) = current_para {
                          p.elements.push(ParaElement::Run(Run::new().add_break(BreakType::TextWrapping)));
                      }
                  }
                  _ => {}
              }
          }
          
          let output_file = std::fs::File::create(&options.output_path).map_err(|e| {
              MartiniError::OutputWrite {
                  reason: format!("Failed to create output file: {}", e),
              }
          })?;
          
          doc.build().pack(output_file).map_err(|e| {
              MartiniError::OutputWrite {
                  reason: format!("Failed to pack DOCX: {}", e),
              }
          })?;
          
          let size = options.output_path.metadata().map(|m| m.len()).unwrap_or(0);
          
          Ok(ConversionResult {
              from: Format::Md,
              to: Format::Docx,
              output_files: vec![OutputFileMetadata {
                  path: options.output_path.to_string_lossy().to_string(),
                  size_bytes: size,
                  description: "Converted from Markdown".to_string(),
              }],
          })
      }
  }

  fn build_paragraph(p: ParagraphState) -> Paragraph {
      let mut docx_p = Paragraph::new();
      
      if p.is_blockquote {
          docx_p = docx_p.indent(Some(720), None, None, None);
      }
      
      if p.is_list_item {
          let left_indent = (p.list_indent as i32) * 360;
          docx_p = docx_p.indent(Some(left_indent), None, None, None);
      }
      
      for elem in p.elements {
          match elem {
              ParaElement::Run(mut r) => {
                  if let Some(level) = p.heading_level {
                      let size = match level {
                          1 => 36,
                          2 => 32,
                          3 => 28,
                          4 => 24,
                          5 => 20,
                          _ => 18,
                      };
                      r = r.bold().size(size);
                  }
                  if p.is_blockquote {
                      r = r.italic().color("555555");
                  }
                  docx_p = docx_p.add_run(r);
              }
              ParaElement::Link(l) => {
                  docx_p = docx_p.add_hyperlink(l);
              }
          }
      }
      
      docx_p
  }
  ```
- [ ] **Step 2: Add md_conv module declaration to mod.rs**
  Add `pub mod md_conv;` under `pub mod pdf_conv;` in `src/converter/mod.rs`.
- [ ] **Step 3: Run Cargo check to make sure the md_conv module compiles**
  Run: `cargo check`
  Expected: Successful compilation.
- [ ] **Step 4: Commit**
  ```bash
  git add src/converter/md_conv.rs src/converter/mod.rs
  git commit -m "feat: implement MarkdownConverter and register md_conv module"
  ```

---

### Task 4: Add routing in mod.rs

**Files:**
- Modify: [src/converter/mod.rs](file:///e:/Source/martini/src/converter/mod.rs)

**Interfaces:**
- Consumes: `md_conv::MarkdownConverter`
- Produces: Dispatches MD->DOCX conversion from the generic `convert` function.

- [ ] **Step 1: Modify convert() function to route Md->Docx conversions**
  Modify `convert` function in `src/converter/mod.rs` to route from `Format::Md` to `Format::Docx`:
  ```rust
  pub fn convert(
      from: Format,
      to: Format,
      input_data: &[u8],
      options: &ConvertOptions,
  ) -> Result<ConversionResult, MartiniError> {
      if to == Format::Pdf && from != Format::Pdf {
          return Err(MartiniError::UnsupportedConversion {
              from: from.to_string(),
              to: to.to_string(),
          });
      }

      if from == Format::Md && to == Format::Docx {
          let converter = md_conv::MarkdownConverter;
          converter.convert_markdown(input_data, options)
      } else if from == Format::Pdf {
          let converter = pdf_conv::PdfConverter;
          converter.convert_pdf(to, input_data, options)
      } else {
          let converter = image_conv::ImageConverter;
          converter.convert_image(from, to, input_data, options)
      }
  }
  ```
- [ ] **Step 2: Run Cargo check to verify compiling**
  Run: `cargo check`
  Expected: Successful compilation.
- [ ] **Step 3: Commit**
  ```bash
  git add src/converter/mod.rs
  git commit -m "feat: route Format::Md to Format::Docx in convert()"
  ```

---

### Task 5: Integrate with CLI in main.rs

**Files:**
- Modify: [src/main.rs](file:///e:/Source/martini/src/main.rs)

**Interfaces:**
- Consumes: `Format::Md`, `Format::Docx`
- Produces: CLI options correctly resolve auto-detected format for Markdown, and ListFormats lists correct options.

- [ ] **Step 1: Update main.rs resolution of file format auto-detect**
  Update `run` in `src/main.rs` to resolve `.md` and `.markdown` to `Format::Md` and default to `Format::Docx`.
  Specifically, look at lines 141-147 in `main.rs`:
  ```rust
                          if is_svg {
                              "favicon".to_string()
                          } else if is_pdf {
                              "png".to_string()
                          } else {
                              "webp".to_string()
                          }
  ```
  Change this check to account for markdown inputs:
  ```rust
              let is_md = input
                  .extension()
                  .and_then(|e| e.to_str())
                  .map(|s| {
                      let ext = s.to_lowercase();
                      ext == "md" || ext == "markdown"
                  })
                  .unwrap_or(false);
  ```
  And then in target format auto-resolution, if `is_md` is true, use `"docx"`.
  Also, update the format listing parameters in `Commands::ListFormats` match statement (line 76):
  ```rust
                      let options_str = match f.to.as_str() {
                          "favicon" => ". Options: --package",
                          "png" => "",
                          "jpg" => ". Options: --quality",
                          "webp" | "avif" | "both" => ". Options: --quality, --lossless",
                          "docx" => ". Options: --overwrite",
                          _ if f.from == "pdf" => ". Options: --pages, --dpi, --quality, --lossless",
                          _ => "",
                      };
  ```
- [ ] **Step 2: Run Cargo check to verify compiler succeeds**
  Run: `cargo check`
  Expected: Compilation passes.
- [ ] **Step 3: Commit CLI integration**
  ```bash
  git add src/main.rs
  git commit -m "feat: integrate Markdown to Word defaults in CLI"
  ```

---

### Task 6: Write integration test and verify

**Files:**
- Create: `tests/fixtures/sample.md`
- Modify: [tests/cli.rs](file:///e:/Source/martini/tests/cli.rs)

**Interfaces:**
- Consumes: CLI binary and test fixtures.
- Produces: Passing integration tests.

- [ ] **Step 1: Create tests/fixtures/sample.md**
  Write a markdown file with multiple headings, paragraphs, strong/em elements, lists, and a table to `tests/fixtures/sample.md`:
  ```markdown
  # Test Heading 1
  ## Test Heading 2

  This is a paragraph with **bold** and *italic* text.

  > This is a blockquote.

  - Unordered item 1
  - Unordered item 2

  1. Ordered item 1
  2. Ordered item 2

  | Col 1 | Col 2 |
  | --- | --- |
  | Cell A | Cell B |
  ```
- [ ] **Step 2: Add integration test case in tests/cli.rs**
  Add a new test inside `tests/cli.rs` that calls CLI:
  ```rust
  #[test]
  fn test_markdown_to_docx_conversion() {
      let temp = tempfile::tempdir().unwrap();
      let output_file = temp.path().join("output.docx");

      let mut cmd = std::process::Command::cargo_bin("martini").unwrap();
      cmd.arg("convert")
          .arg("--input")
          .arg("tests/fixtures/sample.md")
          .arg("--output")
          .arg(&output_file);

      let assert = cmd.assert().success();
      
      assert!(output_file.exists());
      assert!(output_file.metadata().unwrap().len() > 0);
  }
  ```
- [ ] **Step 3: Run cargo test to verify**
  Run: `cargo test`
  Expected: All tests pass.
- [ ] **Step 4: Commit and finalize**
  ```bash
  git add tests/fixtures/sample.md tests/cli.rs
  git commit -m "test: add integration test for markdown to docx conversion"
  ```
