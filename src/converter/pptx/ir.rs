#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectEmu {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl RectEmu {
    pub const fn new(x: i64, y: i64, w: i64, h: i64) -> Self {
        Self { x, y, w, h }
    }

    pub fn union(&self, other: &Self) -> Self {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.w).max(other.x + other.w);
        let y2 = (self.y + self.h).max(other.y + other.h);
        Self {
            x: x1,
            y: y1,
            w: (x2 - x1).max(0),
            h: (y2 - y1).max(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgb {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn black() -> Self {
        Self::new(0, 0, 0, 255)
    }

    pub fn to_hex_rgb(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub font_latin: String,
    pub font_ea: String,
    pub size_pt: f32,
    pub color: ColorRgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub runs: Vec<TextRun>,
    pub line_spacing_pt: Option<f32>,
    pub align: TextAlign,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub rect: RectEmu,
    pub paragraphs: Vec<Paragraph>,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub rect: RectEmu,
    pub image_bytes: Vec<u8>,
    pub format: ImageFormat,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeGeom {
    Preset(String),
    CustomPath(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub rect: RectEmu,
    pub geom: ShapeGeom,
    pub fill: Option<ColorRgb>,
    pub stroke: Option<(ColorRgb, f32)>,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
    pub text: TextBox,
    pub fill: Option<ColorRgb>,
    pub col_span: usize,
    pub row_span: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub height_emu: i64,
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub rect: RectEmu,
    pub col_widths: Vec<i64>,
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    Column,
    Bar,
    Line,
    Pie,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
    pub color: Option<ColorRgb>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub rect: RectEmu,
    pub chart_type: ChartType,
    pub title: Option<String>,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
    pub xlsx_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub rect: RectEmu,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    TextBox(TextBox),
    Picture(Picture),
    Shape(Shape),
    Table(Table),
    Chart(Chart),
    Group(Group),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
    pub width_emu: i64,
    pub height_emu: i64,
    pub elements: Vec<Element>,
}

impl Slide {
    pub fn new(width_emu: i64, height_emu: i64) -> Self {
        Self {
            width_emu,
            height_emu,
            elements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideIR {
    pub width_emu: i64,
    pub height_emu: i64,
    pub slides: Vec<Slide>,
}

impl SlideIR {
    pub fn new(width_emu: i64, height_emu: i64) -> Self {
        Self {
            width_emu,
            height_emu,
            slides: Vec::new(),
        }
    }
}
