use crate::converter::{ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use docx_rs::*;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

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
        let markdown_str =
            std::str::from_utf8(input_data).map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Input is not valid UTF-8: {}", e),
            })?;

        let mut parser_options = Options::empty();
        parser_options.insert(Options::ENABLE_TABLES);
        parser_options.insert(Options::ENABLE_STRIKETHROUGH);
        parser_options.insert(Options::ENABLE_TASKLISTS);
        parser_options.insert(Options::ENABLE_MATH);

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
                            is_blockquote: !list_stack.is_empty()
                                && current_para.as_ref().map_or(false, |p| p.is_blockquote),
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
                            is_list_item: true,
                            list_indent: indent,
                        });
                    }
                    Tag::Table(_) => {
                        table_state = Some(TableState {
                            rows: Vec::new(),
                            current_row_cells: Vec::new(),
                        });
                    }
                    Tag::TableHead => {}
                    Tag::TableRow => {}
                    Tag::TableCell => {
                        current_para = Some(ParagraphState {
                            elements: Vec::new(),
                            heading_level: None,
                            is_blockquote: false,
                            is_list_item: false,
                            list_indent: 0,
                        });
                    }
                    Tag::Strong => style.bold = true,
                    Tag::Emphasis => style.italic = true,
                    Tag::Strikethrough => style.strikethrough = true,
                    Tag::Link { dest_url, .. } => style.link = Some(dest_url.to_string()),
                    Tag::Image {
                        dest_url, title, ..
                    } => {
                        let text = if title.is_empty() {
                            format!("[Image: {}]", dest_url)
                        } else {
                            format!("[Image: {} - {}]", title, dest_url)
                        };
                        if let Some(ref mut p) = current_para {
                            p.elements.push(ParaElement::Run(
                                Run::new().add_text(text).italic().color("888888"),
                            ));
                        }
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = true;
                        current_para = Some(ParagraphState {
                            elements: Vec::new(),
                            heading_level: None,
                            is_blockquote: false,
                            list_indent: 0,
                            is_list_item: false,
                        });
                    }
                    _ => {}
                },
                Event::End(tag_end) => match tag_end {
                    TagEnd::Paragraph | TagEnd::Heading(_) => {
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
                    TagEnd::BlockQuote(_) => {}
                    TagEnd::List(_) => {
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
                            let borders = TableBorders::new()
                                .set(
                                    TableBorder::new(TableBorderPosition::Top)
                                        .border_type(BorderType::Single)
                                        .size(4)
                                        .color("CCCCCC"),
                                )
                                .set(
                                    TableBorder::new(TableBorderPosition::Bottom)
                                        .border_type(BorderType::Single)
                                        .size(4)
                                        .color("CCCCCC"),
                                )
                                .set(
                                    TableBorder::new(TableBorderPosition::Left)
                                        .border_type(BorderType::Single)
                                        .size(4)
                                        .color("CCCCCC"),
                                )
                                .set(
                                    TableBorder::new(TableBorderPosition::Right)
                                        .border_type(BorderType::Single)
                                        .size(4)
                                        .color("CCCCCC"),
                                )
                                .set(
                                    TableBorder::new(TableBorderPosition::InsideH)
                                        .border_type(BorderType::Single)
                                        .size(4)
                                        .color("CCCCCC"),
                                )
                                .set(
                                    TableBorder::new(TableBorderPosition::InsideV)
                                        .border_type(BorderType::Single)
                                        .size(4)
                                        .color("CCCCCC"),
                                );
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
                        if style.bold {
                            run = run.bold();
                        }
                        if style.italic {
                            run = run.italic();
                        }
                        if style.strikethrough {
                            run = run.strike();
                        }
                        if style.is_code {
                            run = run.color("A71D5D");
                        }
                    }

                    if let Some(ref mut p) = current_para {
                        if let Some(ref url) = style.link {
                            let hl = Hyperlink::new(url, HyperlinkType::External)
                                .add_run(run.color("0000FF").underline("single"));
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
                            let hl = Hyperlink::new(url, HyperlinkType::External)
                                .add_run(run.underline("single"));
                            p.elements.push(ParaElement::Link(hl));
                        } else {
                            p.elements.push(ParaElement::Run(run));
                        }
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(ref mut p) = current_para {
                        p.elements.push(ParaElement::Run(
                            Run::new().add_break(BreakType::TextWrapping),
                        ));
                    }
                }
                Event::InlineMath(math) => {
                    let run = Run::new()
                        .add_text(math.to_string())
                        .italic()
                        .color("4A154B");
                    if let Some(ref mut p) = current_para {
                        p.elements.push(ParaElement::Run(run));
                    }
                }
                Event::DisplayMath(math) => {
                    let docx_p = Paragraph::new().align(AlignmentType::Center).add_run(
                        Run::new()
                            .add_text(math.to_string())
                            .italic()
                            .color("4A154B")
                            .size(24),
                    );
                    doc = doc.add_paragraph(docx_p);
                }
                _ => {}
            }
        }

        let output_file =
            std::fs::File::create(&options.output_path).map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to create output file: {}", e),
            })?;

        doc.build()
            .pack(output_file)
            .map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to pack DOCX: {}", e),
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
