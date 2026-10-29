use crate::converter::pptx::extract::{RawPathItem, RawPathSegment, RawTextItem};
use crate::converter::pptx::ir::{RectEmu, Table, TableCell, TableRow, TextBox};
use crate::converter::pptx::paragraph::cluster_paragraphs;
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct LineSegment {
    path_idx: usize,
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
}

pub fn detect_tables(
    paths: &mut Vec<RawPathItem>,
    text_items: &mut Vec<RawTextItem>,
) -> Vec<Table> {
    let mut h_lines = Vec::new();
    let mut v_lines = Vec::new();

    // Tolerance in EMU (2 pt = 25,400 EMU)
    let tol = 25_400;

    for (idx, path) in paths.iter().enumerate() {
        // 1. Check if path is a thin rectangle
        if path.rect.h <= tol && path.rect.w >= tol * 2 {
            h_lines.push(LineSegment {
                path_idx: idx,
                x1: path.rect.x,
                y1: path.rect.y,
                x2: path.rect.x + path.rect.w,
                y2: path.rect.y,
            });
            continue;
        } else if path.rect.w <= tol && path.rect.h >= tol * 2 {
            v_lines.push(LineSegment {
                path_idx: idx,
                x1: path.rect.x,
                y1: path.rect.y,
                x2: path.rect.x,
                y2: path.rect.y + path.rect.h,
            });
            continue;
        }

        // 2. Check linear path segments
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

                    if (y1 - y2).abs() <= tol && (x1 - x2).abs() >= tol * 2 {
                        h_lines.push(LineSegment {
                            path_idx: idx,
                            x1: x1.min(x2),
                            y1: (y1 + y2) / 2,
                            x2: x1.max(x2),
                            y2: (y1 + y2) / 2,
                        });
                    } else if (x1 - x2).abs() <= tol && (y1 - y2).abs() >= tol * 2 {
                        v_lines.push(LineSegment {
                            path_idx: idx,
                            x1: (x1 + x2) / 2,
                            y1: y1.min(y2),
                            x2: (x1 + x2) / 2,
                            y2: y1.max(y2),
                        });
                    }
                    curr_x = *x;
                    curr_y = *y;
                }
                _ => {}
            }
        }
    }

    if h_lines.len() < 2 || v_lines.len() < 2 {
        return Vec::new();
    }

    // Cluster lines that overlap and form a grid
    // For simplicity and high precision, cluster horizontal lines by x-span overlap
    let mut consumed_path_indices = HashSet::new();
    let mut consumed_text_indices = HashSet::new();
    let mut detected_tables = Vec::new();

    // Group horizontal lines by similar x1 and x2
    h_lines.sort_by_key(|h| (h.x1, h.x2, h.y1));
    v_lines.sort_by_key(|v| (v.y1, v.y2, v.x1));

    // Find grid candidates
    // Look at bounding boxes spanned by horizontal lines
    let mut y_coords: Vec<i64> = h_lines.iter().map(|h| h.y1).collect();
    y_coords.sort();
    y_coords.dedup_by(|a, b| (*a - *b).abs() <= tol);

    let mut x_coords: Vec<i64> = v_lines.iter().map(|v| v.x1).collect();
    x_coords.sort();
    x_coords.dedup_by(|a, b| (*a - *b).abs() <= tol);

    if y_coords.len() >= 2 && x_coords.len() >= 2 {
        let x_min = *x_coords.first().unwrap();
        let x_max = *x_coords.last().unwrap();
        let y_min = *y_coords.first().unwrap();
        let y_max = *y_coords.last().unwrap();

        // Check that horizontal lines span roughly [x_min, x_max]
        let h_span_valid = h_lines
            .iter()
            .filter(|h| (h.x1 - x_min).abs() <= tol && (h.x2 - x_max).abs() <= tol)
            .count()
            >= 2;

        // Check that vertical lines span roughly [y_min, y_max]
        let v_span_valid = v_lines
            .iter()
            .filter(|v| (v.y1 - y_min).abs() <= tol && (v.y2 - y_max).abs() <= tol)
            .count()
            >= 2;

        if h_span_valid && v_span_valid {
            let col_widths: Vec<i64> = x_coords.windows(2).map(|w| w[1] - w[0]).collect();
            let mut rows = Vec::new();

            for r in 0..(y_coords.len() - 1) {
                let row_y1 = y_coords[r];
                let row_y2 = y_coords[r + 1];
                let height_emu = row_y2 - row_y1;
                let mut cells = Vec::new();

                for c in 0..(x_coords.len() - 1) {
                    let col_x1 = x_coords[c];
                    let col_x2 = x_coords[c + 1];

                    // Find text items belonging inside this cell
                    let mut cell_text_items = Vec::new();
                    for (t_idx, item) in text_items.iter().enumerate() {
                        let center_x = item.rect.x + item.rect.w / 2;
                        let center_y = item.rect.y + item.rect.h / 2;

                        if center_x >= col_x1 - tol
                            && center_x <= col_x2 + tol
                            && center_y >= row_y1 - tol
                            && center_y <= row_y2 + tol
                        {
                            cell_text_items.push(item.clone());
                            consumed_text_indices.insert(t_idx);
                        }
                    }

                    let cell_textbox = if cell_text_items.is_empty() {
                        TextBox {
                            rect: RectEmu::new(col_x1, row_y1, col_x2 - col_x1, row_y2 - row_y1),
                            paragraphs: Vec::new(),
                            rotation_degrees: 0.0,
                        }
                    } else {
                        let mut clustered = cluster_paragraphs(cell_text_items);
                        if clustered.is_empty() {
                            TextBox {
                                rect: RectEmu::new(
                                    col_x1,
                                    row_y1,
                                    col_x2 - col_x1,
                                    row_y2 - row_y1,
                                ),
                                paragraphs: Vec::new(),
                                rotation_degrees: 0.0,
                            }
                        } else {
                            clustered.remove(0)
                        }
                    };

                    cells.push(TableCell {
                        text: cell_textbox,
                        fill: None,
                        col_span: 1,
                        row_span: 1,
                    });
                }

                rows.push(TableRow { height_emu, cells });
            }

            // Mark paths as consumed
            for h in &h_lines {
                if (h.x1 - x_min).abs() <= tol && (h.x2 - x_max).abs() <= tol {
                    consumed_path_indices.insert(h.path_idx);
                }
            }
            for v in &v_lines {
                if (v.y1 - y_min).abs() <= tol && (v.y2 - y_max).abs() <= tol {
                    consumed_path_indices.insert(v.path_idx);
                }
            }

            detected_tables.push(Table {
                rect: RectEmu::new(x_min, y_min, x_max - x_min, y_max - y_min),
                col_widths,
                rows,
            });
        }
    }

    // Remove consumed paths
    let mut path_indices: Vec<usize> = consumed_path_indices.into_iter().collect();
    path_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in path_indices {
        if idx < paths.len() {
            paths.remove(idx);
        }
    }

    // Remove consumed text items
    let mut text_indices: Vec<usize> = consumed_text_indices.into_iter().collect();
    text_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in text_indices {
        if idx < text_items.len() {
            text_items.remove(idx);
        }
    }

    detected_tables
}
