use crate::converter::pptx::extract::RawTextItem;
use crate::converter::pptx::font::resolve_font_names;
use crate::converter::pptx::ir::{ColorRgb, Paragraph, RectEmu, TextAlign, TextBox, TextRun};

#[derive(Debug)]
struct TextCluster {
    rect: RectEmu,
    runs: Vec<TextRun>,
    rotation: f32,
    last_y: i64,
    last_h: i64,
    font_size_pt: f32,
    font_name: String,
}

pub fn cluster_paragraphs(text_items: Vec<RawTextItem>) -> Vec<TextBox> {
    let mut visible_items: Vec<RawTextItem> = text_items
        .into_iter()
        .filter(|t| !t.render_mode_invisible && !t.text.is_empty())
        .collect();

    if visible_items.is_empty() {
        return Vec::new();
    }

    // Sort top-to-bottom, then left-to-right
    visible_items.sort_by(|a, b| {
        if (a.rect.y - b.rect.y).abs() > 30_000 {
            a.rect.y.cmp(&b.rect.y)
        } else {
            a.rect.x.cmp(&b.rect.x)
        }
    });

    let mut clusters: Vec<TextCluster> = Vec::new();

    for item in visible_items {
        let has_cjk = item.text.chars().any(|c| {
            ('\u{4E00}'..='\u{9FFF}').contains(&c) || ('\u{3400}'..='\u{4DBF}').contains(&c)
        });
        let font_pair = resolve_font_names(&item.font_name, has_cjk);
        let run = TextRun {
            text: item.text.clone(),
            font_latin: font_pair.latin,
            font_ea: font_pair.east_asian,
            size_pt: item.font_size_pt,
            color: ColorRgb::new(
                item.color_rgba.0,
                item.color_rgba.1,
                item.color_rgba.2,
                item.color_rgba.3,
            ),
            bold: item.bold,
            italic: item.italic,
            underline: false,
        };

        // Try merging into an existing cluster
        let mut merged = false;
        for cluster in clusters.iter_mut().rev() {
            if (cluster.rotation - item.rotation_degrees).abs() > 1.0 {
                continue;
            }
            if (cluster.font_size_pt - item.font_size_pt).abs() > 3.0 {
                continue;
            }
            if cluster.font_name != item.font_name {
                continue;
            }

            let max_line_gap = (cluster.font_size_pt * 12700.0 * 2.5) as i64;
            let y_gap = item.rect.y - (cluster.last_y + cluster.last_h);

            // Same line or immediately following line within paragraph gap
            let same_line = (item.rect.y - cluster.last_y).abs() <= 35_000;
            let next_line = y_gap >= -10_000 && y_gap <= max_line_gap;

            if same_line || next_line {
                let left_diff = (item.rect.x - cluster.rect.x).abs();
                let horizontal_overlap = !(item.rect.x > cluster.rect.x + cluster.rect.w + 120_000
                    || item.rect.x + item.rect.w < cluster.rect.x - 120_000);

                if left_diff < 120_000 || horizontal_overlap || same_line {
                    cluster.rect = cluster.rect.union(&item.rect);
                    cluster.last_y = item.rect.y;
                    cluster.last_h = item.rect.h;
                    cluster.runs.push(run.clone());
                    merged = true;
                    break;
                }
            }
        }

        if !merged {
            clusters.push(TextCluster {
                rect: item.rect,
                runs: vec![run],
                rotation: item.rotation_degrees,
                last_y: item.rect.y,
                last_h: item.rect.h,
                font_size_pt: item.font_size_pt,
                font_name: item.font_name,
            });
        }
    }

    clusters
        .into_iter()
        .map(|c| TextBox {
            rect: c.rect,
            paragraphs: vec![Paragraph {
                runs: c.runs,
                line_spacing_pt: None,
                align: TextAlign::Left,
            }],
            rotation_degrees: c.rotation,
        })
        .collect()
}
