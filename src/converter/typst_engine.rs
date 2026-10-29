use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use typst::LibraryExt;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

use crate::error::MartiniError;

static SYSTEM_FONTS: OnceLock<(LazyHash<FontBook>, Vec<Font>)> = OnceLock::new();

fn get_system_fonts() -> &'static (LazyHash<FontBook>, Vec<Font>) {
    SYSTEM_FONTS.get_or_init(|| {
        let mut fonts = Vec::new();
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        for face in db.faces() {
            db.with_face_data(face.id, |data, index| {
                if let Some(font) = Font::new(Bytes::new(data.to_vec()), index) {
                    fonts.push(font);
                }
            });
        }
        let book = LazyHash::new(FontBook::from_fonts(&fonts));
        (book, fonts)
    })
}

/// An in-memory/filesystem hybrid World for compiling Typst documents.
pub struct MartiniTypstWorld {
    library: LazyHash<Library>,
    book: &'static LazyHash<FontBook>,
    fonts: &'static [Font],
    main_id: FileId,
    main_source: Source,
    base_dir: PathBuf,
    file_cache: Mutex<HashMap<FileId, Bytes>>,
}

impl MartiniTypstWorld {
    pub fn new(source_text: &str, base_dir: PathBuf) -> Result<Self, MartiniError> {
        let vpath = VirtualPath::new("main.typ")
            .map_err(|e| MartiniError::PdfRender(format!("Invalid virtual path: {:?}", e)))?;
        let rooted_path = RootedPath::new(VirtualRoot::Project, vpath);
        let main_id = FileId::new(rooted_path);
        let main_source = Source::new(main_id, source_text.to_string());

        let (book, fonts) = get_system_fonts();
        let library = LazyHash::new(Library::default());

        Ok(Self {
            library,
            book,
            fonts: fonts.as_slice(),
            main_id,
            main_source,
            base_dir,
            file_cache: Mutex::new(HashMap::new()),
        })
    }
}

impl World for MartiniTypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main_source.clone())
        } else {
            let path = self.resolve_file_id(id)?;
            let text = std::fs::read_to_string(&path).map_err(|e| FileError::from_io(e, &path))?;
            Ok(Source::new(id, text))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let mut cache = self.file_cache.lock().unwrap();
        if let Some(bytes) = cache.get(&id) {
            return Ok(bytes.clone());
        }

        let path = self.resolve_file_id(id)?;
        let data = std::fs::read(&path).map_err(|e| FileError::from_io(e, &path))?;
        let bytes = Bytes::new(data);
        cache.insert(id, bytes.clone());
        Ok(bytes)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

impl MartiniTypstWorld {
    fn resolve_file_id(&self, id: FileId) -> FileResult<PathBuf> {
        let vpath_str = id.vpath().get_without_slash();
        let target = self.base_dir.join(vpath_str);
        if target.exists() {
            Ok(target)
        } else {
            let from_root = Path::new("/").join(vpath_str);
            if from_root.exists() {
                Ok(from_root)
            } else {
                Err(FileError::NotFound(target))
            }
        }
    }
}

/// Compiles Typst source text into PDF bytes.
pub fn compile_typst_to_pdf(source: &str, base_dir: &Path) -> Result<Vec<u8>, MartiniError> {
    let world = MartiniTypstWorld::new(source, base_dir.to_path_buf())?;
    let warned_output = typst::compile(&world);

    let doc = warned_output.output.map_err(|errors| {
        let mut err_msg = String::new();
        for err in errors {
            let hints: Vec<String> = err.hints.iter().map(|h| h.v.to_string()).collect();
            if hints.is_empty() {
                err_msg.push_str(&format!("{}\n", err.message));
            } else {
                err_msg.push_str(&format!("{}: {}\n", err.message, hints.join(", ")));
            }
        }
        MartiniError::PdfRender(format!("Typst compilation error:\n{}", err_msg.trim()))
    })?;

    let pdf_bytes = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
        .map_err(|e| MartiniError::PdfRender(format!("Typst PDF export error: {:?}", e)))?;

    Ok(pdf_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_basic_typst() {
        let source = "= Hello World\nThis is a test document.";
        let base_dir = std::env::current_dir().unwrap();
        let pdf = compile_typst_to_pdf(source, &base_dir).unwrap();
        assert!(!pdf.is_empty());
        assert!(pdf.starts_with(b"%PDF"));
    }
}
