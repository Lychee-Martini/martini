use indicatif::ProgressBar;
use std::path::Path;
use crate::converter::batch::{ProgressTracker, TaskResult};

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_format_size() {
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(0), "0 B");
    }
}
