use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontPair {
    pub latin: String,
    pub east_asian: String,
}

static SYSTEM_FONTS: OnceLock<fontdb::Database> = OnceLock::new();

fn get_system_fontdb() -> &'static fontdb::Database {
    SYSTEM_FONTS.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

/// Normalizes PDF font name by removing 6-letter subset prefixes (e.g. `ABCDEF+Arial-Bold` -> `Arial-Bold`)
/// and trailing style suffixes.
pub fn clean_font_name(raw_name: &str) -> String {
    let name = if let Some(idx) = raw_name.find('+') {
        if idx == 6 && raw_name[..6].chars().all(|c| c.is_ascii_uppercase()) {
            &raw_name[7..]
        } else {
            raw_name
        }
    } else {
        raw_name
    };

    // Strip common style suffixes
    let name = name.split(',').next().unwrap_or(name);
    let base = if let Some(idx) = name.find('-') {
        &name[..idx]
    } else {
        name
    };
    base.trim().to_string()
}

pub fn is_font_installed(family: &str) -> bool {
    let db = get_system_fontdb();
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        ..Default::default()
    };
    db.query(&query).is_some()
}

pub fn is_standard_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "simsun"
            | "simhei"
            | "microsoft yahei"
            | "kaiti"
            | "fangsong"
            | "pingfang sc"
            | "songti sc"
            | "hiragino sans gb"
            | "noto sans cjk sc"
            | "noto serif cjk sc"
            | "arial"
            | "times new roman"
            | "helvetica"
            | "calibri"
            | "cambria"
            | "consolas"
            | "courier new"
            | "tahoma"
            | "verdana"
            | "georgia"
    )
}

pub fn resolve_font_names(pdf_font: &str, has_cjk: bool) -> FontPair {
    let clean = clean_font_name(pdf_font);

    // If installed locally or is a recognized standard font family, preserve it
    let is_valid_font = is_font_installed(&clean) || is_standard_font(&clean);

    let lower = clean.to_lowercase();
    let is_serif = lower.contains("serif")
        || lower.contains("times")
        || lower.contains("song")
        || lower.contains("mincho")
        || lower.contains("ming");
    let is_mono = lower.contains("mono")
        || lower.contains("courier")
        || lower.contains("consolas")
        || lower.contains("code");

    let default_latin_sans = if cfg!(target_os = "windows") {
        "Arial"
    } else if cfg!(target_os = "macos") {
        "Helvetica"
    } else {
        "Liberation Sans"
    };

    let default_latin_serif = if cfg!(target_os = "windows") {
        "Times New Roman"
    } else if cfg!(target_os = "macos") {
        "Times"
    } else {
        "Liberation Serif"
    };

    let default_ea_sans = if cfg!(target_os = "windows") {
        "Microsoft YaHei"
    } else if cfg!(target_os = "macos") {
        "PingFang SC"
    } else {
        "Noto Sans CJK SC"
    };

    let default_ea_serif = if cfg!(target_os = "windows") {
        "SimSun"
    } else if cfg!(target_os = "macos") {
        "Songti SC"
    } else {
        "Noto Serif CJK SC"
    };

    let latin = if is_valid_font && !has_cjk {
        clean.clone()
    } else if is_mono {
        "Consolas".to_string()
    } else if is_serif {
        default_latin_serif.to_string()
    } else {
        default_latin_sans.to_string()
    };

    let east_asian = if is_valid_font && has_cjk {
        clean
    } else if lower.contains("simsun") || lower.contains("song") || is_serif {
        default_ea_serif.to_string()
    } else {
        default_ea_sans.to_string()
    };

    FontPair { latin, east_asian }
}
