use crate::converter::typst_engine::compile_typst_to_pdf;
use crate::converter::{ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::fs;
use std::path::Path;

pub fn convert_markdown_to_pdf(
    input_data: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    let markdown_str =
        std::str::from_utf8(input_data).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Input is not valid UTF-8: {}", e),
        })?;

    let base_dir = options
        .input_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    let typst_markup = translate_md_to_typst(markdown_str, options);
    let pdf_bytes = compile_typst_to_pdf(&typst_markup, base_dir)?;

    if let Some(parent) = options
        .output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    fs::write(&options.output_path, &pdf_bytes)?;

    let size = options
        .output_path
        .metadata()
        .map(|m| m.len())
        .unwrap_or(pdf_bytes.len() as u64);

    Ok(ConversionResult {
        from: Format::Md,
        to: Format::Pdf,
        output_files: vec![OutputFileMetadata {
            path: options.output_path.to_string_lossy().to_string(),
            size_bytes: size,
            description: "Converted from Markdown to PDF".to_string(),
        }],
    })
}

pub fn translate_md_to_typst(markdown: &str, options: &ConvertOptions) -> String {
    let mut typst = String::new();

    // 1. Setup Document Header & Styling
    typst.push_str(&generate_document_preamble(options));

    // 2. Insert TOC if requested
    if options.toc {
        typst.push_str("#outline(indent: auto)\n#v(1.5em)\n");
    }

    // 3. Parse Markdown Events
    let mut parser_options = Options::empty();
    parser_options.insert(Options::ENABLE_TABLES);
    parser_options.insert(Options::ENABLE_STRIKETHROUGH);
    parser_options.insert(Options::ENABLE_TASKLISTS);
    parser_options.insert(Options::ENABLE_MATH);
    parser_options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, parser_options);

    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_content = String::new();

    let mut list_depth: usize = 0;
    let mut ordered_list_stack = Vec::new();

    let mut table_alignments = Vec::new();
    let mut table_cells = Vec::new();
    let mut in_table_head = false;
    let mut table_header_cells = Vec::new();
    let mut current_cell_content = String::new();
    let mut in_table = false;

    let mut link_url = None;
    let mut link_text = String::new();

    let mut image_url = None;
    let mut image_alt = String::new();

    macro_rules! target_buf {
        () => {
            if in_table {
                &mut current_cell_content
            } else if link_url.is_some() {
                &mut link_text
            } else {
                &mut typst
            }
        };
    }

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let level_num = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    typst.push('\n');
                    for _ in 0..level_num {
                        typst.push('=');
                    }
                    typst.push(' ');
                }
                Tag::Paragraph => {
                    if !in_table && list_depth == 0 {
                        typst.push('\n');
                    }
                }
                Tag::BlockQuote(_kind) => {
                    typst.push_str("\n#quote(block: true)[\n");
                }
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    code_lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                    code_content.clear();
                }
                Tag::List(start) => {
                    list_depth += 1;
                    ordered_list_stack.push(start.is_some());
                }
                Tag::Item => {
                    typst.push('\n');
                    for _ in 1..list_depth {
                        typst.push_str("  ");
                    }
                    if ordered_list_stack.last().copied().unwrap_or(false) {
                        typst.push_str("+ ");
                    } else {
                        typst.push_str("- ");
                    }
                }
                Tag::Table(alignments) => {
                    in_table = true;
                    table_alignments = alignments;
                    table_cells.clear();
                    table_header_cells.clear();
                }
                Tag::TableHead => {
                    in_table_head = true;
                }
                Tag::TableRow => {}
                Tag::TableCell => {
                    current_cell_content.clear();
                }
                Tag::Strong => target_buf!().push('*'),
                Tag::Emphasis => target_buf!().push('_'),
                Tag::Strikethrough => target_buf!().push_str("#strike["),
                Tag::Link { dest_url, .. } => {
                    link_url = Some(dest_url.to_string());
                    link_text.clear();
                }
                Tag::Image { dest_url, .. } => {
                    image_url = Some(dest_url.to_string());
                    image_alt.clear();
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    typst.push('\n');
                }
                TagEnd::Paragraph => {
                    if !in_table && list_depth == 0 {
                        typst.push('\n');
                    }
                }
                TagEnd::BlockQuote(_) => {
                    typst.push_str("\n]\n");
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    typst.push_str("\n```");
                    if !code_lang.is_empty() {
                        typst.push_str(&code_lang);
                    }
                    typst.push('\n');
                    typst.push_str(&code_content);
                    if !code_content.ends_with('\n') {
                        typst.push('\n');
                    }
                    typst.push_str("```\n");
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    ordered_list_stack.pop();
                    if list_depth == 0 {
                        typst.push('\n');
                    }
                }
                TagEnd::Item => {}
                TagEnd::TableHead => {
                    in_table_head = false;
                }
                TagEnd::TableRow => {}
                TagEnd::TableCell => {
                    let cell_str = current_cell_content.trim().to_string();
                    if in_table_head {
                        table_header_cells.push(cell_str);
                    } else {
                        table_cells.push(cell_str);
                    }
                }
                TagEnd::Table => {
                    in_table = false;
                    typst.push_str(&format_typst_table(
                        &table_alignments,
                        &table_header_cells,
                        &table_cells,
                    ));
                }
                TagEnd::Strong => target_buf!().push('*'),
                TagEnd::Emphasis => target_buf!().push('_'),
                TagEnd::Strikethrough => target_buf!().push(']'),
                TagEnd::Link => {
                    if let Some(url) = link_url.take() {
                        let link_code =
                            format!("#link(\"{}\")[{}]", escape_typst_string(&url), link_text);
                        if in_table {
                            current_cell_content.push_str(&link_code);
                        } else {
                            typst.push_str(&link_code);
                        }
                    }
                }
                TagEnd::Image => {
                    if let Some(url) = image_url.take() {
                        let url_normalized = url.replace('\\', "/");
                        let img_code = if image_alt.is_empty() {
                            format!(
                                "\n#align(center)[#image(\"{}\")]\n",
                                escape_typst_string(&url_normalized)
                            )
                        } else {
                            format!(
                                "\n#figure(image(\"{}\"), caption: [{}])\n",
                                escape_typst_string(&url_normalized),
                                image_alt
                            )
                        };
                        if in_table {
                            current_cell_content.push_str(&format!(
                                "#image(\"{}\")",
                                escape_typst_string(&url_normalized)
                            ));
                        } else {
                            typst.push_str(&img_code);
                        }
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    code_content.push_str(&text);
                } else if image_url.is_some() && !in_table && link_url.is_none() {
                    image_alt.push_str(&escape_typst_content(&text));
                } else {
                    target_buf!().push_str(&escape_typst_content(&text));
                }
            }
            Event::Code(code) => {
                target_buf!().push_str(&format!("`{}`", code.replace('`', "\\`")));
            }
            Event::InlineMath(math) => {
                target_buf!().push_str(&format!("${}$", latex_to_typst_math(&math)));
            }
            Event::DisplayMath(math) => {
                let translated_math = latex_to_typst_math(&math);
                typst.push_str(&format!("\n$ {} $\n", translated_math.trim()));
            }
            Event::SoftBreak => {
                target_buf!().push(if in_table || link_url.is_some() {
                    ' '
                } else {
                    '\n'
                });
            }
            Event::HardBreak => {
                target_buf!().push_str(if in_table || link_url.is_some() {
                    " \\ "
                } else {
                    " \\\n"
                });
            }
            Event::Rule => {
                typst.push_str("\n#line(length: 100%, stroke: 0.5pt + luma(180))\n");
            }
            Event::TaskListMarker(checked) => {
                typst.push_str(if checked { "[x] " } else { "[ ] " });
            }
            _ => {}
        }
    }

    typst
}

fn generate_document_preamble(options: &ConvertOptions) -> String {
    let mut preamble = String::new();

    // Paper size
    let paper = match options
        .paper_size
        .as_deref()
        .unwrap_or("a4")
        .to_lowercase()
        .as_str()
    {
        "letter" | "us-letter" => "us-letter",
        "a3" => "a3",
        "a5" => "a5",
        _ => "a4",
    };

    let margin = options.margin.as_deref().unwrap_or("20mm");

    // Fonts setup
    let font_list = if let Some(ref custom_font) = options.font_family {
        format!(
            "(\"{}\", \"Microsoft YaHei\", \"PingFang SC\", \"Noto Sans CJK SC\", \"SimSun\", \"Arial\")",
            custom_font
        )
    } else {
        "(\"Segoe UI\", \"Microsoft YaHei\", \"PingFang SC\", \"Noto Sans CJK SC\", \"SimSun\", \"Helvetica Neue\", \"Arial\")".to_string()
    };

    // Header and Footer setup
    let header_expr = if let Some(ref h) = options.header {
        format!(
            "align(right)[#text(size: 8.5pt, fill: luma(120))[{}]]",
            escape_typst_content(h)
        )
    } else {
        "none".to_string()
    };

    let footer_expr = if options.no_page_numbers {
        if let Some(ref f) = options.footer {
            format!(
                "align(center)[#text(size: 8.5pt, fill: luma(120))[{}]]",
                escape_typst_content(f)
            )
        } else {
            "none".to_string()
        }
    } else if let Some(ref f) = options.footer {
        format!(
            "grid(columns: (1fr, 1fr), align: (left, right), text(size: 8.5pt, fill: luma(120))[{}], text(size: 8.5pt, fill: luma(120))[#context counter(page).display(\"1 / 1\", both: true)])",
            escape_typst_content(f)
        )
    } else {
        "align(center)[#text(size: 8.5pt, fill: luma(120))[#context counter(page).display(\"1 / 1\", both: true)]]".to_string()
    };

    preamble.push_str(&format!(
        "#set page(paper: \"{}\", margin: {}, header: {}, footer: {})\n",
        paper, margin, header_expr, footer_expr
    ));

    preamble.push_str(&format!(
        "#set text(font: {}, size: 10.5pt, lang: \"zh\")\n",
        font_list
    ));
    preamble.push_str("#set par(justify: true, leading: 0.75em)\n");

    // Theme specific styles
    let theme = options.theme.as_deref().unwrap_or("github").to_lowercase();
    match theme.as_str() {
        "academic" => {
            preamble.push_str("#set heading(numbering: \"1.1 \")\n");
            preamble.push_str("#show heading: it => block(spacing: 1.2em, text(weight: \"bold\", it.body))\n");
            preamble.push_str("#show raw.where(block: true): block.with(fill: rgb(\"#f9fafb\"), inset: 8pt, radius: 2pt, stroke: 0.5pt + luma(200), width: 100%)\n");
            preamble.push_str("#show raw.where(block: false): box.with(fill: rgb(\"#f3f4f6\"), inset: (x: 2pt, y: 0pt), outset: (y: 2pt), radius: 2pt)\n");
        }
        "modern" => {
            preamble.push_str("#show heading.where(level: 1): it => block(spacing: 1.2em, text(fill: rgb(\"#1d4ed8\"), size: 1.5em, weight: \"bold\", it.body))\n");
            preamble.push_str("#show heading.where(level: 2): it => block(spacing: 1em, text(fill: rgb(\"#2563eb\"), size: 1.25em, weight: \"bold\", it.body))\n");
            preamble.push_str("#show raw.where(block: true): block.with(fill: rgb(\"#eff6ff\"), inset: 10pt, radius: 6pt, stroke: 0.5pt + rgb(\"#bfdbfe\"), width: 100%)\n");
            preamble.push_str("#show raw.where(block: false): box.with(fill: rgb(\"#eff6ff\"), inset: (x: 3pt, y: 0pt), outset: (y: 3pt), radius: 3pt)\n");
            preamble.push_str("#show quote: it => block(fill: rgb(\"#f8fafc\"), inset: 10pt, stroke: (left: 3pt + rgb(\"#3b82f6\")), radius: (right: 4pt), it.body)\n");
        }
        "minimal" => {
            preamble.push_str("#show heading: it => block(spacing: 1em, text(weight: \"bold\", it.body))\n");
            preamble.push_str("#show raw.where(block: true): block.with(inset: 8pt, stroke: 0.5pt + luma(220), width: 100%)\n");
            preamble.push_str("#show quote: it => block(stroke: (left: 2pt + luma(180)), inset: (left: 8pt), it.body)\n");
        }
        _ /* github */ => {
            preamble.push_str("#show heading.where(level: 1): it => block(spacing: 1.2em, [ #text(size: 1.5em, weight: \"bold\", it.body) #v(0.3em) #line(length: 100%, stroke: 0.5pt + luma(210)) ])\n");
            preamble.push_str("#show heading.where(level: 2): it => block(spacing: 1em, [ #text(size: 1.3em, weight: \"bold\", it.body) #v(0.2em) #line(length: 100%, stroke: 0.5pt + luma(220)) ])\n");
            preamble.push_str("#show raw.where(block: true): block.with(fill: rgb(\"#f6f8fa\"), inset: 10pt, radius: 4pt, stroke: 0.5pt + rgb(\"#d0d7de\"), width: 100%)\n");
            preamble.push_str("#show raw.where(block: false): box.with(fill: rgb(\"#f6f8fa\"), inset: (x: 3pt, y: 0pt), outset: (y: 3pt), radius: 3pt)\n");
            preamble.push_str("#show quote: it => block(stroke: (left: 3pt + rgb(\"#d0d7de\")), inset: (left: 10pt), text(fill: luma(80), it.body))\n");
        }
    }

    preamble.push('\n');
    preamble
}

fn format_typst_table(alignments: &[Alignment], headers: &[String], cells: &[String]) -> String {
    let col_count = if !headers.is_empty() {
        headers.len()
    } else if !alignments.is_empty() {
        alignments.len()
    } else {
        1
    };

    let mut align_strs = Vec::new();
    for a in alignments {
        match a {
            Alignment::Left => align_strs.push("left"),
            Alignment::Center => align_strs.push("center"),
            Alignment::Right => align_strs.push("right"),
            Alignment::None => align_strs.push("left"),
        }
    }
    while align_strs.len() < col_count {
        align_strs.push("left");
    }

    let mut out = String::new();
    out.push_str("\n#align(center)[\n");
    out.push_str(&format!(
        "#table(\n  columns: {},\n  align: ({}),\n  stroke: 0.5pt + luma(200),\n  fill: (col, row) => if row == 0 {{ rgb(\"#f6f8fa\") }} else {{ none }},\n",
        col_count,
        align_strs.join(", ")
    ));

    if !headers.is_empty() {
        out.push_str("  table.header(\n");
        for h in headers {
            out.push_str(&format!("    [*{}*],\n", h));
        }
        out.push_str("  ),\n");
    }

    for chunk in cells.chunks(col_count) {
        for cell in chunk {
            out.push_str(&format!("  [{}],\n", cell));
        }
    }

    out.push_str(")\n]\n");
    out
}

fn escape_typst_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '@' => out.push_str("\\@"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_typst_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn latex_to_typst_math(latex: &str) -> String {
    let mut s = latex.trim().to_string();

    // 1. Remove \left and \right
    s = s.replace("\\left", "").replace("\\right", "");

    // 2. Translate \frac{a}{b} -> ((a) / (b))
    while let Some(frac_pos) = s.find("\\frac") {
        if let Some((num, den, end_pos)) = extract_two_braced_args(&s[frac_pos + 5..]) {
            let replacement = format!("(({}) / ({}))", num, den);
            s = format!(
                "{}{}{}",
                &s[..frac_pos],
                replacement,
                &s[frac_pos + 5 + end_pos..]
            );
        } else {
            break;
        }
    }

    // 3. Translate \sqrt[n]{x} and \sqrt{x}
    while let Some(sqrt_pos) = s.find("\\sqrt") {
        let rest = s[sqrt_pos + 5..].trim_start();
        if rest.starts_with('[')
            && let Some(bracket_end) = rest.find(']')
        {
            let root_deg = &rest[1..bracket_end];
            let after_deg = rest[bracket_end + 1..].trim_start();
            if let Some((body, end_pos)) = extract_braced_arg(after_deg) {
                let replacement = format!("root({}, {})", root_deg, body);
                let total_consumed = (s.len() - sqrt_pos) - (after_deg[end_pos..].len());
                s = format!(
                    "{}{}{}",
                    &s[..sqrt_pos],
                    replacement,
                    &s[sqrt_pos + total_consumed..]
                );
                continue;
            }
        }
        if let Some((body, end_pos)) = extract_braced_arg(&s[sqrt_pos + 5..]) {
            let replacement = format!("sqrt({})", body);
            s = format!(
                "{}{}{}",
                &s[..sqrt_pos],
                replacement,
                &s[sqrt_pos + 5 + end_pos..]
            );
        } else {
            break;
        }
    }

    // 4. Translate \sum, \prod, \int
    s = s
        .replace("\\sum", "sum")
        .replace("\\prod", "product")
        .replace("\\int", "integral")
        .replace("\\iint", "integral.double")
        .replace("\\iiint", "integral.triple")
        .replace("\\oint", "integral.cont");

    // 5. Translate common operators & symbols
    let replacements = [
        ("\\times", " times "),
        ("\\cdot", " dot "),
        ("\\div", " div "),
        ("\\pm", " plus.minus "),
        ("\\mp", " minus.plus "),
        ("\\approx", " approx "),
        ("\\equiv", " equiv "),
        ("\\sim", " tilde "),
        ("\\ne", " != "),
        ("\\neq", " != "),
        ("\\le", " <= "),
        ("\\leq", " <= "),
        ("\\ge", " >= "),
        ("\\geq", " >= "),
        ("\\ll", " << "),
        ("\\gg", " >> "),
        ("\\infty", " oo "),
        ("\\partial", " diff "),
        ("\\nabla", " nabla "),
        ("\\forall", " forall "),
        ("\\exists", " exists "),
        ("\\in", " in "),
        ("\\notin", " in.not "),
        ("\\subset", " subset "),
        ("\\subseteq", " subset.eq "),
        ("\\supset", " supset "),
        ("\\supseteq", " supset.eq "),
        ("\\cup", " union "),
        ("\\cap", " sect "),
        ("\\to", " -> "),
        ("\\rightarrow", " -> "),
        ("\\leftarrow", " <- "),
        ("\\Rightarrow", " => "),
        ("\\Leftarrow", " <= "),
        ("\\leftrightarrow", " <-> "),
        ("\\Leftrightarrow", " <=> "),
        ("\\alpha", " alpha "),
        ("\\beta", " beta "),
        ("\\gamma", " gamma "),
        ("\\delta", " delta "),
        ("\\epsilon", " epsilon "),
        ("\\varepsilon", " epsilon.alt "),
        ("\\zeta", " zeta "),
        ("\\eta", " eta "),
        ("\\theta", " theta "),
        ("\\vartheta", " theta.alt "),
        ("\\iota", " iota "),
        ("\\kappa", " kappa "),
        ("\\lambda", " lambda "),
        ("\\mu", " mu "),
        ("\\nu", " nu "),
        ("\\xi", " xi "),
        ("\\pi", " pi "),
        ("\\rho", " rho "),
        ("\\sigma", " sigma "),
        ("\\tau", " tau "),
        ("\\upsilon", " upsilon "),
        ("\\phi", " phi "),
        ("\\varphi", " phi.alt "),
        ("\\chi", " chi "),
        ("\\psi", " psi "),
        ("\\omega", " omega "),
        ("\\Delta", " Delta "),
        ("\\Gamma", " Gamma "),
        ("\\Theta", " Theta "),
        ("\\Lambda", " Lambda "),
        ("\\Xi", " Xi "),
        ("\\Pi", " Pi "),
        ("\\Sigma", " Sigma "),
        ("\\Upsilon", " Upsilon "),
        ("\\Phi", " Phi "),
        ("\\Psi", " Psi "),
        ("\\Omega", " Omega "),
        ("\\{", "{"),
        ("\\}", "}"),
        ("\\,", " "),
        ("\\;", " "),
        ("\\!", ""),
        ("\\quad", "  "),
        ("\\qquad", "    "),
    ];

    for (from, to) in replacements {
        s = s.replace(from, to);
    }

    // 6. Translate subscript and superscript braces: _{...} -> _(...) and ^{...} -> ^(...)
    s = replace_sub_super_braces(&s);

    // 7. Translate multi-letter identifiers
    s = quote_multichar_identifiers(&s);

    s
}

fn extract_braced_arg(s: &str) -> Option<(String, usize)> {
    let start_trimmed = s.trim_start();
    let trim_offset = s.len() - start_trimmed.len();
    if !start_trimmed.starts_with('{') {
        return None;
    }
    let mut depth = 0;
    let mut start_idx = None;
    for (i, c) in start_trimmed.char_indices() {
        if c == '{' {
            if depth == 0 {
                start_idx = Some(i + 1);
            }
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0
                && let Some(start) = start_idx
            {
                let content = &start_trimmed[start..i];
                return Some((content.to_string(), trim_offset + i + 1));
            }
        }
    }
    None
}

fn extract_two_braced_args(s: &str) -> Option<(String, String, usize)> {
    let (arg1, end1) = extract_braced_arg(s)?;
    let (arg2, end2) = extract_braced_arg(&s[end1..])?;
    Some((arg1, arg2, end1 + end2))
}

fn replace_sub_super_braces(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if (c == '_' || c == '^')
            && chars
                .peek()
                .map(|&(_, next_c)| next_c == '{')
                .unwrap_or(false)
        {
            out.push(c);
            let rest = &s[i + 1..];
            if let Some((body, end_pos)) = extract_braced_arg(rest) {
                let converted_body = replace_sub_super_braces(&body);
                out.push('(');
                out.push_str(&converted_body);
                out.push(')');
                for _ in 0..end_pos {
                    chars.next();
                }
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn quote_multichar_identifiers(s: &str) -> String {
    const BUILTINS: &[&str] = &[
        "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh",
        "tanh", "log", "ln", "exp", "lim", "max", "min", "det", "dim", "gcd", "lcm", "deg", "arg",
        "ker", "hom", "mod", "inf", "sup", "root", "frac", "sqrt", "sum", "product", "integral",
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
        "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi",
        "omega", "Delta", "Gamma", "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi",
        "Omega", "times", "dot", "div", "plus", "minus", "approx", "equiv", "tilde", "in", "oo",
        "diff", "nabla", "forall", "exists", "union", "sect", "subset", "supset", "and", "or",
        "not",
    ];

    let mut out = String::new();
    let mut current_word = String::new();
    let mut in_quote = false;
    let flush_word = |out: &mut String, word: &mut String, quoted: bool| {
        if !word.is_empty() {
            if !quoted && word.len() > 1 && !BUILTINS.contains(&word.as_str()) {
                out.push('"');
                out.push_str(word);
                out.push('"');
            } else {
                out.push_str(word);
            }
            word.clear();
        }
    };

    for c in s.chars() {
        if c == '"' {
            flush_word(&mut out, &mut current_word, in_quote);
            in_quote = !in_quote;
            out.push(c);
        } else if in_quote {
            out.push(c);
        } else if c.is_ascii_alphabetic() {
            current_word.push(c);
        } else {
            flush_word(&mut out, &mut current_word, false);
            out.push(c);
        }
    }
    flush_word(&mut out, &mut current_word, in_quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_headings_and_styles() {
        let md =
            "# Title 1\n## Subtitle 2\n\nThis is **bold** and _italic_ text with `inline_code`.";
        let typst = translate_md_to_typst(md, &ConvertOptions::default());
        assert!(typst.contains("= Title 1"));
        assert!(typst.contains("== Subtitle 2"));
        assert!(typst.contains("*bold*"));
        assert!(typst.contains("_italic_"));
        assert!(typst.contains("`inline_code`"));
    }

    #[test]
    fn test_translate_table_and_math() {
        let md = "| A | B |\n|---|---|\n| 1 | $x^2 + y^2 = z^2$ |";
        let typst = translate_md_to_typst(md, &ConvertOptions::default());
        assert!(typst.contains("#table("));
        assert!(typst.contains("$x^2 + y^2 = z^2$"));
    }

    #[test]
    fn test_latex_to_typst_math_complex() {
        let latex = r"DCF = \sum_{t=1}^{n} \frac{FCF_t}{(1+WACC)^t} + \frac{FCF_n \times (1+g)}{(WACC - g) \times (1+WACC)^n}";
        let typst_math = latex_to_typst_math(latex);
        assert!(typst_math.contains("\"DCF\""));
        assert!(typst_math.contains("sum_(t=1)^(n)"));
        assert!(typst_math.contains("\"FCF\"_t"));
        assert!(typst_math.contains("\"WACC\""));
    }
}
