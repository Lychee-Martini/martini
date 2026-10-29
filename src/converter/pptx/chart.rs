use crate::converter::pptx::extract::{RawPathItem, RawPathSegment, RawTextItem};
use crate::converter::pptx::ir::{Chart, ChartSeries, ChartType, ColorRgb, Element, RectEmu};
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct TickInfo {
    text_idx: usize,
    y: i64,
    val: f64,
}

#[derive(Debug, Clone)]
struct ColMark {
    path_idx: usize,
    rect: RectEmu,
}

pub fn create_chart_xlsx(
    categories: &[String],
    series: &[ChartSeries],
) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let worksheet = workbook.add_worksheet();

    // Row 0: Category header, then each Series name
    worksheet.write_string(0, 0, "")?;
    for (s_idx, s) in series.iter().enumerate() {
        worksheet.write_string(0, (s_idx + 1) as u16, &s.name)?;
    }

    // Rows 1..: Category name, then values for each Series
    for (cat_idx, cat) in categories.iter().enumerate() {
        let r = (cat_idx + 1) as u32;
        worksheet.write_string(r, 0, cat)?;
        for (s_idx, s) in series.iter().enumerate() {
            if let Some(val) = s.values.get(cat_idx) {
                worksheet.write_number(r, (s_idx + 1) as u16, *val)?;
            }
        }
    }

    workbook.save_to_buffer()
}

pub fn detect_charts(
    paths: &mut Vec<RawPathItem>,
    text_items: &mut Vec<RawTextItem>,
) -> Vec<Element> {
    let mut elements = Vec::new();

    // Tolerance in EMU
    let tol = 30_000;

    // 1. Find vertical line candidates (axes)
    let mut v_axes = Vec::new();
    for (idx, path) in paths.iter().enumerate() {
        if path.rect.w <= tol && path.rect.h >= 100_000 {
            v_axes.push((idx, path.rect));
        } else {
            // Check segments
            let mut curr_x = 0.0f32;
            let mut curr_y = 0.0f32;
            for seg in &path.segments {
                match seg {
                    RawPathSegment::MoveTo(x, y) => {
                        curr_x = *x;
                        curr_y = *y;
                    }
                    RawPathSegment::LineTo(x, y) => {
                        let x1 = (curr_x * 12700.0).round() as i64;
                        let y1 = (curr_y * 12700.0).round() as i64;
                        let x2 = (*x * 12700.0).round() as i64;
                        let y2 = (*y * 12700.0).round() as i64;
                        if (x1 - x2).abs() <= tol && (y1 - y2).abs() >= 100_000 {
                            v_axes.push((
                                idx,
                                RectEmu::new((x1 + x2) / 2, y1.min(y2), tol, (y1 - y2).abs()),
                            ));
                        }
                        curr_x = *x;
                        curr_y = *y;
                    }
                    _ => {}
                }
            }
        }
    }

    if v_axes.is_empty() {
        return elements;
    }

    let mut consumed_paths = HashSet::new();
    let mut consumed_texts = HashSet::new();

    for &(axis_idx, axis_rect) in &v_axes {
        if consumed_paths.contains(&axis_idx) {
            continue;
        }

        // Look for aligned numeric ticks near the left of the axis
        let mut ticks = Vec::new();
        for (t_idx, item) in text_items.iter().enumerate() {
            if consumed_texts.contains(&t_idx) {
                continue;
            }
            let text = item.text.trim().trim_end_matches('%');
            if let Ok(val) = text.parse::<f64>() {
                // Must be situated to the left of the axis (or touching it)
                if item.rect.x <= axis_rect.x + tol
                    && item.rect.x >= axis_rect.x - 150_000
                    && item.rect.y >= axis_rect.y - tol
                    && item.rect.y <= axis_rect.y + axis_rect.h + tol
                {
                    ticks.push(TickInfo {
                        text_idx: t_idx,
                        y: item.rect.y,
                        val,
                    });
                }
            }
        }

        if ticks.len() < 2 {
            continue;
        }

        // Sort ticks by y (top to bottom)
        ticks.sort_by_key(|t| t.y);

        let top_tick = ticks.first().unwrap();
        let btm_tick = ticks.last().unwrap();
        let dy = (btm_tick.y - top_tick.y) as f64;
        let dv = btm_tick.val - top_tick.val;

        if dy.abs() < 1000.0 {
            continue;
        }

        let val_per_emu = dv / dy; // typically negative since y increases downwards and val decreases

        // Mark ticks as consumed
        for t in &ticks {
            consumed_texts.insert(t.text_idx);
        }

        // Find baseline horizontal line
        let mut baseline_opt = None;
        for (p_idx, path) in paths.iter().enumerate() {
            if p_idx == axis_idx || consumed_paths.contains(&p_idx) {
                continue;
            }
            if path.rect.h <= tol && path.rect.w >= 100_000 {
                // Must start near axis_rect.x
                if (path.rect.x - axis_rect.x).abs() <= tol * 2
                    || (path.rect.x + path.rect.w - axis_rect.x).abs() <= tol * 2
                {
                    // y should be within axis y range
                    if path.rect.y >= axis_rect.y - tol
                        && path.rect.y <= axis_rect.y + axis_rect.h + tol
                    {
                        baseline_opt = Some((p_idx, path.rect));
                        break;
                    }
                }
            }
        }

        // Find column rectangles to the right of axis
        let mut cols = Vec::new();
        for (p_idx, path) in paths.iter().enumerate() {
            if p_idx == axis_idx || consumed_paths.contains(&p_idx) {
                continue;
            }
            if matches!(baseline_opt, Some((base_idx, _)) if base_idx == p_idx) {
                continue;
            }

            // Check if it's a solid rectangle
            let is_col_shape = path.rect.w >= 10_000
                && path.rect.w <= 200_000
                && path.rect.h >= 10_000
                && path.rect.x >= axis_rect.x - tol
                && path.rect.x <= axis_rect.x + 800_000;

            if is_col_shape {
                cols.push(ColMark {
                    path_idx: p_idx,
                    rect: path.rect,
                });
            }
        }

        if cols.is_empty() {
            continue;
        }

        // Sort columns from left to right
        cols.sort_by_key(|c| c.rect.x);

        // Find zero-value y position
        let zero_y = if let Some((_, base_rect)) = baseline_opt {
            base_rect.y
        } else {
            // Find tick with val == 0.0 or interpolate
            let zero_tick = ticks.iter().find(|t| t.val.abs() < 0.001);
            if let Some(zt) = zero_tick {
                zt.y
            } else {
                ((0.0 - top_tick.val) / val_per_emu + (top_tick.y as f64)).round() as i64
            }
        };

        let mut categories = Vec::new();
        let mut values = Vec::new();

        for (c_idx, col) in cols.iter().enumerate() {
            // Compute value by interpolating the column height / edge relative to zero_y
            // If col extends above zero_y: positive
            // If col extends below zero_y: negative
            let val = if col.rect.y < zero_y {
                // Column points upwards
                let height = (zero_y - col.rect.y) as f64;
                (height * (-val_per_emu)).round()
            } else {
                // Column points downwards (negative value)
                let height = ((col.rect.y + col.rect.h) - zero_y) as f64;
                (-(height * (-val_per_emu))).round()
            };

            values.push(val);

            // Find matching category text under/near the column
            let mut cat_name = None;
            for (t_idx, item) in text_items.iter().enumerate() {
                if consumed_texts.contains(&t_idx) {
                    continue;
                }
                // Check if text item starts within column horizontal span
                let x_aligned =
                    item.rect.x >= col.rect.x - tol && item.rect.x <= col.rect.x + col.rect.w + tol;
                if x_aligned {
                    // Check if below col or below baseline
                    if item.rect.y >= col.rect.y + col.rect.h - tol || item.rect.y >= zero_y - tol {
                        cat_name = Some(item.text.trim().to_string());
                        consumed_texts.insert(t_idx);
                        break;
                    }
                }
            }

            categories.push(cat_name.unwrap_or_else(|| format!("Category {}", c_idx + 1)));
            consumed_paths.insert(col.path_idx);
        }

        // Mark ticks as consumed
        for t in &ticks {
            consumed_texts.insert(t.text_idx);
        }
        consumed_paths.insert(axis_idx);
        if let Some((base_idx, _)) = baseline_opt {
            consumed_paths.insert(base_idx);
        }

        let series = vec![ChartSeries {
            name: "Series 1".to_string(),
            values,
            color: Some(ColorRgb::new(68, 114, 196, 255)),
        }];

        let xlsx_bytes = create_chart_xlsx(&categories, &series).unwrap_or_default();

        let chart_w = cols
            .last()
            .map(|c| c.rect.x + c.rect.w - axis_rect.x + 50_000)
            .unwrap_or(400_000);
        let chart_rect = RectEmu::new(axis_rect.x, axis_rect.y, chart_w, axis_rect.h + 50_000);

        elements.push(Element::Chart(Chart {
            rect: chart_rect,
            chart_type: ChartType::Column,
            title: None,
            categories,
            series,
            xlsx_bytes,
        }));
    }

    // Remove consumed paths
    let mut p_indices: Vec<usize> = consumed_paths.into_iter().collect();
    p_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in p_indices {
        if idx < paths.len() {
            paths.remove(idx);
        }
    }

    // Remove consumed texts
    let mut t_indices: Vec<usize> = consumed_texts.into_iter().collect();
    t_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in t_indices {
        if idx < text_items.len() {
            text_items.remove(idx);
        }
    }

    elements
}
