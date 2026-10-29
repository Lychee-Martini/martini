use crate::converter::pptx::ir::*;
use crate::error::MartiniError;
use std::io::{Cursor, Write};
use zip::write::{SimpleFileOptions, ZipWriter};

fn zip_err(e: zip::result::ZipError) -> MartiniError {
    MartiniError::Io(std::io::Error::other(e))
}

pub fn write_pptx(ir: &SlideIR) -> Result<Vec<u8>, MartiniError> {
    let mut buffer = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buffer));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 1. [Content_Types].xml
        zip.start_file("[Content_Types].xml", options)
            .map_err(zip_err)?;
        zip.write_all(generate_content_types(ir).as_bytes())
            .map_err(MartiniError::Io)?;

        // 2. _rels/.rels
        zip.start_file("_rels/.rels", options).map_err(zip_err)?;
        zip.write_all(generate_root_rels().as_bytes())
            .map_err(MartiniError::Io)?;

        // 3. ppt/presentation.xml
        zip.start_file("ppt/presentation.xml", options)
            .map_err(zip_err)?;
        zip.write_all(generate_presentation(ir).as_bytes())
            .map_err(MartiniError::Io)?;

        // 4. ppt/_rels/presentation.xml.rels
        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .map_err(zip_err)?;
        zip.write_all(generate_presentation_rels(ir.slides.len()).as_bytes())
            .map_err(MartiniError::Io)?;

        // 5. Static templates: theme1, slideMaster1, slideLayout1
        zip.start_file("ppt/theme/theme1.xml", options)
            .map_err(zip_err)?;
        zip.write_all(generate_theme().as_bytes())
            .map_err(MartiniError::Io)?;

        zip.start_file("ppt/slideMasters/slideMaster1.xml", options)
            .map_err(zip_err)?;
        zip.write_all(generate_slide_master().as_bytes())
            .map_err(MartiniError::Io)?;

        zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", options)
            .map_err(zip_err)?;
        zip.write_all(generate_slide_master_rels().as_bytes())
            .map_err(MartiniError::Io)?;

        zip.start_file("ppt/slideLayouts/slideLayout1.xml", options)
            .map_err(zip_err)?;
        zip.write_all(generate_slide_layout().as_bytes())
            .map_err(MartiniError::Io)?;

        zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", options)
            .map_err(zip_err)?;
        zip.write_all(generate_slide_layout_rels().as_bytes())
            .map_err(MartiniError::Io)?;

        // 6. Media deduplication / indexing
        let mut media_counter = 1;
        let mut chart_counter = 1;

        // 7. Slides
        for (slide_idx, slide) in ir.slides.iter().enumerate() {
            let slide_num = slide_idx + 1;
            let (slide_xml, slide_rels, media_files, chart_files) = render_slide(
                slide,
                ir.width_emu,
                ir.height_emu,
                &mut media_counter,
                &mut chart_counter,
            );

            // Write media
            for (media_name, media_bytes) in media_files {
                zip.start_file(format!("ppt/media/{}", media_name), options)
                    .map_err(zip_err)?;
                zip.write_all(&media_bytes).map_err(MartiniError::Io)?;
            }

            // Write charts & xlsx embeddings
            for (chart_xml_name, chart_xml, xlsx_name, xlsx_bytes) in chart_files {
                zip.start_file(format!("ppt/charts/{}", chart_xml_name), options)
                    .map_err(zip_err)?;
                zip.write_all(chart_xml.as_bytes())
                    .map_err(MartiniError::Io)?;

                // chart rels
                zip.start_file(format!("ppt/charts/_rels/{}.rels", chart_xml_name), options)
                    .map_err(zip_err)?;
                zip.write_all(generate_chart_rels(&xlsx_name).as_bytes())
                    .map_err(MartiniError::Io)?;

                // xlsx workbook
                zip.start_file(format!("ppt/embeddings/{}", xlsx_name), options)
                    .map_err(zip_err)?;
                zip.write_all(&xlsx_bytes).map_err(MartiniError::Io)?;
            }

            // Write slide XML
            zip.start_file(format!("ppt/slides/slide{}.xml", slide_num), options)
                .map_err(zip_err)?;
            zip.write_all(slide_xml.as_bytes())
                .map_err(MartiniError::Io)?;

            // Write slide rels
            zip.start_file(
                format!("ppt/slides/_rels/slide{}.xml.rels", slide_num),
                options,
            )
            .map_err(zip_err)?;
            zip.write_all(slide_rels.as_bytes())
                .map_err(MartiniError::Io)?;
        }

        zip.finish().map_err(zip_err)?;
    }
    Ok(buffer)
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn generate_content_types(ir: &SlideIR) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
"#,
    );

    for idx in 0..ir.slides.len() {
        xml.push_str(&format!(
            r#"  <Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
"#,
            idx + 1
        ));
    }

    let mut chart_idx = 1;
    for slide in &ir.slides {
        for el in &slide.elements {
            if matches!(el, Element::Chart(_)) {
                xml.push_str(&format!(
                    r#"  <Override PartName="/ppt/charts/chart{}.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>
"#,
                    chart_idx
                ));
                chart_idx += 1;
            }
        }
    }

    xml.push_str("</Types>");
    xml
}

fn generate_root_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#.to_string()
}

fn generate_presentation(ir: &SlideIR) -> String {
    let mut sld_id_lst = String::new();
    for idx in 0..ir.slides.len() {
        sld_id_lst.push_str(&format!(
            r#"    <p:sldId id="{}" r:id="rId{}"/>"#,
            256 + idx,
            idx + 2
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
{}
  </p:sldIdLst>
  <p:sldSz cx="{}" cy="{}" type="custom"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#,
        sld_id_lst, ir.width_emu, ir.height_emu
    )
}

fn generate_presentation_rels(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
"#,
    );

    for idx in 0..slide_count {
        xml.push_str(&format!(
            r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>
"#,
            idx + 2,
            idx + 1
        ));
    }

    xml.push_str(&format!(
        r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
</Relationships>"#,
        slide_count + 2
    ));

    xml
}

fn generate_theme() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5B9BD5"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont>
        <a:latin typeface="Arial"/>
        <a:ea typeface="Microsoft YaHei"/>
        <a:cs typeface=""/>
      </a:majorFont>
      <a:minorFont>
        <a:latin typeface="Arial"/>
        <a:ea typeface="Microsoft YaHei"/>
        <a:cs typeface=""/>
      </a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Office">
      <a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
      <a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
      <a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>
      <a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>"#.to_string()
}

fn generate_slide_master() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm>
      </p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="1" r:id="rId1"/>
  </p:sldLayoutIdLst>
</p:sldMaster>"#.to_string()
}

fn generate_slide_master_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#.to_string()
}

fn generate_slide_layout() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm>
      </p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sldLayout>"#.to_string()
}

fn generate_slide_layout_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#.to_string()
}

fn generate_chart_rels(xlsx_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/package" Target="../embeddings/{}"/>
</Relationships>"#,
        xlsx_name
    )
}

type SlideArtifacts = (
    String,
    String,
    Vec<(String, Vec<u8>)>,
    Vec<(String, String, String, Vec<u8>)>,
);

fn render_slide(
    slide: &Slide,
    presentation_width: i64,
    presentation_height: i64,
    media_counter: &mut usize,
    chart_counter: &mut usize,
) -> SlideArtifacts {
    // Review Focus 1: Uniform scaling & centering for non-standard slide sizes
    let scale = if slide.width_emu > 0 && slide.height_emu > 0 {
        let sx = presentation_width as f64 / slide.width_emu as f64;
        let sy = presentation_height as f64 / slide.height_emu as f64;
        sx.min(sy)
    } else {
        1.0
    };

    let offset_x = (presentation_width - (slide.width_emu as f64 * scale).round() as i64) / 2;
    let offset_y = (presentation_height - (slide.height_emu as f64 * scale).round() as i64) / 2;

    let transform_rect = |r: &RectEmu| -> RectEmu {
        RectEmu::new(
            offset_x + (r.x as f64 * scale).round() as i64,
            offset_y + (r.y as f64 * scale).round() as i64,
            (r.w as f64 * scale).round().max(1.0) as i64,
            (r.h as f64 * scale).round().max(1.0) as i64,
        )
    };

    let mut sp_tree = String::new();
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
"#,
    );

    let mut rel_id_counter = 2;
    let mut media_files = Vec::new();
    let mut chart_files = Vec::new();
    let mut shape_id = 2;

    for element in &slide.elements {
        match element {
            Element::TextBox(tb) => {
                let rect = transform_rect(&tb.rect);
                sp_tree.push_str(&render_textbox(tb, &rect, shape_id));
                shape_id += 1;
            }
            Element::Shape(sh) => {
                let rect = transform_rect(&sh.rect);
                sp_tree.push_str(&render_shape(sh, &rect, shape_id));
                shape_id += 1;
            }
            Element::Picture(pic) => {
                let rect = transform_rect(&pic.rect);
                let ext = match pic.format {
                    ImageFormat::Jpeg => "jpg",
                    ImageFormat::Png => "png",
                };
                let media_name = format!("image{}.{}", *media_counter, ext);
                *media_counter += 1;

                let r_id = format!("rId{}", rel_id_counter);
                rel_id_counter += 1;

                rels.push_str(&format!(
                    r#"  <Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{}"/>
"#,
                    r_id, media_name
                ));

                media_files.push((media_name, pic.image_bytes.clone()));
                sp_tree.push_str(&render_picture(pic, &rect, shape_id, &r_id));
                shape_id += 1;
            }
            Element::Table(tbl) => {
                let rect = transform_rect(&tbl.rect);
                sp_tree.push_str(&render_table(tbl, &rect, shape_id));
                shape_id += 1;
            }
            Element::Chart(ch) => {
                let rect = transform_rect(&ch.rect);
                let chart_xml_name = format!("chart{}.xml", *chart_counter);
                let xlsx_name = format!("Microsoft_Excel_Worksheet{}.xlsx", *chart_counter);
                *chart_counter += 1;

                let r_id = format!("rId{}", rel_id_counter);
                rel_id_counter += 1;

                rels.push_str(&format!(
                    r#"  <Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/{}"/>
"#,
                    r_id, chart_xml_name
                ));

                let chart_xml = render_chart_xml(ch);
                chart_files.push((chart_xml_name, chart_xml, xlsx_name, ch.xlsx_bytes.clone()));
                sp_tree.push_str(&render_chart_frame(ch, &rect, shape_id, &r_id));
                shape_id += 1;
            }
            Element::Group(grp) => {
                // Flatten group elements directly
                for sub in &grp.elements {
                    match sub {
                        Element::TextBox(tb) => {
                            let rect = transform_rect(&tb.rect);
                            sp_tree.push_str(&render_textbox(tb, &rect, shape_id));
                            shape_id += 1;
                        }
                        Element::Shape(sh) => {
                            let rect = transform_rect(&sh.rect);
                            sp_tree.push_str(&render_shape(sh, &rect, shape_id));
                            shape_id += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    rels.push_str("</Relationships>");

    let slide_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm>
      </p:grpSpPr>
{}
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#,
        sp_tree
    );

    (slide_xml, rels, media_files, chart_files)
}

fn render_textbox(tb: &TextBox, rect: &RectEmu, shape_id: usize) -> String {
    let mut paragraphs_xml = String::new();
    for p in &tb.paragraphs {
        let align_attr = match p.align {
            TextAlign::Left => "algn=\"l\"",
            TextAlign::Center => "algn=\"ctr\"",
            TextAlign::Right => "algn=\"r\"",
            TextAlign::Justify => "algn=\"just\"",
        };

        paragraphs_xml.push_str(&format!(r#"        <a:p><a:pPr {}/>"#, align_attr));
        for run in &p.runs {
            let bold_attr = if run.bold { " b=\"1\"" } else { "" };
            let italic_attr = if run.italic { " i=\"1\"" } else { "" };
            let underline_attr = if run.underline { " u=\"sng\"" } else { "" };
            let size_centipoints = (run.size_pt * 100.0).round() as i32;

            paragraphs_xml.push_str(&format!(
                r#"<a:r><a:rPr sz="{}"{}{}{}><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/><a:ea typeface="{}"/></a:rPr><a:t>{}</a:t></a:r>"#,
                size_centipoints,
                bold_attr,
                italic_attr,
                underline_attr,
                run.color.to_hex_rgb(),
                escape_xml(&run.font_latin),
                escape_xml(&run.font_ea),
                escape_xml(&run.text)
            ));
        }
        paragraphs_xml.push_str("</a:p>\n");
    }

    if paragraphs_xml.is_empty() {
        paragraphs_xml.push_str("        <a:p><a:endParaRPr/></a:p>\n");
    }

    let rot_attr = if tb.rotation_degrees.abs() > 0.01 {
        format!(
            " rot=\"{}\"",
            (tb.rotation_degrees * 60000.0).round() as i64
        )
    } else {
        String::new()
    };

    format!(
        r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="{}" name="TextBox {}"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm{}>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr wrap="square" rtlCol="0">
            <a:spAutoFit/>
          </a:bodyPr>
          <a:lstStyle/>
{}        </p:txBody>
      </p:sp>
"#,
        shape_id, shape_id, rot_attr, rect.x, rect.y, rect.w, rect.h, paragraphs_xml
    )
}

fn render_shape(sh: &Shape, rect: &RectEmu, shape_id: usize) -> String {
    let rot_attr = if sh.rotation_degrees.abs() > 0.01 {
        format!(
            " rot=\"{}\"",
            (sh.rotation_degrees * 60000.0).round() as i64
        )
    } else {
        String::new()
    };

    let fill_xml = match sh.fill {
        Some(c) => format!(
            r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
            c.to_hex_rgb()
        ),
        None => "<a:noFill/>".to_string(),
    };

    let stroke_xml = match sh.stroke {
        Some((c, w_pt)) => format!(
            r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
            (w_pt * 12700.0).round() as i64,
            c.to_hex_rgb()
        ),
        None => "<a:ln><a:noFill/></a:ln>".to_string(),
    };

    let geom_xml = match &sh.geom {
        ShapeGeom::Preset(name) => {
            format!(r#"<a:prstGeom prst="{}"><a:avLst/></a:prstGeom>"#, name)
        }
        ShapeGeom::CustomPath(path_data) => {
            // Review Focus 5: custom geometries with path commands
            format!(
                r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="{}" b="{}"/><a:pathLst><a:path w="{}" h="{}">{}</a:path></a:pathLst></a:custGeom>"#,
                rect.w, rect.h, rect.w, rect.h, path_data
            )
        }
    };

    format!(
        r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="{}" name="Shape {}"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm{}>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
          {}
          {}
          {}
        </p:spPr>
      </p:sp>
"#,
        shape_id,
        shape_id,
        rot_attr,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        geom_xml,
        fill_xml,
        stroke_xml
    )
}

fn render_picture(pic: &Picture, rect: &RectEmu, shape_id: usize, rel_id: &str) -> String {
    let rot_attr = if pic.rotation_degrees.abs() > 0.01 {
        format!(
            " rot=\"{}\"",
            (pic.rotation_degrees * 60000.0).round() as i64
        )
    } else {
        String::new()
    };

    format!(
        r#"      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="{}" name="Picture {}"/>
          <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="{}"/>
          <a:stretch><a:fillRect/></a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm{}>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
      </p:pic>
"#,
        shape_id, shape_id, rel_id, rot_attr, rect.x, rect.y, rect.w, rect.h
    )
}

fn render_table(tbl: &Table, rect: &RectEmu, shape_id: usize) -> String {
    let mut grid_xml = String::new();
    for w in &tbl.col_widths {
        grid_xml.push_str(&format!(r#"          <a:gridCol w="{}"/>"#, w));
    }

    let mut rows_xml = String::new();
    for row in &tbl.rows {
        rows_xml.push_str(&format!(
            r#"        <a:tr h="{}">"#,
            row.height_emu.max(100_000)
        ));
        for cell in &row.cells {
            let col_span_attr = if cell.col_span > 1 {
                format!(" gridSpan=\"{}\"", cell.col_span)
            } else {
                String::new()
            };
            let row_span_attr = if cell.row_span > 1 {
                format!(" rowSpan=\"{}\"", cell.row_span)
            } else {
                String::new()
            };

            let fill_xml = match cell.fill {
                Some(c) => format!(
                    r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
                    c.to_hex_rgb()
                ),
                None => "<a:noFill/>".to_string(),
            };

            let mut cell_paragraphs = String::new();
            for p in &cell.text.paragraphs {
                cell_paragraphs.push_str("              <a:p>");
                for run in &p.runs {
                    let bold = if run.bold { " b=\"1\"" } else { "" };
                    let sz = (run.size_pt * 100.0).round() as i32;
                    cell_paragraphs.push_str(&format!(
                        r#"<a:r><a:rPr sz="{}"{}><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/><a:ea typeface="{}"/></a:rPr><a:t>{}</a:t></a:r>"#,
                        sz,
                        bold,
                        run.color.to_hex_rgb(),
                        escape_xml(&run.font_latin),
                        escape_xml(&run.font_ea),
                        escape_xml(&run.text)
                    ));
                }
                cell_paragraphs.push_str("</a:p>");
            }
            if cell_paragraphs.is_empty() {
                cell_paragraphs.push_str("              <a:p><a:endParaRPr/></a:p>");
            }

            rows_xml.push_str(&format!(
                r#"          <a:tc{}{}>
            <a:txBody>
              <a:bodyPr/>
              <a:lstStyle/>
{}
            </a:txBody>
            <a:tcPr>
              {}
            </a:tcPr>
          </a:tc>"#,
                col_span_attr, row_span_attr, cell_paragraphs, fill_xml
            ));
        }
        rows_xml.push_str("        </a:tr>\n");
    }

    format!(
        r#"      <p:graphicFrame>
        <p:nvGraphicFramePr>
          <p:cNvPr id="{}" name="Table {}"/>
          <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr>
          <p:nvPr/>
        </p:nvGraphicFramePr>
        <p:xfrm>
          <a:off x="{}" y="{}"/>
          <a:ext cx="{}" cy="{}"/>
        </p:xfrm>
        <a:graphic>
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
            <a:tbl>
              <a:tblPr firstRow="1" bandRow="1"/>
              <a:tblGrid>
{}
              </a:tblGrid>
{}
            </a:tbl>
          </a:graphicData>
        </a:graphic>
      </p:graphicFrame>
"#,
        shape_id, shape_id, rect.x, rect.y, rect.w, rect.h, grid_xml, rows_xml
    )
}

fn render_chart_frame(_ch: &Chart, rect: &RectEmu, shape_id: usize, rel_id: &str) -> String {
    format!(
        r#"      <p:graphicFrame>
        <p:nvGraphicFramePr>
          <p:cNvPr id="{}" name="Chart {}"/>
          <p:cNvGraphicFramePr/>
          <p:nvPr/>
        </p:nvGraphicFramePr>
        <p:xfrm>
          <a:off x="{}" y="{}"/>
          <a:ext cx="{}" cy="{}"/>
        </p:xfrm>
        <a:graphic>
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">
            <c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
                     xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                     r:id="{}"/>
          </a:graphicData>
        </a:graphic>
      </p:graphicFrame>
"#,
        shape_id, shape_id, rect.x, rect.y, rect.w, rect.h, rel_id
    )
}

pub fn render_chart_xml(ch: &Chart) -> String {
    let title_xml = match &ch.title {
        Some(t) => format!(
            r#"<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr/><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:layout/></c:title>"#,
            escape_xml(t)
        ),
        None => "<c:autoTitleDeleted val=\"1\"/>".to_string(),
    };

    let mut series_xml = String::new();
    for (s_idx, s) in ch.series.iter().enumerate() {
        let mut cat_pts = String::new();
        for (c_idx, cat) in ch.categories.iter().enumerate() {
            cat_pts.push_str(&format!(
                r#"<c:pt idx="{}"><c:v>{}</c:v></c:pt>"#,
                c_idx,
                escape_xml(cat)
            ));
        }

        let mut val_pts = String::new();
        for (v_idx, val) in s.values.iter().enumerate() {
            val_pts.push_str(&format!(
                r#"<c:pt idx="{}"><c:v>{}</c:v></c:pt>"#,
                v_idx, val
            ));
        }

        let srgb = match s.color {
            Some(c) => c.to_hex_rgb(),
            None => match s_idx % 6 {
                0 => "4472C4".to_string(),
                1 => "ED7D31".to_string(),
                2 => "A5A5A5".to_string(),
                3 => "FFC000".to_string(),
                4 => "5B9BD5".to_string(),
                _ => "70AD47".to_string(),
            },
        };

        series_xml.push_str(&format!(
            r#"<c:ser>
  <c:idx val="{idx}"/>
  <c:order val="{idx}"/>
  <c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>{s_name}</c:v></c:pt></c:strCache></c:strRef></c:tx>
  <c:spPr><a:solidFill><a:srgbClr val="{srgb}"/></a:solidFill></c:spPr>
  <c:cat><c:strRef><c:strCache><c:ptCount val="{cat_count}"/>{cat_pts}</c:strCache></c:strRef></c:cat>
  <c:val><c:numRef><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{val_count}"/>{val_pts}</c:numCache></c:numRef></c:val>
</c:ser>"#,
            idx = s_idx,
            s_name = escape_xml(&s.name),
            srgb = srgb,
            cat_count = ch.categories.len(),
            cat_pts = cat_pts,
            val_count = s.values.len(),
            val_pts = val_pts
        ));
    }

    let chart_type_xml = match ch.chart_type {
        ChartType::Column => format!(
            r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>{}<c:axId val="12345"/><c:axId val="54321"/></c:barChart>"#,
            series_xml
        ),
        ChartType::Bar => format!(
            r#"<c:barChart><c:barDir val="bar"/><c:grouping val="clustered"/>{}<c:axId val="12345"/><c:axId val="54321"/></c:barChart>"#,
            series_xml
        ),
        ChartType::Line => format!(
            r#"<c:lineChart><c:grouping val="standard"/>{}<c:axId val="12345"/><c:axId val="54321"/></c:lineChart>"#,
            series_xml
        ),
        ChartType::Pie => format!(r#"<c:pieChart>{}</c:pieChart>"#, series_xml),
    };

    let axes_xml = if ch.chart_type != ChartType::Pie {
        r#"<c:catAx>
  <c:axId val="12345"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:crossAx val="54321"/><c:crosses val="autoZero"/>
</c:catAx>
<c:valAx>
  <c:axId val="54321"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:crossAx val="12345"/><c:crosses val="autoZero"/>
</c:valAx>"#
    } else {
        ""
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
              xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
              xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <c:chart>
    {}
    <c:plotArea>
      <c:layout/>
      {}
      {}
    </c:plotArea>
    <c:legend><c:legendPos val="r"/><c:layout/></c:legend>
    <c:plotVisOnly val="1"/>
  </c:chart>
  <c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData>
</c:chartSpace>"#,
        title_xml, chart_type_xml, axes_xml
    )
}
