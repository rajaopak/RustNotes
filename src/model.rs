use eframe::egui::{Color32, Pos2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageTemplate {
    Blank,
    Ruled,
    RuledNarrow,
    RuledWide,
    Grid,
    GridFine,
    Dotted,
    DottedFine,
    Cornell,
    Isometric,
}

impl PageTemplate {
    pub const ALL: [PageTemplate; 10] = [
        PageTemplate::Blank,
        PageTemplate::Ruled,
        PageTemplate::RuledNarrow,
        PageTemplate::RuledWide,
        PageTemplate::Grid,
        PageTemplate::GridFine,
        PageTemplate::Dotted,
        PageTemplate::DottedFine,
        PageTemplate::Cornell,
        PageTemplate::Isometric,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Blank => "Blank (Polos)",
            Self::Ruled => "Ruled Standard",
            Self::RuledNarrow => "Ruled Narrow",
            Self::RuledWide => "Ruled Wide",
            Self::Grid => "Grid Standard",
            Self::GridFine => "Grid Fine (5mm)",
            Self::Dotted => "Dotted Standard",
            Self::DottedFine => "Dotted Fine",
            Self::Cornell => "Cornell Notes",
            Self::Isometric => "Isometric 3D",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Blank => "Kertas polos bersih serbaguna",
            Self::Ruled => "Garis horizontal standar (30 pt)",
            Self::RuledNarrow => "Garis rapat untuk tulisan padat (20 pt)",
            Self::RuledWide => "Garis lebar untuk tulisan besar (40 pt)",
            Self::Grid => "Kotak-kotak simetris standar (30 pt)",
            Self::GridFine => "Kotak mikro presisi (15 pt / ~5 mm)",
            Self::Dotted => "Titik-titik simetris (30 pt)",
            Self::DottedFine => "Titik-titik halus (15 pt)",
            Self::Cornell => "Metode Cornell dengan kolom petunjuk & ringkasan",
            Self::Isometric => "Grid titik isometrik 60° untuk gambar teknik & 3D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    Triangle,
    RightTriangle,
    ScaleneTriangle {
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        u2: f32,
        v2: f32,
    },
    Parallelogram,
    Diamond,
    Curve,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn to_pos2(self) -> Pos2 {
        Pos2::new(self.x, self.y)
    }

    pub fn dist(self, other: Self) -> f32 {
        self.dist_sq(other).sqrt()
    }

    pub fn dist_sq(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalized(self) -> Option<Self> {
        let len = self.length();
        if len > 1e-6 {
            Some(Self {
                x: self.x / len,
                y: self.y / len,
            })
        } else {
            None
        }
    }

    pub fn rotate_around(self, origin: Self, angle_rad: f32) -> Self {
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();
        let dx = self.x - origin.x;
        let dy = self.y - origin.y;
        Self {
            x: origin.x + dx * cos_a - dy * sin_a,
            y: origin.y + dx * sin_a + dy * cos_a,
        }
    }
}

impl std::ops::Add for Point2D {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Point2D {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for Point2D {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl std::ops::Div<f32> for Point2D {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl From<Pos2> for Point2D {
    fn from(p: Pos2) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<Point2D> for Pos2 {
    fn from(p: Point2D) -> Self {
        Pos2::new(p.x, p.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VerticalAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeLabel {
    pub text: String,
    pub font_size: f32,
    pub color: ColorData,
    #[serde(default)]
    pub alignment: TextAlign,
    #[serde(default)]
    pub vertical_alignment: VerticalAlign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ConnectionSite {
    Top,
    Right,
    Bottom,
    Left,
    #[default]
    Auto,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConnectorEnd {
    Attached {
        shape_id: String,
        site: ConnectionSite,
    },
    Free {
        x: f32,
        y: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConnectorRoute {
    Straight,
    #[default]
    Orthogonal,
    Curved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarkerKind {
    #[default]
    None,
    Arrow,
}

fn default_arrow_marker() -> MarkerKind {
    MarkerKind::Arrow
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorStyle {
    pub stroke_color: ColorData,
    pub stroke_width: f32,
    #[serde(default)]
    pub start_marker: MarkerKind,
    #[serde(default = "default_arrow_marker")]
    pub end_marker: MarkerKind,
}

impl Default for ConnectorStyle {
    fn default() -> Self {
        Self {
            stroke_color: ColorData {
                r: 40,
                g: 45,
                b: 55,
                a: 255,
            },
            stroke_width: 2.5,
            start_marker: MarkerKind::None,
            end_marker: MarkerKind::Arrow,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

impl From<Pos2> for StrokePoint {
    fn from(p: Pos2) -> Self {
        Self {
            x: p.x,
            y: p.y,
            pressure: 1.0,
        }
    }
}

impl StrokePoint {
    pub fn to_pos2(&self) -> Pos2 {
        Pos2::new(self.x, self.y)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorData {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<Color32> for ColorData {
    fn from(c: Color32) -> Self {
        Self {
            r: c.r(),
            g: c.g(),
            b: c.b(),
            a: c.a(),
        }
    }
}

impl ColorData {
    pub fn to_color32(&self) -> Color32 {
        Color32::from_rgba_premultiplied(self.r, self.g, self.b, self.a)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StrokeStyle {
    #[default]
    Solid,
    Dashed,
    DashDot,
    Dotted,
}

impl StrokeStyle {
    pub const ALL: [StrokeStyle; 4] = [
        StrokeStyle::Solid,
        StrokeStyle::Dashed,
        StrokeStyle::DashDot,
        StrokeStyle::Dotted,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            StrokeStyle::Solid => "Solid",
            StrokeStyle::Dashed => "Dashed",
            StrokeStyle::DashDot => "Dash-Dot",
            StrokeStyle::Dotted => "Dotted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DrawObject {
    Stroke {
        id: String,
        points: Vec<StrokePoint>,
        color: ColorData,
        width: f32,
        is_highlighter: bool,
        #[serde(default)]
        style: StrokeStyle,
    },
    Shape {
        id: String,
        shape_type: ShapeKind,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ColorData,
        stroke_width: f32,
        fill: Option<ColorData>,
        #[serde(default)]
        rotation: f32,
        #[serde(default)]
        corner_radius: f32,
        #[serde(default)]
        label: Option<ShapeLabel>,
        #[serde(default)]
        locked: bool,
        #[serde(default)]
        curve_points: Vec<Point2D>,
    },
    Text {
        id: String,
        x: f32,
        y: f32,
        text: String,
        font_size: f32,
        color: ColorData,
        #[serde(default)]
        rotation: f32,
    },
    StickyNote {
        id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        text: String,
        color: ColorData,
    },
    Image {
        id: String,
        asset_id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        #[serde(default)]
        rotation: f32,
        #[serde(default = "default_true")]
        lock_aspect: bool,
    },
    Connector {
        id: String,
        start: ConnectorEnd,
        end: ConnectorEnd,
        #[serde(default)]
        route: ConnectorRoute,
        #[serde(default)]
        control_points: Vec<Point2D>,
        style: ConnectorStyle,
        #[serde(default)]
        locked: bool,
    },
}

fn default_true() -> bool {
    true
}

impl DrawObject {
    pub fn id(&self) -> &str {
        match self {
            Self::Stroke { id, .. } => id,
            Self::Shape { id, .. } => id,
            Self::Text { id, .. } => id,
            Self::StickyNote { id, .. } => id,
            Self::Image { id, .. } => id,
            Self::Connector { id, .. } => id,
        }
    }

    pub fn is_locked(&self) -> bool {
        match self {
            Self::Shape { locked, .. } | Self::Connector { locked, .. } => *locked,
            _ => false,
        }
    }

    pub fn set_locked(&mut self, val: bool) {
        match self {
            Self::Shape { locked, .. } | Self::Connector { locked, .. } => *locked = val,
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSizePreset {
    A4,
    Letter,
    A5,
    Legacy,
    Slide16x9,
    Standard4x3,
    Square,
}

impl PaperSizePreset {
    pub const ALL: [PaperSizePreset; 7] = [
        PaperSizePreset::A4,
        PaperSizePreset::Letter,
        PaperSizePreset::A5,
        PaperSizePreset::Slide16x9,
        PaperSizePreset::Standard4x3,
        PaperSizePreset::Square,
        PaperSizePreset::Legacy,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::A4 => "A4 (595 × 842)",
            Self::Letter => "US Letter (612 × 792)",
            Self::A5 => "A5 (420 × 595)",
            Self::Legacy => "Legacy (800 × 1100)",
            Self::Slide16x9 => "16:9 Slide (1280 × 720)",
            Self::Standard4x3 => "4:3 Standard (1024 × 768)",
            Self::Square => "Square (800 × 800)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::A4 => "A4",
            Self::Letter => "Letter",
            Self::A5 => "A5",
            Self::Legacy => "Legacy",
            Self::Slide16x9 => "16:9 Slide",
            Self::Standard4x3 => "4:3 Standard",
            Self::Square => "Square",
        }
    }

    pub fn dimensions(&self, orientation: PageOrientation) -> (f32, f32) {
        let (w, h) = match self {
            Self::A4 => (595.0, 842.0),
            Self::Letter => (612.0, 792.0),
            Self::A5 => (420.0, 595.0),
            Self::Legacy => (800.0, 1100.0),
            Self::Slide16x9 => (720.0, 1280.0),
            Self::Standard4x3 => (768.0, 1024.0),
            Self::Square => (800.0, 800.0),
        };
        match orientation {
            PageOrientation::Portrait => (w, h),
            PageOrientation::Landscape => (h, w),
        }
    }

    pub fn match_dimensions(w: f32, h: f32) -> Option<(Self, PageOrientation)> {
        for preset in Self::ALL {
            for orient in [PageOrientation::Portrait, PageOrientation::Landscape] {
                let (pw, ph) = preset.dimensions(orient);
                if (w - pw).abs() < 1.0 && (h - ph).abs() < 1.0 {
                    return Some((preset, orient));
                }
            }
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PageBackground {
    Template(PageTemplate),
    Pdf { asset_id: String, page_index: usize },
    Image { asset_id: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub width: f32,
    pub height: f32,
    pub template: PageTemplate,
    #[serde(default)]
    pub background: Option<PageBackground>,
    pub paper_color: ColorData,
    pub objects: Vec<DrawObject>,
    pub bookmarked: bool,
    /// Fase 8.6: Teks hasil ekstraksi OCR tulisan tangan untuk pengindeksan pencarian.
    #[serde(default)]
    pub ocr_text: String,
    /// Orientasi rotasi halaman dalam derajat (0, 90, 180, 270).
    #[serde(default)]
    pub rotation: i32,
}

impl Page {
    pub fn new(template: PageTemplate) -> Self {
        Self::with_size(template, 800.0, 1100.0)
    }

    pub fn with_size(template: PageTemplate, width: f32, height: f32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            width,
            height,
            template,
            background: None,
            paper_color: Color32::from_rgb(255, 254, 250).into(),
            objects: Vec::new(),
            bookmarked: false,
            ocr_text: String::new(),
            rotation: 0,
        }
    }

    pub fn effective_background(&self) -> PageBackground {
        self.background
            .clone()
            .unwrap_or(PageBackground::Template(self.template))
    }

    pub fn new_preset(
        template: PageTemplate,
        preset: PaperSizePreset,
        orientation: PageOrientation,
    ) -> Self {
        let (w, h) = preset.dimensions(orientation);
        Self::with_size(template, w, h)
    }

    /// Aturan integritas FASE G1.6:
    /// Memvalidasi seluruh `Connector.Attached` di halaman ini.
    /// Jika `shape_id` yang direferensikan tidak ditemukan di antara objek halaman ini,
    /// ubah ujung tersebut menjadi `ConnectorEnd::Free { x, y }`
    /// agar dokumen tidak crash dan hubungan orphan tertangani dengan aman.
    pub fn sanitize_connector_references(&mut self) -> usize {
        let mut fixed = 0;
        let valid_shape_ids: std::collections::HashSet<String> = self
            .objects
            .iter()
            .filter_map(|obj| {
                if let DrawObject::Shape { id, .. } = obj {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        for i in 0..self.objects.len() {
            if let DrawObject::Connector {
                ref start,
                ref end,
                ref control_points,
                ..
            } = self.objects[i]
            {
                let start_orphan = match start {
                    ConnectorEnd::Attached { shape_id, .. } => !valid_shape_ids.contains(shape_id),
                    _ => false,
                };
                let end_orphan = match end {
                    ConnectorEnd::Attached { shape_id, .. } => !valid_shape_ids.contains(shape_id),
                    _ => false,
                };

                if start_orphan || end_orphan {
                    let fallback_start = if let Some(first_cp) = control_points.first() {
                        (first_cp.x, first_cp.y)
                    } else if let ConnectorEnd::Free { x, y } = end {
                        (x - 80.0, *y)
                    } else {
                        (100.0, 100.0)
                    };

                    let fallback_end = if let Some(last_cp) = control_points.last() {
                        (last_cp.x, last_cp.y)
                    } else if let ConnectorEnd::Free { x, y } = start {
                        (x + 80.0, *y)
                    } else {
                        (fallback_start.0 + 80.0, fallback_start.1)
                    };

                    if let DrawObject::Connector {
                        ref mut start,
                        ref mut end,
                        ..
                    } = self.objects[i]
                    {
                        if start_orphan {
                            *start = ConnectorEnd::Free {
                                x: fallback_start.0,
                                y: fallback_start.1,
                            };
                            fixed += 1;
                        }
                        if end_orphan {
                            *end = ConnectorEnd::Free {
                                x: fallback_end.0,
                                y: fallback_end.1,
                            };
                            fixed += 1;
                        }
                    }
                }
            }
        }
        fixed
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutlineItem {
    pub id: String,
    pub title: String,
    pub page_index: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomTemplate {
    pub id: String,
    pub name: String,
    pub template: PageTemplate,
    pub background: Option<PageBackground>,
    pub paper_color: ColorData,
    pub width: f32,
    pub height: f32,
}

pub const CURRENT_NOTEBOOK_SCHEMA_VERSION: u32 = 2;

/// Nilai alpha standar (0-255) untuk goresan alat Highlighter di seluruh kanvas, sidebar, dan ekspor PNG
pub const HIGHLIGHTER_ALPHA: u8 = 95;

/// Nilai opacity desimal (0.0 - 1.0) untuk goresan alat Highlighter pada ekspor SVG
pub const HIGHLIGHTER_OPACITY_F32: f32 = HIGHLIGHTER_ALPHA as f32 / 255.0;

fn default_notebook_schema_version() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notebook {
    #[serde(default = "default_notebook_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub cover_color: ColorData,
    pub pages: Vec<Page>,
    pub folder_id: Option<String>,
    #[serde(default)]
    pub outline: Vec<OutlineItem>,
    #[serde(default)]
    pub trashed: bool,
    #[serde(default)]
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Notebook {
    pub fn new(title: &str, template: PageTemplate, cover_color: Color32) -> Self {
        Self::new_with_size(title, template, cover_color, 800.0, 1100.0)
    }

    pub fn new_with_size(
        title: &str,
        template: PageTemplate,
        cover_color: Color32,
        page_width: f32,
        page_height: f32,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            schema_version: CURRENT_NOTEBOOK_SCHEMA_VERSION,
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            cover_color: cover_color.into(),
            pages: vec![Page::with_size(template, page_width, page_height)],
            folder_id: None,
            outline: Vec::new(),
            trashed: false,
            favorite: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_with_preset(
        title: &str,
        template: PageTemplate,
        cover_color: Color32,
        preset: PaperSizePreset,
        orientation: PageOrientation,
    ) -> Self {
        let (w, h) = preset.dimensions(orientation);
        Self::new_with_size(title, template, cover_color, w, h)
    }

    pub fn duplicate_page(&mut self, page_idx: usize) -> Option<usize> {
        if page_idx >= self.pages.len() {
            return None;
        }
        let mut cloned = self.pages[page_idx].clone();
        cloned.id = Uuid::new_v4().to_string();
        let mut shape_id_map = std::collections::HashMap::new();
        for obj in &mut cloned.objects {
            match obj {
                DrawObject::Stroke { id, .. }
                | DrawObject::Text { id, .. }
                | DrawObject::StickyNote { id, .. }
                | DrawObject::Image { id, .. }
                | DrawObject::Connector { id, .. } => {
                    *id = Uuid::new_v4().to_string();
                }
                DrawObject::Shape { id, .. } => {
                    let old_id = id.clone();
                    let new_id = Uuid::new_v4().to_string();
                    *id = new_id.clone();
                    shape_id_map.insert(old_id, new_id);
                }
            }
        }
        // Remap attached shape IDs on connectors inside the duplicated page
        for obj in &mut cloned.objects {
            if let DrawObject::Connector { start, end, .. } = obj {
                if let ConnectorEnd::Attached { shape_id, .. } = start
                    && let Some(new_shape_id) = shape_id_map.get(shape_id)
                {
                    *shape_id = new_shape_id.clone();
                }
                if let ConnectorEnd::Attached { shape_id, .. } = end
                    && let Some(new_shape_id) = shape_id_map.get(shape_id)
                {
                    *shape_id = new_shape_id.clone();
                }
            }
        }
        let new_idx = page_idx + 1;
        self.pages.insert(new_idx, cloned);
        self.updated_at = chrono::Utc::now().timestamp_millis();
        Some(new_idx)
    }

    pub fn delete_page(&mut self, page_idx: usize) -> Result<usize, &'static str> {
        if self.pages.len() <= 1 {
            return Err("Cannot delete the only page in the notebook");
        }
        if page_idx >= self.pages.len() {
            return Err("Page index out of bounds");
        }
        self.pages.remove(page_idx);
        self.updated_at = chrono::Utc::now().timestamp_millis();
        let new_idx = page_idx.min(self.pages.len() - 1);
        Ok(new_idx)
    }

    pub fn insert_page(
        &mut self,
        insert_idx: usize,
        template: PageTemplate,
        width: f32,
        height: f32,
    ) -> usize {
        self.insert_page_with_color(
            insert_idx,
            template,
            width,
            height,
            Color32::from_rgb(255, 254, 250),
        )
    }

    pub fn insert_page_with_color(
        &mut self,
        insert_idx: usize,
        template: PageTemplate,
        width: f32,
        height: f32,
        paper_color: Color32,
    ) -> usize {
        let clamped_idx = insert_idx.min(self.pages.len());
        let mut new_page = Page::with_size(template, width, height);
        new_page.paper_color = paper_color.into();
        self.pages.insert(clamped_idx, new_page);
        self.updated_at = chrono::Utc::now().timestamp_millis();
        clamped_idx
    }

    pub fn move_page(&mut self, from_idx: usize, to_idx: usize) -> bool {
        if from_idx >= self.pages.len() || to_idx >= self.pages.len() || from_idx == to_idx {
            return false;
        }
        let page = self.pages.remove(from_idx);
        self.pages.insert(to_idx, page);
        self.updated_at = chrono::Utc::now().timestamp_millis();
        true
    }

    pub fn resize_page(&mut self, page_idx: usize, new_width: f32, new_height: f32) -> bool {
        if let Some(page) = self.pages.get_mut(page_idx) {
            page.width = new_width;
            page.height = new_height;
            self.updated_at = chrono::Utc::now().timestamp_millis();
            true
        } else {
            false
        }
    }

    pub fn rotate_page(&mut self, page_idx: usize, degrees: i32) -> bool {
        if let Some(page) = self.pages.get_mut(page_idx) {
            crate::geometry::rotate_page_content(page, degrees);
            self.updated_at = chrono::Utc::now().timestamp_millis();
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub color: ColorData,
    pub parent_id: Option<String>,
    pub created_at: i64,
}

#[allow(dead_code)]
impl Folder {
    pub fn new(name: &str, parent_id: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            color: Color32::from_rgb(136, 179, 213).into(),
            parent_id,
            created_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// Indeks relasi runtime (tidak diserialisasi):
/// Peta shape_id -> daftar connector_id yang menempel pada bentuk tersebut.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectorRelationIndex {
    pub shape_to_connectors: std::collections::HashMap<String, Vec<String>>,
}

impl ConnectorRelationIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rebuild_from_objects(&mut self, objects: &[DrawObject]) {
        self.shape_to_connectors.clear();
        for obj in objects {
            if let DrawObject::Connector { id, start, end, .. } = obj {
                if let ConnectorEnd::Attached { shape_id, .. } = start {
                    let list = self
                        .shape_to_connectors
                        .entry(shape_id.clone())
                        .or_default();
                    if !list.contains(id) {
                        list.push(id.clone());
                    }
                }
                if let ConnectorEnd::Attached { shape_id, .. } = end {
                    let list = self
                        .shape_to_connectors
                        .entry(shape_id.clone())
                        .or_default();
                    if !list.contains(id) {
                        list.push(id.clone());
                    }
                }
            }
        }
    }

    pub fn connectors_for_shape(&self, shape_id: &str) -> &[String] {
        self.shape_to_connectors
            .get(shape_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn remove_connector(&mut self, connector_id: &str) {
        for list in self.shape_to_connectors.values_mut() {
            list.retain(|id| id != connector_id);
        }
    }
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotebookMeta {
    pub id: String,
    pub title: String,
    pub cover_color: ColorData,
    pub page_count: usize,
    pub folder_id: Option<String>,
    pub trashed: bool,
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl NotebookMeta {
    pub fn from_notebook(nb: &Notebook) -> Self {
        Self {
            id: nb.id.clone(),
            title: nb.title.clone(),
            cover_color: nb.cover_color.clone(),
            page_count: nb.pages.len(),
            folder_id: nb.folder_id.clone(),
            trashed: nb.trashed,
            favorite: nb.favorite,
            created_at: nb.created_at,
            updated_at: nb.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryIndex {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub notebooks: Vec<NotebookMeta>,
    pub folders: Vec<Folder>,
}

impl Default for LibraryIndex {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            notebooks: Vec::new(),
            folders: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Library {
    #[serde(default = "default_schema_version", alias = "version")]
    pub schema_version: u32,
    pub notebooks: Vec<Notebook>,
    pub folders: Vec<Folder>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            notebooks: Vec::new(),
            folders: Vec::new(),
        }
    }
}

/// Fase 8.7: Pilihan Tema UI (Gelap / Terang / Ikuti Sistem OS Windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn next(self) -> Self {
        match self {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::System,
            ThemeMode::System => ThemeMode::Dark,
        }
    }
}

/// Fase 8.8: Pilihan Bahasa Antarmuka (Bahasa Indonesia & Bahasa Inggris).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppLanguage {
    #[default]
    Indonesian,
    English,
}

impl AppLanguage {
    pub fn toggle(self) -> Self {
        match self {
            AppLanguage::Indonesian => AppLanguage::English,
            AppLanguage::English => AppLanguage::Indonesian,
        }
    }

    pub fn tr<'a>(self, id_text: &'a str, en_text: &'a str) -> &'a str {
        match self {
            AppLanguage::Indonesian => id_text,
            AppLanguage::English => en_text,
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            AppLanguage::Indonesian => "🌐 ID",
            AppLanguage::English => "🌐 EN",
        }
    }
}

fn default_pen_widths() -> Vec<f32> {
    vec![1.5, 3.0, 5.0, 8.0]
}

fn default_color_swatches() -> Vec<ColorData> {
    vec![
        ColorData {
            r: 25,
            g: 27,
            b: 32,
            a: 255,
        }, // Black
        ColorData {
            r: 0,
            g: 114,
            b: 206,
            a: 255,
        }, // Goodnotes Blue
        ColorData {
            r: 226,
            g: 56,
            b: 56,
            a: 255,
        }, // Red
        ColorData {
            r: 130,
            g: 76,
            b: 163,
            a: 255,
        }, // Purple
        ColorData {
            r: 43,
            g: 149,
            b: 101,
            a: 255,
        }, // Emerald Green
        ColorData {
            r: 230,
            g: 182,
            b: 64,
            a: 255,
        }, // Amber
        ColorData {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }, // White
    ]
}

fn default_zoom_window_scale() -> f32 {
    2.5
}

fn default_palm_rejection() -> bool {
    true
}

fn default_pressure_min() -> f32 {
    0.1
}

fn default_pressure_multiplier() -> f32 {
    1.0
}

/// Fase 8.3, 8.4, 8.5, 8.7, 8.8: Pengaturan pengguna yang disimpan di `%APPDATA%\RustNotes\settings.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default = "default_pen_widths")]
    pub pen_widths: Vec<f32>,
    #[serde(default = "default_color_swatches")]
    pub color_swatches: Vec<ColorData>,
    #[serde(default = "default_zoom_window_scale")]
    pub zoom_window_scale: f32,
    #[serde(default = "default_palm_rejection")]
    pub palm_rejection: bool,
    #[serde(default)]
    pub language: AppLanguage,
    /// Fase G4.4: Opsi otomatis mengonversi bentuk saat pena ditahan (Hold-to-Snap).
    #[serde(default = "default_true")]
    pub auto_shape_enabled: bool,
    /// Pengaturan pintasan keyboard yang dapat disesuaikan pengguna.
    #[serde(default)]
    pub shortcuts: ShortcutSettings,
    /// Pengaturan sensitivitas tekanan stylus (Stage 1: Inking & Stroke System)
    #[serde(default = "default_true")]
    pub pressure_sensitivity_enabled: bool,
    #[serde(default = "default_pressure_min")]
    pub pressure_min: f32,
    #[serde(default = "default_pressure_multiplier")]
    pub pressure_multiplier: f32,
    /// Pengaturan transparansi highlighter (0.15 - 0.70, default HIGHLIGHTER_OPACITY_F32)
    #[serde(default = "default_highlighter_opacity")]
    pub highlighter_opacity: f32,
    #[serde(default = "default_false")]
    pub highlighter_pressure_enabled: bool,
}

fn default_false() -> bool {
    false
}

fn default_highlighter_opacity() -> f32 {
    HIGHLIGHTER_OPACITY_F32
}

impl AppSettings {
    pub fn effective_pressure(&self, raw_pressure: f32) -> f32 {
        if self.pressure_sensitivity_enabled {
            let mult = self.pressure_multiplier.clamp(0.1, 5.0);
            let min_p = self.pressure_min.clamp(0.01, 0.9);
            (raw_pressure * mult).clamp(min_p, 1.0)
        } else {
            1.0
        }
    }
}

/// Aksi-aksi yang dapat dikonfigurasi pintasan keyboard-nya.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShortcutAction {
    ToolPen,
    ToolHighlighter,
    ToolEraser,
    ToolLasso,
    ToolText,
    ToolShape,
    ToolConnector,
    ToolStickyNote,
    Undo,
    Redo,
    Save,
    SelectAll,
    Copy,
    Cut,
    Paste,
    Duplicate,
    Delete,
    Search,
    ResetZoom,
    ToggleSidebar,
    ToggleZoomWindow,
    ToggleLock,
    ToggleGrid,
}

impl ShortcutAction {
    pub const ALL: [Self; 23] = [
        Self::ToolPen,
        Self::ToolHighlighter,
        Self::ToolEraser,
        Self::ToolLasso,
        Self::ToolText,
        Self::ToolShape,
        Self::ToolConnector,
        Self::ToolStickyNote,
        Self::Undo,
        Self::Redo,
        Self::Save,
        Self::SelectAll,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Duplicate,
        Self::Delete,
        Self::Search,
        Self::ResetZoom,
        Self::ToggleSidebar,
        Self::ToggleZoomWindow,
        Self::ToggleLock,
        Self::ToggleGrid,
    ];

    pub fn label(self, lang: AppLanguage) -> &'static str {
        match (self, lang) {
            (Self::ToolPen, AppLanguage::Indonesian) => "Alat Pena (Pen)",
            (Self::ToolPen, AppLanguage::English) => "Pen Tool",
            (Self::ToolHighlighter, AppLanguage::Indonesian) => "Alat Penstabilo (Highlighter)",
            (Self::ToolHighlighter, AppLanguage::English) => "Highlighter Tool",
            (Self::ToolEraser, AppLanguage::Indonesian) => "Alat Penghapus (Eraser)",
            (Self::ToolEraser, AppLanguage::English) => "Eraser Tool",
            (Self::ToolLasso, AppLanguage::Indonesian) => "Alat Lasso / Seleksi",
            (Self::ToolLasso, AppLanguage::English) => "Lasso Selection Tool",
            (Self::ToolText, AppLanguage::Indonesian) => "Alat Teks",
            (Self::ToolText, AppLanguage::English) => "Text Tool",
            (Self::ToolShape, AppLanguage::Indonesian) => "Alat Bentuk (Shape)",
            (Self::ToolShape, AppLanguage::English) => "Shape Tool",
            (Self::ToolConnector, AppLanguage::Indonesian) => "Alat Konektor Diagram",
            (Self::ToolConnector, AppLanguage::English) => "Diagram Connector Tool",
            (Self::ToolStickyNote, AppLanguage::Indonesian) => "Catatan Tempel (Sticky Note)",
            (Self::ToolStickyNote, AppLanguage::English) => "Sticky Note Tool",
            (Self::Undo, AppLanguage::Indonesian) => "Batalkan (Undo)",
            (Self::Undo, AppLanguage::English) => "Undo",
            (Self::Redo, AppLanguage::Indonesian) => "Ulangi (Redo)",
            (Self::Redo, AppLanguage::English) => "Redo",
            (Self::Save, AppLanguage::Indonesian) => "Simpan Dokumen Segera",
            (Self::Save, AppLanguage::English) => "Save Document",
            (Self::SelectAll, AppLanguage::Indonesian) => "Pilih Semua Objek",
            (Self::SelectAll, AppLanguage::English) => "Select All Objects",
            (Self::Copy, AppLanguage::Indonesian) => "Salin Objek (Copy)",
            (Self::Copy, AppLanguage::English) => "Copy Objects",
            (Self::Cut, AppLanguage::Indonesian) => "Potong Objek (Cut)",
            (Self::Cut, AppLanguage::English) => "Cut Objects",
            (Self::Paste, AppLanguage::Indonesian) => "Tempel Objek (Paste)",
            (Self::Paste, AppLanguage::English) => "Paste Objects",
            (Self::Duplicate, AppLanguage::Indonesian) => "Gandakan Objek (Duplicate)",
            (Self::Duplicate, AppLanguage::English) => "Duplicate Objects",
            (Self::Delete, AppLanguage::Indonesian) => "Hapus Objek Terpilih",
            (Self::Delete, AppLanguage::English) => "Delete Selected Objects",
            (Self::Search, AppLanguage::Indonesian) => "Cari di Dokumen (Find)",
            (Self::Search, AppLanguage::English) => "Search In Document",
            (Self::ResetZoom, AppLanguage::Indonesian) => "Reset Zoom ke 100%",
            (Self::ResetZoom, AppLanguage::English) => "Reset Zoom to 100%",
            (Self::ToggleSidebar, AppLanguage::Indonesian) => "Buka/Tutup Panel Halaman",
            (Self::ToggleSidebar, AppLanguage::English) => "Toggle Page Sidebar",
            (Self::ToggleZoomWindow, AppLanguage::Indonesian) => "Buka/Tutup Zoom Window",
            (Self::ToggleZoomWindow, AppLanguage::English) => "Toggle Zoom Window",
            (Self::ToggleLock, AppLanguage::Indonesian) => "Kunci/Buka Kunci Objek Terpilih",
            (Self::ToggleLock, AppLanguage::English) => "Lock/Unlock Selected Objects",
            (Self::ToggleGrid, AppLanguage::Indonesian) => "Nyalakan/Matikan Grid Snapping",
            (Self::ToggleGrid, AppLanguage::English) => "Toggle Grid Snapping",
        }
    }

    pub fn default_binding(self) -> KeyBinding {
        match self {
            Self::ToolPen => KeyBinding::plain("P"),
            Self::ToolHighlighter => KeyBinding::plain("H"),
            Self::ToolEraser => KeyBinding::plain("E"),
            Self::ToolLasso => KeyBinding::plain("L"),
            Self::ToolText => KeyBinding::plain("T"),
            Self::ToolShape => KeyBinding::plain("S"),
            Self::ToolConnector => KeyBinding::plain("C"),
            Self::ToolStickyNote => KeyBinding::plain("N"),
            Self::Undo => KeyBinding::ctrl("Z"),
            Self::Redo => KeyBinding::ctrl("Y"),
            Self::Save => KeyBinding::ctrl("S"),
            Self::SelectAll => KeyBinding::ctrl("A"),
            Self::Copy => KeyBinding::ctrl("C"),
            Self::Cut => KeyBinding::ctrl("X"),
            Self::Paste => KeyBinding::ctrl("V"),
            Self::Duplicate => KeyBinding::ctrl("D"),
            Self::Delete => KeyBinding::plain("Delete"),
            Self::Search => KeyBinding::ctrl("F"),
            Self::ResetZoom => KeyBinding::ctrl("0"),
            Self::ToggleSidebar => KeyBinding::plain("F6"),
            Self::ToggleZoomWindow => KeyBinding::alt("Z"),
            Self::ToggleLock => KeyBinding::ctrl("L"),
            Self::ToggleGrid => KeyBinding::alt("G"),
        }
    }
}

/// Representasi kombinasi tombol pintasan yang dapat dikonfigurasi pengguna.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
}

impl KeyBinding {
    pub fn new(key: impl Into<String>, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self {
            key: key.into(),
            ctrl,
            alt,
            shift,
        }
    }

    pub fn plain(key: impl Into<String>) -> Self {
        Self::new(key, false, false, false)
    }

    pub fn ctrl(key: impl Into<String>) -> Self {
        Self::new(key, true, false, false)
    }

    pub fn alt(key: impl Into<String>) -> Self {
        Self::new(key, false, true, false)
    }

    pub fn display_string(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(&self.key);
        parts.join(" + ")
    }
}

/// Kumpulan pemetaan aksi ke pintasan tombol keyboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutSettings {
    #[serde(default = "default_custom_bindings")]
    pub bindings: std::collections::HashMap<ShortcutAction, KeyBinding>,
}

pub fn default_custom_bindings() -> std::collections::HashMap<ShortcutAction, KeyBinding> {
    let mut map = std::collections::HashMap::new();
    for action in ShortcutAction::ALL {
        map.insert(action, action.default_binding());
    }
    map
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            bindings: default_custom_bindings(),
        }
    }
}

impl ShortcutSettings {
    pub fn get(&self, action: ShortcutAction) -> KeyBinding {
        self.bindings
            .get(&action)
            .cloned()
            .unwrap_or_else(|| action.default_binding())
    }

    pub fn set(&mut self, action: ShortcutAction, binding: KeyBinding) {
        self.bindings.insert(action, binding);
    }

    pub fn reset_all(&mut self) {
        self.bindings = default_custom_bindings();
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            pen_widths: default_pen_widths(),
            color_swatches: default_color_swatches(),
            zoom_window_scale: default_zoom_window_scale(),
            palm_rejection: default_palm_rejection(),
            language: AppLanguage::Indonesian,
            auto_shape_enabled: true,
            shortcuts: ShortcutSettings::default(),
            pressure_sensitivity_enabled: true,
            pressure_min: 0.1,
            pressure_multiplier: 1.0,
            highlighter_opacity: HIGHLIGHTER_OPACITY_F32,
            highlighter_pressure_enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_object_stroke_roundtrip() {
        let obj = DrawObject::Stroke {
            id: "stroke-123".to_string(),
            points: vec![
                StrokePoint {
                    x: 10.0,
                    y: 20.0,
                    pressure: 0.8,
                },
                StrokePoint {
                    x: 30.0,
                    y: 40.0,
                    pressure: 0.9,
                },
            ],
            color: ColorData {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            width: 3.5,
            is_highlighter: false,
            style: StrokeStyle::Dashed,
        };

        let json = serde_json::to_string(&obj).expect("serialize");
        let decoded: DrawObject = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(obj, decoded);

        // Backward compatibility: verify legacy JSON without `style` defaults to Solid
        let legacy_json = r#"{
            "type": "Stroke",
            "id": "legacy-stroke",
            "points": [{"x": 0.0, "y": 0.0, "pressure": 1.0}],
            "color": {"r": 0, "g": 0, "b": 0, "a": 255},
            "width": 2.0,
            "is_highlighter": false
        }"#;
        let legacy_decoded: DrawObject =
            serde_json::from_str(legacy_json).expect("deserialize legacy stroke");
        if let DrawObject::Stroke { style, points, .. } = legacy_decoded {
            assert_eq!(style, StrokeStyle::Solid);
            assert_eq!(points.len(), 1);
        } else {
            panic!("Expected Stroke object");
        }
    }

    #[test]
    fn test_app_settings_pressure_calculation() {
        let mut settings = AppSettings::default();
        assert!(settings.pressure_sensitivity_enabled);
        assert_eq!(settings.effective_pressure(0.5), 0.5);
        assert_eq!(settings.effective_pressure(0.02), 0.1); // clamped to min 0.1
        assert_eq!(settings.effective_pressure(1.0), 1.0);

        settings.pressure_multiplier = 1.5;
        assert_eq!(settings.effective_pressure(0.4), 0.6);

        settings.pressure_sensitivity_enabled = false;
        assert_eq!(settings.effective_pressure(0.2), 1.0); // when disabled, always 1.0
    }

    #[test]
    fn test_draw_object_shape_roundtrip() {
        let obj = DrawObject::Shape {
            id: "shape-123".to_string(),
            shape_type: ShapeKind::Ellipse,
            x: 15.0,
            y: 25.0,
            width: 100.0,
            height: 80.0,
            color: ColorData {
                r: 0,
                g: 120,
                b: 255,
                a: 255,
            },
            stroke_width: 2.0,
            fill: Some(ColorData {
                r: 200,
                g: 220,
                b: 255,
                a: 100,
            }),
            rotation: 0.5,
            corner_radius: 8.0,
            label: Some(ShapeLabel {
                text: "Node A".to_string(),
                font_size: 14.0,
                color: ColorData {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                alignment: TextAlign::Center,
                vertical_alignment: VerticalAlign::Middle,
            }),
            locked: true,
            curve_points: Vec::new(),
        };

        let json = serde_json::to_string(&obj).expect("serialize");
        let decoded: DrawObject = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(obj, decoded);
    }

    #[test]
    fn test_draw_object_text_and_sticky_roundtrip() {
        let text_obj = DrawObject::Text {
            id: "text-1".to_string(),
            x: 50.0,
            y: 50.0,
            text: "Hello RustNotes!\nLine 2".to_string(),
            font_size: 18.0,
            color: ColorData {
                r: 30,
                g: 30,
                b: 30,
                a: 255,
            },
            rotation: 0.0,
        };
        let json_text = serde_json::to_string(&text_obj).expect("serialize");
        let decoded_text: DrawObject = serde_json::from_str(&json_text).expect("deserialize");
        assert_eq!(text_obj, decoded_text);

        let sticky_obj = DrawObject::StickyNote {
            id: "sticky-1".to_string(),
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 150.0,
            text: "Remember to test edge cases".to_string(),
            color: ColorData {
                r: 254,
                g: 243,
                b: 189,
                a: 255,
            },
        };
        let json_sticky = serde_json::to_string(&sticky_obj).expect("serialize");
        let decoded_sticky: DrawObject = serde_json::from_str(&json_sticky).expect("deserialize");
        assert_eq!(sticky_obj, decoded_sticky);
    }

    #[test]
    fn test_page_and_notebook_roundtrip() {
        let mut page = Page::new(PageTemplate::Cornell);
        page.objects.push(DrawObject::Text {
            id: "t1".into(),
            x: 10.0,
            y: 10.0,
            text: "Cornell Notes Header".into(),
            font_size: 16.0,
            color: ColorData {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            rotation: 0.0,
        });

        let mut nb = Notebook::new(
            "Test Notebook",
            PageTemplate::Cornell,
            Color32::from_rgb(49, 85, 119),
        );
        nb.pages = vec![page];
        nb.favorite = true;

        let json = serde_json::to_string(&nb).expect("serialize");
        let decoded: Notebook = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(nb, decoded);
    }

    #[test]
    fn test_legacy_library_json_compatibility() {
        // Old JSON without schema_version field
        let legacy_json = r#"{
            "notebooks": [],
            "folders": []
        }"#;

        let lib: Library =
            serde_json::from_str(legacy_json).expect("parse legacy without schema_version");
        assert_eq!(lib.schema_version, 1);

        // Old JSON with "version": 1
        let legacy_v1 = r#"{
            "version": 1,
            "notebooks": [],
            "folders": []
        }"#;

        let lib_v1: Library =
            serde_json::from_str(legacy_v1).expect("parse legacy with version: 1");
        assert_eq!(lib_v1.schema_version, 1);
    }

    #[test]
    fn test_legacy_notebook_schema_version_compatibility() {
        // Legacy JSON without schema_version field defaults to 1
        let legacy_nb_json = r#"{
            "id": "legacy-nb-1",
            "title": "Legacy Notebook",
            "cover_color": {"r": 255, "g": 255, "b": 255, "a": 255},
            "pages": [],
            "created_at": 1000,
            "updated_at": 1000
        }"#;

        let nb: Notebook = serde_json::from_str(legacy_nb_json)
            .expect("parse legacy notebook without schema_version");
        assert_eq!(nb.schema_version, 1);

        // Current created notebook uses CURRENT_NOTEBOOK_SCHEMA_VERSION (2)
        let new_nb = Notebook::new("Current NB", PageTemplate::Blank, Color32::WHITE);
        assert_eq!(new_nb.schema_version, CURRENT_NOTEBOOK_SCHEMA_VERSION);
    }

    #[test]
    fn test_paper_size_presets_and_orientation() {
        assert_eq!(
            PaperSizePreset::A4.dimensions(PageOrientation::Portrait),
            (595.0, 842.0)
        );
        assert_eq!(
            PaperSizePreset::A4.dimensions(PageOrientation::Landscape),
            (842.0, 595.0)
        );
        assert_eq!(
            PaperSizePreset::Letter.dimensions(PageOrientation::Portrait),
            (612.0, 792.0)
        );
        assert_eq!(
            PaperSizePreset::A5.dimensions(PageOrientation::Portrait),
            (420.0, 595.0)
        );
        assert_eq!(
            PaperSizePreset::Legacy.dimensions(PageOrientation::Portrait),
            (800.0, 1100.0)
        );
        assert_eq!(
            PaperSizePreset::Slide16x9.dimensions(PageOrientation::Landscape),
            (1280.0, 720.0)
        );
        assert_eq!(
            PaperSizePreset::Slide16x9.dimensions(PageOrientation::Portrait),
            (720.0, 1280.0)
        );
        assert_eq!(
            PaperSizePreset::Standard4x3.dimensions(PageOrientation::Landscape),
            (1024.0, 768.0)
        );
        assert_eq!(
            PaperSizePreset::Square.dimensions(PageOrientation::Portrait),
            (800.0, 800.0)
        );

        // Test match_dimensions
        assert_eq!(
            PaperSizePreset::match_dimensions(1280.0, 720.0),
            Some((PaperSizePreset::Slide16x9, PageOrientation::Landscape))
        );
        assert_eq!(
            PaperSizePreset::match_dimensions(720.0, 1280.0),
            Some((PaperSizePreset::Slide16x9, PageOrientation::Portrait))
        );
        assert_eq!(
            PaperSizePreset::match_dimensions(595.0, 842.0),
            Some((PaperSizePreset::A4, PageOrientation::Portrait))
        );
        assert_eq!(
            PaperSizePreset::match_dimensions(842.0, 595.0),
            Some((PaperSizePreset::A4, PageOrientation::Landscape))
        );
        assert_eq!(PaperSizePreset::match_dimensions(999.0, 123.0), None);

        // Test PageTemplate ALL and descriptions
        assert_eq!(PageTemplate::ALL.len(), 10);
        for tmpl in PageTemplate::ALL {
            assert!(!tmpl.name().is_empty());
            assert!(!tmpl.description().is_empty());
        }

        let p_a4 = Page::new_preset(
            PageTemplate::Grid,
            PaperSizePreset::A4,
            PageOrientation::Portrait,
        );
        assert_eq!(p_a4.width, 595.0);
        assert_eq!(p_a4.height, 842.0);

        let nb = Notebook::new_with_preset(
            "A4 Notebook",
            PageTemplate::Ruled,
            Color32::BLUE,
            PaperSizePreset::A4,
            PageOrientation::Landscape,
        );
        assert_eq!(nb.pages[0].width, 842.0);
        assert_eq!(nb.pages[0].height, 595.0);
    }

    #[test]
    fn test_notebook_page_operations() {
        let mut nb = Notebook::new("Multi-Page Test", PageTemplate::Blank, Color32::BLACK);
        assert_eq!(nb.pages.len(), 1);

        // Insert page after
        let idx1 = nb.insert_page(1, PageTemplate::Ruled, 600.0, 800.0);
        assert_eq!(idx1, 1);
        assert_eq!(nb.pages.len(), 2);
        assert_eq!(nb.pages[1].template, PageTemplate::Ruled);

        // Insert page with color and template
        let test_color = Color32::from_rgb(240, 245, 235);
        let idx2 = nb.insert_page_with_color(2, PageTemplate::Isometric, 1280.0, 720.0, test_color);
        assert_eq!(idx2, 2);
        assert_eq!(nb.pages.len(), 3);
        assert_eq!(nb.pages[2].template, PageTemplate::Isometric);
        assert_eq!(nb.pages[2].width, 1280.0);
        assert_eq!(nb.pages[2].height, 720.0);
        assert_eq!(nb.pages[2].paper_color.to_color32(), test_color);

        // Duplicate page
        let dup_idx = nb.duplicate_page(1).expect("duplicate page 1");
        assert_eq!(dup_idx, 2);
        assert_eq!(nb.pages.len(), 4);
        assert_ne!(nb.pages[1].id, nb.pages[2].id);

        // Reorder / move page: move page at index 0 to index 2
        let p0_id = nb.pages[0].id.clone();
        assert!(nb.move_page(0, 2));
        assert_eq!(nb.pages[2].id, p0_id);

        // Resize page
        assert!(nb.resize_page(0, 595.0, 842.0));
        assert_eq!(nb.pages[0].width, 595.0);
        assert_eq!(nb.pages[0].height, 842.0);

        // Delete page
        let after_del = nb.delete_page(1).expect("delete page 1");
        assert_eq!(nb.pages.len(), 3);
        assert_eq!(after_del, 1);

        // Delete pages down to last remaining
        let _ = nb.delete_page(0);
        let _ = nb.delete_page(0);
        assert_eq!(nb.pages.len(), 1);
        let err = nb.delete_page(0);
        assert!(err.is_err());
    }

    #[test]
    fn test_draw_object_image_roundtrip() {
        let img = DrawObject::Image {
            id: "img-123".to_string(),
            asset_id: "hash-456.png".to_string(),
            x: 50.0,
            y: 75.0,
            width: 320.0,
            height: 240.0,
            rotation: 0.0,
            lock_aspect: true,
        };

        let json = serde_json::to_string(&img).expect("serialize image");
        let decoded: DrawObject = serde_json::from_str(&json).expect("deserialize image");
        assert_eq!(img, decoded);
    }

    #[test]
    fn test_page_background_roundtrip() {
        let mut page = Page::with_size(PageTemplate::Blank, 595.0, 842.0);
        page.background = Some(PageBackground::Pdf {
            asset_id: "doc-sample.pdf".to_string(),
            page_index: 3,
        });

        let json = serde_json::to_string(&page).expect("serialize page");
        let decoded: Page = serde_json::from_str(&json).expect("deserialize page");
        assert_eq!(page, decoded);
        assert_eq!(
            decoded.effective_background(),
            PageBackground::Pdf {
                asset_id: "doc-sample.pdf".to_string(),
                page_index: 3,
            }
        );
    }

    #[test]
    fn test_outline_and_bookmark_roundtrip() {
        let mut nb = Notebook::new_with_preset(
            "Test Outline",
            PageTemplate::Ruled,
            Color32::from_rgb(10, 20, 30),
            PaperSizePreset::A4,
            PageOrientation::Portrait,
        );
        nb.pages[0].bookmarked = true;
        nb.outline.push(OutlineItem {
            id: "out-1".to_string(),
            title: "Bab 1: Pendahuluan".to_string(),
            page_index: 0,
        });

        let json = serde_json::to_string(&nb).expect("serialize notebook with outline");
        let decoded: Notebook = serde_json::from_str(&json).expect("deserialize notebook");
        assert!(decoded.pages[0].bookmarked);
        assert_eq!(decoded.outline.len(), 1);
        assert_eq!(decoded.outline[0].title, "Bab 1: Pendahuluan");
    }

    #[test]
    fn test_custom_template_roundtrip() {
        let tmpl = CustomTemplate {
            id: "ct-1".to_string(),
            name: "Planner Mingguan".to_string(),
            template: PageTemplate::Grid,
            background: None,
            paper_color: ColorData::from(Color32::from_rgb(250, 248, 240)),
            width: 595.0,
            height: 842.0,
        };

        let json = serde_json::to_string(&tmpl).expect("serialize custom template");
        let decoded: CustomTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tmpl, decoded);
    }

    #[test]
    fn test_folder_nesting_model() {
        let root_f = Folder::new("Root Folder", None);
        let child_f = Folder::new("Sub Folder", Some(root_f.id.clone()));

        assert_eq!(child_f.parent_id.as_deref(), Some(root_f.id.as_str()));
    }

    #[test]
    fn test_app_settings_roundtrip() {
        let mut settings = AppSettings {
            theme_mode: ThemeMode::System,
            zoom_window_scale: 3.0,
            palm_rejection: false,
            language: AppLanguage::English,
            ..Default::default()
        };
        settings.pen_widths.push(12.0);

        let json = serde_json::to_string(&settings).expect("serialize settings");
        let decoded: AppSettings = serde_json::from_str(&json).expect("deserialize settings");
        assert_eq!(settings, decoded);
        assert_eq!(decoded.theme_mode, ThemeMode::System);
        assert_eq!(decoded.language, AppLanguage::English);
        assert!(!decoded.palm_rejection);
        assert_eq!(decoded.pen_widths.len(), 5);
        assert_eq!(decoded.language.tr("Halo", "Hello"), "Hello");
    }

    #[test]
    fn test_connector_roundtrip_and_relation_index() {
        let conn = DrawObject::Connector {
            id: "conn-1".to_string(),
            start: ConnectorEnd::Attached {
                shape_id: "s1".to_string(),
                site: ConnectionSite::Right,
            },
            end: ConnectorEnd::Attached {
                shape_id: "s2".to_string(),
                site: ConnectionSite::Left,
            },
            route: ConnectorRoute::Orthogonal,
            control_points: vec![Point2D::new(100.0, 50.0), Point2D::new(100.0, 150.0)],
            style: ConnectorStyle {
                stroke_color: ColorData {
                    r: 40,
                    g: 80,
                    b: 200,
                    a: 255,
                },
                stroke_width: 2.5,
                start_marker: MarkerKind::None,
                end_marker: MarkerKind::Arrow,
            },
            locked: false,
        };

        let json = serde_json::to_string(&conn).expect("serialize connector");
        let decoded: DrawObject = serde_json::from_str(&json).expect("deserialize connector");
        assert_eq!(conn, decoded);

        let objects = vec![
            DrawObject::Shape {
                id: "s1".to_string(),
                shape_type: ShapeKind::Rectangle,
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 60.0,
                color: ColorData {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: None,
                rotation: 0.0,
                corner_radius: 6.0,
                label: None,
                locked: false,
                curve_points: Vec::new(),
            },
            DrawObject::Shape {
                id: "s2".to_string(),
                shape_type: ShapeKind::Diamond,
                x: 200.0,
                y: 120.0,
                width: 80.0,
                height: 60.0,
                color: ColorData {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: None,
                rotation: 0.0,
                corner_radius: 0.0,
                label: None,
                locked: false,
                curve_points: Vec::new(),
            },
            conn,
        ];

        let mut index = ConnectorRelationIndex::default();
        index.rebuild_from_objects(&objects);
        assert_eq!(index.connectors_for_shape("s1"), vec!["conn-1".to_string()]);
        assert_eq!(index.connectors_for_shape("s2"), vec!["conn-1".to_string()]);
        assert!(index.connectors_for_shape("non-existent").is_empty());
    }

    #[test]
    fn test_sanitize_orphan_connector_references() {
        let mut page = Page::new(PageTemplate::Blank);
        page.objects.push(DrawObject::Shape {
            id: "valid-shape".to_string(),
            shape_type: ShapeKind::Rectangle,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            color: ColorData {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            stroke_width: 2.0,
            fill: None,
            rotation: 0.0,
            corner_radius: 0.0,
            label: None,
            locked: false,
            curve_points: Vec::new(),
        });
        page.objects.push(DrawObject::Connector {
            id: "conn-orphan".to_string(),
            start: ConnectorEnd::Attached {
                shape_id: "valid-shape".to_string(),
                site: ConnectionSite::Bottom,
            },
            end: ConnectorEnd::Attached {
                shape_id: "deleted-shape".to_string(),
                site: ConnectionSite::Top,
            },
            route: ConnectorRoute::Straight,
            control_points: Vec::new(),
            style: ConnectorStyle::default(),
            locked: false,
        });

        let repaired = page.sanitize_connector_references();
        assert_eq!(repaired, 1);

        if let DrawObject::Connector { start, end, .. } = &page.objects[1] {
            assert!(matches!(start, ConnectorEnd::Attached { .. }));
            assert!(matches!(end, ConnectorEnd::Free { .. }));
        } else {
            panic!("Expected connector object");
        }
    }

    #[test]
    fn test_shape_curve_and_diamond_variants() {
        let curve_obj = DrawObject::Shape {
            id: "curve-1".to_string(),
            shape_type: ShapeKind::Curve,
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
            color: ColorData {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            stroke_width: 3.0,
            fill: None,
            rotation: 0.0,
            corner_radius: 0.0,
            label: None,
            locked: false,
            curve_points: vec![
                Point2D::new(10.0, 20.0),
                Point2D::new(40.0, 80.0),
                Point2D::new(80.0, 10.0),
                Point2D::new(110.0, 100.0),
            ],
        };
        let json = serde_json::to_string(&curve_obj).expect("serialize curve");
        let decoded: DrawObject = serde_json::from_str(&json).expect("deserialize curve");
        assert_eq!(curve_obj, decoded);
    }

    #[test]
    fn test_shortcut_settings_and_keybindings_roundtrip() {
        let mut settings = ShortcutSettings::default();
        assert_eq!(
            settings.get(ShortcutAction::ToolPen),
            KeyBinding::plain("P")
        );
        assert_eq!(settings.get(ShortcutAction::Undo), KeyBinding::ctrl("Z"));
        assert_eq!(
            settings.get(ShortcutAction::ToggleGrid),
            KeyBinding::alt("G")
        );

        // Test display_string
        assert_eq!(KeyBinding::plain("P").display_string(), "P");
        assert_eq!(KeyBinding::ctrl("S").display_string(), "Ctrl + S");
        assert_eq!(KeyBinding::alt("G").display_string(), "Alt + G");
        assert_eq!(
            KeyBinding::new("Z", true, false, true).display_string(),
            "Ctrl + Shift + Z"
        );

        // Test set and persistence
        settings.set(
            ShortcutAction::ToolPen,
            KeyBinding::new("B", true, false, false),
        );
        assert_eq!(settings.get(ShortcutAction::ToolPen), KeyBinding::ctrl("B"));

        // Test JSON roundtrip
        let json = serde_json::to_string(&settings).expect("serialize shortcuts");
        let decoded: ShortcutSettings = serde_json::from_str(&json).expect("deserialize shortcuts");
        assert_eq!(settings, decoded);

        // Test reset_all
        settings.reset_all();
        assert_eq!(
            settings.get(ShortcutAction::ToolPen),
            KeyBinding::plain("P")
        );
    }
}
