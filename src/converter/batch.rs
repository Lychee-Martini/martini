use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread;

use crate::converter::{EncodedFile, Format};
use crate::error::MartiniError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskResult {
    pub input_path: String,
    pub output_path: Option<String>,
    pub status: String, // "success", "skipped", "failed"
    pub original_size: u64,
    pub converted_size: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BatchConvertOptions {
    pub input_dir: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub from_filter: String,
    pub targets: Vec<Format>,
    pub quality: u8,
    pub lossless: bool,
    pub recursive: bool,
    pub overwrite: bool,
    pub delete_original: bool,
    pub workers: Option<usize>,
    pub pages: Option<String>,
    pub dpi: u16,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchResult {
    pub tasks: Vec<TaskResult>,
    pub deleted_count: usize,
    pub deletion_errors: Vec<(String, String)>,
}

pub trait ProgressTracker: Send + Sync {
    fn set_message(&self, msg: &str);
    fn inc(&self, delta: u64);
}

#[derive(Clone)]
struct SubTask {
    target_fmt: Format,
    out_path: PathBuf,
}

#[derive(Clone)]
struct FileTask {
    input_path: PathBuf,
    sub_tasks: Vec<SubTask>,
}

struct ReadPayload {
    file_task: FileTask,
    input_data: Result<Vec<u8>, std::io::Error>,
}

struct WritePayload {
    input_path: PathBuf,
    #[allow(dead_code)]
    target_fmt: Format,
    status: String,
    original_size: u64,
    converted_size: u64,
    error_message: Option<String>,
    files_to_write: Vec<EncodedFile>,
}

pub fn get_all_images(directory: &Path, recursive: bool, from_filter: &str) -> Vec<PathBuf> {
    let mut image_files = Vec::new();
    let extensions: HashSet<&str> = match from_filter.to_lowercase().as_str() {
        "png" => ["png"].iter().copied().collect(),
        "jpg" | "jpeg" => ["jpg", "jpeg"].iter().copied().collect(),
        "webp" => ["webp"].iter().copied().collect(),
        "avif" => ["avif"].iter().copied().collect(),
        "svg" => ["svg"].iter().copied().collect(),
        "pdf" => ["pdf"].iter().copied().collect(),
        _ => ["png", "jpg", "jpeg"].iter().copied().collect(), // default/auto
    };

    let mut dirs_to_visit = vec![directory.to_path_buf()];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.starts_with('.')
                    && name != "."
                {
                    continue;
                }

                if path.is_dir() {
                    if recursive {
                        dirs_to_visit.push(path);
                    }
                } else if path.is_file()
                    && let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && extensions.contains(ext.to_lowercase().as_str())
                {
                    image_files.push(path);
                }
            }
        }
    }

    image_files.sort();
    image_files
}

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

    let mut file_tasks = Vec::new();
    for file_path in &files {
        let mut sub_tasks = Vec::new();
        for target_fmt in &options.targets {
            let out_path = match &options.output_dir {
                Some(out_dir) => {
                    let relative = file_path.strip_prefix(&options.input_dir).map_err(|e| {
                        MartiniError::InvalidInputData {
                            reason: format!(
                                "File path {:?} does not start with input directory {:?}: {}",
                                file_path, options.input_dir, e
                            ),
                        }
                    })?;
                    out_dir
                        .join(relative)
                        .with_extension(target_fmt.to_string())
                }
                None => file_path.with_extension(target_fmt.to_string()),
            };
            sub_tasks.push(SubTask {
                target_fmt: *target_fmt,
                out_path,
            });
        }
        file_tasks.push(FileTask {
            input_path: file_path.clone(),
            sub_tasks,
        });
    }

    // Pipeline channels: bounded sync channels to limit memory usage
    let (read_tx, read_rx) = sync_channel::<ReadPayload>(16);
    let (write_tx, write_rx) = sync_channel::<WritePayload>(32);

    // 1. Spawn Reader Thread
    let file_tasks_clone = file_tasks.clone();
    let reader_handle = thread::spawn(move || {
        for task in file_tasks_clone {
            let input_data = std::fs::read(&task.input_path);
            if read_tx
                .send(ReadPayload {
                    file_task: task,
                    input_data,
                })
                .is_err()
            {
                break;
            }
        }
    });

    // 2. Set up CPU conversion pool / Rayon workers coordinator
    let pool = if let Some(w) = options.workers {
        rayon::ThreadPoolBuilder::new()
            .num_threads(w)
            .build()
            .ok()
            .map(Arc::new)
    } else {
        None
    };

    let tracker_arc = tracker;
    let coordinator_handle = {
        let write_tx = write_tx.clone();
        let tracker_arc = tracker_arc.clone();
        let options = options.clone();
        let pool_clone = pool.clone();
        thread::spawn(move || {
            while let Ok(payload) = read_rx.recv() {
                let write_tx = write_tx.clone();
                let tracker_arc = tracker_arc.clone();
                let options = options.clone();
                let pool_clone2 = pool_clone.clone();

                let run_task = move || {
                    process_file_task(payload, &options, tracker_arc, write_tx);
                };

                if let Some(ref p) = pool_clone2 {
                    p.spawn(run_task);
                } else {
                    rayon::spawn(run_task);
                }
            }
        })
    };

    // Drop our main thread's write_tx copy so it doesn't keep the channel open.
    drop(write_tx);

    // 3. Spawn Writer Thread
    let writer_handle = thread::spawn(move || {
        let mut tasks = Vec::new();
        while let Ok(payload) = write_rx.recv() {
            let mut converted_size = 0;
            let mut status = payload.status;
            let mut error_message = payload.error_message;

            if status == "success" {
                // Perform file writes
                let mut write_failed = false;
                for file in &payload.files_to_write {
                    if let Some(parent) = file.path.parent().filter(|p| !p.as_os_str().is_empty())
                        && let Err(e) = std::fs::create_dir_all(parent)
                    {
                        write_failed = true;
                        error_message = Some(format!("Failed to create parent directories: {}", e));
                        break;
                    }
                    if let Err(e) = std::fs::write(&file.path, &file.bytes) {
                        write_failed = true;
                        error_message = Some(format!("Failed to write output file: {}", e));
                        break;
                    }
                    converted_size += file.bytes.len() as u64;
                }

                if write_failed {
                    status = "failed".to_string();
                    converted_size = 0;
                }
            } else if status == "skipped" {
                converted_size = payload.converted_size;
            }

            let task_res = TaskResult {
                input_path: payload.input_path.to_string_lossy().to_string(),
                output_path: payload
                    .files_to_write
                    .first()
                    .map(|f| f.path.to_string_lossy().to_string()),
                status,
                original_size: payload.original_size,
                converted_size,
                error_message,
            };

            tasks.push(task_res);
        }
        tasks
    });

    // Wait for reader and coordinator threads to finish
    let _ = reader_handle.join();
    let _ = coordinator_handle.join();

    // Get final results from writer thread
    let tasks = writer_handle.join().unwrap_or_default();

    // Group successes and failures for safe deletion check
    let mut conversion_successes = HashMap::new();
    let mut conversion_failures = HashMap::new();
    for res in &tasks {
        if res.status == "success" || res.status == "skipped" {
            conversion_successes
                .entry(res.input_path.clone())
                .or_insert_with(HashSet::new)
                .insert(res.output_path.clone().unwrap_or_default());
        } else {
            conversion_failures
                .entry(res.input_path.clone())
                .or_insert_with(HashSet::new)
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

fn process_file_task(
    payload: ReadPayload,
    options: &BatchConvertOptions,
    tracker: Option<Arc<dyn ProgressTracker>>,
    write_tx: SyncSender<WritePayload>,
) {
    let file_task = payload.file_task;
    let input_path = file_task.input_path;

    if let Some(ref t) = tracker {
        let filename = input_path
            .file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_default();
        t.set_message(&format!("Converting {}", filename));
    }

    let input_bytes = match payload.input_data {
        Ok(bytes) => bytes,
        Err(e) => {
            for sub in file_task.sub_tasks {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size: 0,
                    converted_size: 0,
                    error_message: Some(format!("Failed to read source file: {}", e)),
                    files_to_write: Vec::new(),
                });
                if let Some(ref t) = tracker {
                    t.inc(1);
                }
            }
            return;
        }
    };

    let original_size = input_bytes.len() as u64;

    let file_ext = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let from_fmt = match Format::from_str(&file_ext) {
        Ok(fmt) => fmt,
        Err(_) => {
            for sub in file_task.sub_tasks {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: Some(format!("Unsupported source format: '{}'", file_ext)),
                    files_to_write: Vec::new(),
                });
                if let Some(ref t) = tracker {
                    t.inc(1);
                }
            }
            return;
        }
    };

    if from_fmt == Format::Pdf {
        use pdfium_auto::bind_pdfium_silent;
        use pdfium_render::prelude::PdfRenderConfig;

        for sub in file_task.sub_tasks {
            if sub.out_path.exists() && !options.overwrite {
                let converted_size = sub.out_path.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "skipped".to_string(),
                    original_size,
                    converted_size,
                    error_message: None,
                    files_to_write: Vec::new(),
                });
                if let Some(ref t) = tracker {
                    t.inc(1);
                }
                continue;
            }

            let pdfium = match crate::converter::pdf_conv::get_pdfium() {
                Ok(p) => p,
                Err(e) => {
                    let _ = write_tx.send(WritePayload {
                        input_path: input_path.clone(),
                        target_fmt: sub.target_fmt,
                        status: "failed".to_string(),
                        original_size,
                        converted_size: 0,
                        error_message: Some(format!("Failed to load PDFium: {:?}", e)),
                        files_to_write: Vec::new(),
                    });
                    if let Some(ref t) = tracker {
                        t.inc(1);
                    }
                    continue;
                }
            };

            let document = match pdfium.load_pdf_from_byte_slice(&input_bytes, None) {
                Ok(doc) => doc,
                Err(e) => {
                    let _ = write_tx.send(WritePayload {
                        input_path: input_path.clone(),
                        target_fmt: sub.target_fmt,
                        status: "failed".to_string(),
                        original_size,
                        converted_size: 0,
                        error_message: Some(format!("Failed to parse PDF: {:?}", e)),
                        files_to_write: Vec::new(),
                    });
                    if let Some(ref t) = tracker {
                        t.inc(1);
                    }
                    continue;
                }
            };

            let total_pages = document.pages().len() as u16;
            if total_pages == 0 {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: Some("PDF has no pages".to_string()),
                    files_to_write: Vec::new(),
                });
                if let Some(ref t) = tracker {
                    t.inc(1);
                }
                continue;
            }

            let pages_to_render = if let Some(ref range_str) = options.pages {
                match crate::converter::pdf_conv::parse_pages(range_str, total_pages) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = write_tx.send(WritePayload {
                            input_path: input_path.clone(),
                            target_fmt: sub.target_fmt,
                            status: "failed".to_string(),
                            original_size,
                            converted_size: 0,
                            error_message: Some(format!("Invalid page selection: {}", e)),
                            files_to_write: Vec::new(),
                        });
                        if let Some(ref t) = tracker {
                            t.inc(1);
                        }
                        continue;
                    }
                }
            } else {
                (0..total_pages).collect()
            };

            let encoder: Box<dyn crate::converter::image_conv::Encoder> = match sub.target_fmt {
                Format::Webp => Box::new(crate::converter::image_conv::WebpEncoder),
                Format::Avif => Box::new(crate::converter::image_conv::AvifEncoder),
                Format::Png => Box::new(crate::converter::image_conv::PngEncoder),
                Format::Jpg => Box::new(crate::converter::image_conv::JpegEncoder),
                _ => {
                    let _ = write_tx.send(WritePayload {
                        input_path: input_path.clone(),
                        target_fmt: sub.target_fmt,
                        status: "failed".to_string(),
                        original_size,
                        converted_size: 0,
                        error_message: Some(format!(
                            "Unsupported target format for PDF: '{}'",
                            sub.target_fmt
                        )),
                        files_to_write: Vec::new(),
                    });
                    if let Some(ref t) = tracker {
                        t.inc(1);
                    }
                    continue;
                }
            };

            let dpi = options.dpi.max(1);
            let render_config = PdfRenderConfig::new()
                .scale_page_by_factor(dpi as f32 / 72.0)
                .render_form_data(true)
                .render_annotations(true);

            let mut files_to_write = Vec::new();
            let mut failed_page = None;

            for &page_index in &pages_to_render {
                let page_num = page_index + 1;
                let page = match document.pages().get(page_index) {
                    Ok(p) => p,
                    Err(e) => {
                        failed_page = Some(format!("Failed to get page {}: {:?}", page_num, e));
                        break;
                    }
                };
                let img = match page.render_with_config(&render_config) {
                    Ok(i) => i,
                    Err(e) => {
                        failed_page = Some(format!("Failed to render page {}: {:?}", page_num, e));
                        break;
                    }
                };
                let dynamic_img = img.as_image();

                let stem = sub
                    .out_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                let ext = sub
                    .out_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let page_filename = format!("{}_page_{}.{}", stem, page_num, ext);
                let page_out_path = sub.out_path.with_file_name(page_filename);

                let sub_options = crate::converter::ConvertOptions {
                    input_path: input_path.clone(),
                    output_path: page_out_path.clone(),
                    package: false,
                    quality: options.quality,
                    lossless: options.lossless,
                    overwrite: options.overwrite,
                    pages: None,
                    dpi: options.dpi,
                };

                match encoder.encode(
                    Format::Pdf,
                    sub.target_fmt,
                    None,
                    Some(&dynamic_img),
                    &sub_options,
                ) {
                    Ok(mut encoded_files) => {
                        files_to_write.append(&mut encoded_files);
                    }
                    Err(e) => {
                        failed_page = Some(format!("Failed to encode page {}: {}", page_num, e));
                        break;
                    }
                }
            }

            if let Some(err) = failed_page {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: Some(err),
                    files_to_write: Vec::new(),
                });
            } else {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "success".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: None,
                    files_to_write,
                });
            }

            if let Some(ref t) = tracker {
                t.inc(1);
            }
        }
        return;
    }

    let decoded_result = if from_fmt == Format::Svg {
        let opt = resvg::usvg::Options::default();
        resvg::usvg::Tree::from_data(&input_bytes, &opt)
            .map(|tree| (Some(tree), None))
            .map_err(MartiniError::from)
    } else {
        image::load_from_memory(&input_bytes)
            .map(|img| (None, Some(img)))
            .map_err(MartiniError::from)
    };

    let (svg_tree, target_img) = match decoded_result {
        Ok(val) => val,
        Err(e) => {
            for sub in file_task.sub_tasks {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: Some(format!("Failed to decode image: {}", e)),
                    files_to_write: Vec::new(),
                });
                if let Some(ref t) = tracker {
                    t.inc(1);
                }
            }
            return;
        }
    };

    for sub in file_task.sub_tasks {
        if sub.out_path.exists() && !options.overwrite {
            let converted_size = sub.out_path.metadata().map(|m| m.len()).unwrap_or(0);
            let _ = write_tx.send(WritePayload {
                input_path: input_path.clone(),
                target_fmt: sub.target_fmt,
                status: "skipped".to_string(),
                original_size,
                converted_size,
                error_message: None,
                files_to_write: Vec::new(),
            });
            if let Some(ref t) = tracker {
                t.inc(1);
            }
            continue;
        }

        let mut raster_img = target_img.clone();
        if sub.target_fmt != Format::Favicon
            && let Some(ref tree) = svg_tree
        {
            let w = tree.size().width().round() as u32;
            let h = tree.size().height().round() as u32;
            match crate::converter::image_conv::render_svg_to_rgba(tree, w, h) {
                Ok(rgba) => {
                    if let Some(img_buffer) =
                        image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(w, h, rgba)
                    {
                        raster_img = Some(image::DynamicImage::ImageRgba8(img_buffer));
                    } else {
                        let _ = write_tx.send(WritePayload {
                            input_path: input_path.clone(),
                            target_fmt: sub.target_fmt,
                            status: "failed".to_string(),
                            original_size,
                            converted_size: 0,
                            error_message: Some(
                                "Failed to create ImageBuffer from SVG".to_string(),
                            ),
                            files_to_write: Vec::new(),
                        });
                        if let Some(ref t) = tracker {
                            t.inc(1);
                        }
                        continue;
                    }
                }
                Err(e) => {
                    let _ = write_tx.send(WritePayload {
                        input_path: input_path.clone(),
                        target_fmt: sub.target_fmt,
                        status: "failed".to_string(),
                        original_size,
                        converted_size: 0,
                        error_message: Some(e.to_string()),
                        files_to_write: Vec::new(),
                    });
                    if let Some(ref t) = tracker {
                        t.inc(1);
                    }
                    continue;
                }
            }
        }

        if sub.target_fmt != Format::Favicon
            && let Some(ref img) = raster_img
        {
            let color_type = img.color();
            let converted = match color_type {
                image::ColorType::La8
                | image::ColorType::La16
                | image::ColorType::Rgba8
                | image::ColorType::Rgba16 => image::DynamicImage::ImageRgba8(img.to_rgba8()),
                _ => image::DynamicImage::ImageRgb8(img.to_rgb8()),
            };
            raster_img = Some(converted);
        }

        let sub_options = crate::converter::ConvertOptions {
            input_path: input_path.clone(),
            output_path: sub.out_path.clone(),
            package: false,
            quality: options.quality,
            lossless: options.lossless,
            overwrite: options.overwrite,
            pages: None,
            dpi: options.dpi,
        };

        let encoder: Box<dyn crate::converter::image_conv::Encoder> = match sub.target_fmt {
            Format::Webp => Box::new(crate::converter::image_conv::WebpEncoder),
            Format::Avif => Box::new(crate::converter::image_conv::AvifEncoder),
            Format::Png => Box::new(crate::converter::image_conv::PngEncoder),
            Format::Jpg => Box::new(crate::converter::image_conv::JpegEncoder),
            Format::Favicon => Box::new(crate::converter::image_conv::FaviconEncoder),
            _ => {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: Some(format!("Unsupported target format: '{}'", sub.target_fmt)),
                    files_to_write: Vec::new(),
                });
                if let Some(ref t) = tracker {
                    t.inc(1);
                }
                continue;
            }
        };

        match encoder.encode(
            from_fmt,
            sub.target_fmt,
            svg_tree.as_ref(),
            raster_img.as_ref(),
            &sub_options,
        ) {
            Ok(files_to_write) => {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "success".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: None,
                    files_to_write,
                });
            }
            Err(e) => {
                let _ = write_tx.send(WritePayload {
                    input_path: input_path.clone(),
                    target_fmt: sub.target_fmt,
                    status: "failed".to_string(),
                    original_size,
                    converted_size: 0,
                    error_message: Some(e.to_string()),
                    files_to_write: Vec::new(),
                });
            }
        }
    }
}
