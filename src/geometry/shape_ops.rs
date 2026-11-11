use crate::model::{
    ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
    MarkerKind, Point2D, ShapeKind, ShapeLabel,
};
use eframe::egui::{Pos2, Vec2};
use uuid::Uuid;

/// Jenis diagram starter siap pakai untuk inisialisasi diagram cepat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramStarterKind {
    Flowchart,
    MindMap,
    Architecture3Tier,
}

impl DiagramStarterKind {
    pub const ALL: [DiagramStarterKind; 3] = [
        DiagramStarterKind::Flowchart,
        DiagramStarterKind::MindMap,
        DiagramStarterKind::Architecture3Tier,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Flowchart => "Alur Kerja (Flowchart)",
            Self::MindMap => "Peta Pikiran (Mind Map)",
            Self::Architecture3Tier => "Arsitektur Sistem (3-Tier)",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Flowchart => {
                "Start, proses data, percabangan keputusan, dan selesai terhubung panah"
            }
            Self::MindMap => {
                "Gagasan utama di pusat dengan 4 cabang topik terhubung konektor lengkung"
            }
            Self::Architecture3Tier => {
                "Presentation Tier, Application Logic, dan Data Storage terhubung"
            }
        }
    }
}

/// Menghasilkan sekumpulan bentuk dan konektor terhubung siap pakai untuk diagram awal.
pub fn generate_diagram_starter(
    kind: DiagramStarterKind,
    page_width: f32,
    page_height: f32,
) -> Vec<DrawObject> {
    let mut objects = Vec::new();
    let cx = page_width * 0.5;

    let dark_navy = ColorData {
        r: 30,
        g: 45,
        b: 65,
        a: 255,
    };
    let conn_style = ConnectorStyle {
        stroke_color: ColorData {
            r: 45,
            g: 55,
            b: 72,
            a: 255,
        },
        stroke_width: 2.2,
        start_marker: MarkerKind::None,
        end_marker: MarkerKind::Arrow,
    };

    match kind {
        DiagramStarterKind::Flowchart => {
            let start_id = Uuid::new_v4().to_string();
            let proc_id = Uuid::new_v4().to_string();
            let dec_id = Uuid::new_v4().to_string();
            let end_id = Uuid::new_v4().to_string();

            // 1. Start Pill
            objects.push(DrawObject::Shape {
                id: start_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 75.0,
                y: 90.0,
                width: 150.0,
                height: 48.0,
                color: ColorData {
                    r: 40,
                    g: 167,
                    b: 69,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 40,
                    g: 167,
                    b: 69,
                    a: 45,
                }),
                rotation: 0.0,
                corner_radius: 24.0,
                label: Some(ShapeLabel {
                    text: "Mulai (Start)".to_string(),
                    font_size: 15.0,
                    color: dark_navy.clone(),
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // 2. Process Rectangle
            objects.push(DrawObject::Shape {
                id: proc_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 85.0,
                y: 195.0,
                width: 170.0,
                height: 56.0,
                color: ColorData {
                    r: 0,
                    g: 114,
                    b: 206,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 0,
                    g: 114,
                    b: 206,
                    a: 40,
                }),
                rotation: 0.0,
                corner_radius: 6.0,
                label: Some(ShapeLabel {
                    text: "Proses Data Input".to_string(),
                    font_size: 15.0,
                    color: dark_navy.clone(),
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // 3. Decision Diamond
            objects.push(DrawObject::Shape {
                id: dec_id.clone(),
                shape_type: ShapeKind::Diamond,
                x: cx - 80.0,
                y: 310.0,
                width: 160.0,
                height: 75.0,
                color: ColorData {
                    r: 220,
                    g: 130,
                    b: 0,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 255,
                    g: 193,
                    b: 7,
                    a: 45,
                }),
                rotation: 0.0,
                corner_radius: 0.0,
                label: Some(ShapeLabel {
                    text: "Valid?".to_string(),
                    font_size: 15.0,
                    color: dark_navy.clone(),
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // 4. End Pill
            objects.push(DrawObject::Shape {
                id: end_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 75.0,
                y: 445.0,
                width: 150.0,
                height: 48.0,
                color: ColorData {
                    r: 220,
                    g: 53,
                    b: 69,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 220,
                    g: 53,
                    b: 69,
                    a: 45,
                }),
                rotation: 0.0,
                corner_radius: 24.0,
                label: Some(ShapeLabel {
                    text: "Selesai (End)".to_string(),
                    font_size: 15.0,
                    color: dark_navy,
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // Connectors
            objects.push(DrawObject::Connector {
                id: Uuid::new_v4().to_string(),
                start: ConnectorEnd::Attached {
                    shape_id: start_id,
                    site: ConnectionSite::Bottom,
                },
                end: ConnectorEnd::Attached {
                    shape_id: proc_id.clone(),
                    site: ConnectionSite::Top,
                },
                route: ConnectorRoute::Orthogonal,
                control_points: Vec::new(),
                style: conn_style.clone(),
                locked: false,
            });
            objects.push(DrawObject::Connector {
                id: Uuid::new_v4().to_string(),
                start: ConnectorEnd::Attached {
                    shape_id: proc_id,
                    site: ConnectionSite::Bottom,
                },
                end: ConnectorEnd::Attached {
                    shape_id: dec_id.clone(),
                    site: ConnectionSite::Top,
                },
                route: ConnectorRoute::Orthogonal,
                control_points: Vec::new(),
                style: conn_style.clone(),
                locked: false,
            });
            objects.push(DrawObject::Connector {
                id: Uuid::new_v4().to_string(),
                start: ConnectorEnd::Attached {
                    shape_id: dec_id,
                    site: ConnectionSite::Bottom,
                },
                end: ConnectorEnd::Attached {
                    shape_id: end_id,
                    site: ConnectionSite::Top,
                },
                route: ConnectorRoute::Orthogonal,
                control_points: Vec::new(),
                style: conn_style,
                locked: false,
            });
        }
        DiagramStarterKind::MindMap => {
            let cy = page_height * 0.45;
            let center_id = Uuid::new_v4().to_string();
            let b1_id = Uuid::new_v4().to_string();
            let b2_id = Uuid::new_v4().to_string();
            let b3_id = Uuid::new_v4().to_string();
            let b4_id = Uuid::new_v4().to_string();

            // Center Idea
            objects.push(DrawObject::Shape {
                id: center_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 80.0,
                y: cy - 35.0,
                width: 160.0,
                height: 70.0,
                color: ColorData {
                    r: 111,
                    g: 66,
                    b: 193,
                    a: 255,
                },
                stroke_width: 2.5,
                fill: Some(ColorData {
                    r: 111,
                    g: 66,
                    b: 193,
                    a: 45,
                }),
                rotation: 0.0,
                corner_radius: 16.0,
                label: Some(ShapeLabel {
                    text: "Gagasan Utama".to_string(),
                    font_size: 16.0,
                    color: dark_navy.clone(),
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // 4 Branches
            let branches = [
                (
                    b1_id.clone(),
                    cx + 140.0,
                    cy - 110.0,
                    "Riset & Ide",
                    ColorData {
                        r: 0,
                        g: 114,
                        b: 206,
                        a: 255,
                    },
                    ConnectionSite::Left,
                    ConnectionSite::Right,
                ),
                (
                    b2_id.clone(),
                    cx + 140.0,
                    cy + 65.0,
                    "Pengembangan",
                    ColorData {
                        r: 40,
                        g: 167,
                        b: 69,
                        a: 255,
                    },
                    ConnectionSite::Left,
                    ConnectionSite::Right,
                ),
                (
                    b3_id.clone(),
                    cx - 260.0,
                    cy - 110.0,
                    "Desain & UI",
                    ColorData {
                        r: 253,
                        g: 126,
                        b: 20,
                        a: 255,
                    },
                    ConnectionSite::Right,
                    ConnectionSite::Left,
                ),
                (
                    b4_id.clone(),
                    cx - 260.0,
                    cy + 65.0,
                    "Peluncuran",
                    ColorData {
                        r: 220,
                        g: 53,
                        b: 69,
                        a: 255,
                    },
                    ConnectionSite::Right,
                    ConnectionSite::Left,
                ),
            ];

            for (bid, bx, by, text, col, target_site, source_site) in branches {
                let fill_col = ColorData {
                    r: col.r,
                    g: col.g,
                    b: col.b,
                    a: 35,
                };
                objects.push(DrawObject::Shape {
                    id: bid.clone(),
                    shape_type: ShapeKind::Rectangle,
                    x: bx,
                    y: by,
                    width: 125.0,
                    height: 46.0,
                    color: col,
                    stroke_width: 2.0,
                    fill: Some(fill_col),
                    rotation: 0.0,
                    corner_radius: 10.0,
                    label: Some(ShapeLabel {
                        text: text.to_string(),
                        font_size: 14.0,
                        color: dark_navy.clone(),
                        alignment: crate::model::TextAlign::Center,
                        vertical_alignment: crate::model::VerticalAlign::Middle,
                    }),
                    locked: false,
                    curve_points: Vec::new(),
                });

                // Curved connector
                objects.push(DrawObject::Connector {
                    id: Uuid::new_v4().to_string(),
                    start: ConnectorEnd::Attached {
                        shape_id: center_id.clone(),
                        site: source_site,
                    },
                    end: ConnectorEnd::Attached {
                        shape_id: bid,
                        site: target_site,
                    },
                    route: ConnectorRoute::Curved,
                    control_points: Vec::new(),
                    style: conn_style.clone(),
                    locked: false,
                });
            }
        }
        DiagramStarterKind::Architecture3Tier => {
            let t1_id = Uuid::new_v4().to_string();
            let t2_id = Uuid::new_v4().to_string();
            let t3_id = Uuid::new_v4().to_string();

            // Tier 1: Client
            objects.push(DrawObject::Shape {
                id: t1_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 110.0,
                y: 95.0,
                width: 220.0,
                height: 65.0,
                color: ColorData {
                    r: 0,
                    g: 114,
                    b: 206,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 0,
                    g: 114,
                    b: 206,
                    a: 35,
                }),
                rotation: 0.0,
                corner_radius: 8.0,
                label: Some(ShapeLabel {
                    text: "Presentation Tier\n(Web & Mobile Apps)".to_string(),
                    font_size: 14.0,
                    color: dark_navy.clone(),
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // Tier 2: Logic
            objects.push(DrawObject::Shape {
                id: t2_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 110.0,
                y: 235.0,
                width: 220.0,
                height: 65.0,
                color: ColorData {
                    r: 111,
                    g: 66,
                    b: 193,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 111,
                    g: 66,
                    b: 193,
                    a: 35,
                }),
                rotation: 0.0,
                corner_radius: 8.0,
                label: Some(ShapeLabel {
                    text: "Application Logic Tier\n(REST API & Microservices)".to_string(),
                    font_size: 14.0,
                    color: dark_navy.clone(),
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            // Tier 3: Database
            objects.push(DrawObject::Shape {
                id: t3_id.clone(),
                shape_type: ShapeKind::Rectangle,
                x: cx - 110.0,
                y: 375.0,
                width: 220.0,
                height: 65.0,
                color: ColorData {
                    r: 40,
                    g: 167,
                    b: 69,
                    a: 255,
                },
                stroke_width: 2.0,
                fill: Some(ColorData {
                    r: 40,
                    g: 167,
                    b: 69,
                    a: 35,
                }),
                rotation: 0.0,
                corner_radius: 8.0,
                label: Some(ShapeLabel {
                    text: "Data Storage Tier\n(PostgreSQL / Redis Cache)".to_string(),
                    font_size: 14.0,
                    color: dark_navy,
                    alignment: crate::model::TextAlign::Center,
                    vertical_alignment: crate::model::VerticalAlign::Middle,
                }),
                locked: false,
                curve_points: Vec::new(),
            });

            let mut bi_conn_style = conn_style;
            bi_conn_style.start_marker = MarkerKind::Arrow;
            bi_conn_style.end_marker = MarkerKind::Arrow;

            objects.push(DrawObject::Connector {
                id: Uuid::new_v4().to_string(),
                start: ConnectorEnd::Attached {
                    shape_id: t1_id,
                    site: ConnectionSite::Bottom,
                },
                end: ConnectorEnd::Attached {
                    shape_id: t2_id.clone(),
                    site: ConnectionSite::Top,
                },
                route: ConnectorRoute::Orthogonal,
                control_points: Vec::new(),
                style: bi_conn_style.clone(),
                locked: false,
            });
            objects.push(DrawObject::Connector {
                id: Uuid::new_v4().to_string(),
                start: ConnectorEnd::Attached {
                    shape_id: t2_id,
                    site: ConnectionSite::Bottom,
                },
                end: ConnectorEnd::Attached {
                    shape_id: t3_id,
                    site: ConnectionSite::Top,
                },
                route: ConnectorRoute::Orthogonal,
                control_points: Vec::new(),
                style: bi_conn_style,
                locked: false,
            });
        }
    }

    objects
}

/// Jenis-jenis handle kontrol transformasi geometri untuk bentuk atau seleksi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransformHandleKind {
    /// Sudut Kiri Atas (Scale proporsional)
    NorthWest,
    /// Tepi Atas (Resize satu sumbu vertikal)
    North,
    /// Sudut Kanan Atas (Scale proporsional)
    NorthEast,
    /// Tepi Kanan (Resize satu sumbu horizontal)
    East,
    /// Sudut Kanan Bawah (Scale proporsional)
    SouthEast,
    /// Tepi Bawah (Resize satu sumbu vertikal)
    South,
    /// Sudut Kiri Bawah (Scale proporsional)
    SouthWest,
    /// Tepi Kiri (Resize satu sumbu horizontal)
    West,
    /// Tangkai rotasi di atas
    Rotation,
}

impl TransformHandleKind {
    /// Cek apakah handle merupakan handle sudut (skala proporsional).
    pub fn is_corner(self) -> bool {
        matches!(
            self,
            Self::NorthWest | Self::NorthEast | Self::SouthEast | Self::SouthWest
        )
    }

    /// Cek apakah handle merupakan handle tepi sisi (resize satu sumbu).
    pub fn is_edge(self) -> bool {
        matches!(self, Self::North | Self::East | Self::South | Self::West)
    }
}

/// Struktur data posisi handle transformasi di ruang koordinat halaman.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformHandle {
    pub kind: TransformHandleKind,
    pub pos: Point2D,
}

/// Menghitung seluruh posisi 9 handle transformasi (4 sudut, 4 tepi sisi, 1 rotasi)
/// untuk bounding box yang dapat berotasi dengan sudut `rotation` (radian).
pub fn compute_transform_handles(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
    stem_length: f32,
) -> Vec<TransformHandle> {
    let cx = x + width * 0.5;
    let cy = y + height * 0.5;
    let hw = width.abs() * 0.5;
    let hh = height.abs() * 0.5;

    let cos_r = rotation.cos();
    let sin_r = rotation.sin();

    // Fungsi rotasi lokal terhadap titik pusat bentuk
    let to_world = |lx: f32, ly: f32| -> Point2D {
        Point2D::new(cx + lx * cos_r - ly * sin_r, cy + lx * sin_r + ly * cos_r)
    };

    vec![
        // 4 Sudut (lingkaran skala proporsional)
        TransformHandle {
            kind: TransformHandleKind::NorthWest,
            pos: to_world(-hw, -hh),
        },
        TransformHandle {
            kind: TransformHandleKind::NorthEast,
            pos: to_world(hw, -hh),
        },
        TransformHandle {
            kind: TransformHandleKind::SouthEast,
            pos: to_world(hw, hh),
        },
        TransformHandle {
            kind: TransformHandleKind::SouthWest,
            pos: to_world(-hw, hh),
        },
        // 4 Tepi Sisi (persegi resize satu sumbu)
        TransformHandle {
            kind: TransformHandleKind::North,
            pos: to_world(0.0, -hh),
        },
        TransformHandle {
            kind: TransformHandleKind::East,
            pos: to_world(hw, 0.0),
        },
        TransformHandle {
            kind: TransformHandleKind::South,
            pos: to_world(0.0, hh),
        },
        TransformHandle {
            kind: TransformHandleKind::West,
            pos: to_world(-hw, 0.0),
        },
        // 1 Tangkai Rotasi (menonjol di atas sisi North)
        TransformHandle {
            kind: TransformHandleKind::Rotation,
            pos: to_world(0.0, -hh - stem_length.max(14.0)),
        },
    ]
}

/// Hit-test untuk mendeteksi handle yang disentuh oleh kursor pointer.
/// Mengembalikan handle terdekat dalam jarak `tolerance`.
pub fn hit_test_transform_handles(
    handles: &[TransformHandle],
    click_pos: Point2D,
    tolerance: f32,
) -> Option<TransformHandleKind> {
    let mut best_hit = None;
    let mut min_dist_sq = tolerance * tolerance;

    for handle in handles {
        let d_sq = handle.pos.dist_sq(click_pos);
        if d_sq <= min_dist_sq {
            min_dist_sq = d_sq;
            best_hit = Some(handle.kind);
        }
    }

    best_hit
}

/// Representasi bingkai seleksi berotasi yang otoritatif (Task 7).
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionFrame {
    /// Titik pusat / poros rotasi dalam koordinat halaman
    pub pivot: Pos2,
    /// Dimensi lokal (lebar dan tinggi sebelum rotasi)
    pub local_size: Vec2,
    /// Sudut orientasi bingkai dalam radian
    pub rotation: f32,
    /// 4 Sudut bingkai terotasi: [NorthWest, NorthEast, SouthEast, SouthWest]
    pub corners: [Pos2; 4],
    /// Kumpulan 9 handle transformasi (8 resize + 1 rotasi)
    pub handles: Vec<TransformHandle>,
}

impl SelectionFrame {
    /// Membuat bingkai seleksi berotasi dari objek-objek yang dipilih.
    pub fn from_objects(selected: &[&crate::model::DrawObject], stem_length: f32) -> Option<Self> {
        if selected.is_empty() {
            return None;
        }

        // Jika hanya 1 objek dipilih
        if selected.len() == 1 {
            let obj = selected[0];
            match obj {
                crate::model::DrawObject::Shape {
                    x,
                    y,
                    width,
                    height,
                    rotation,
                    shape_type,
                    ..
                } => {
                    if *shape_type == crate::model::ShapeKind::Line {
                        // Garis memiliki penanganan handle ujung tersendiri (single_line)
                        return None;
                    }
                    let w = width.abs().max(4.0);
                    let h = height.abs().max(4.0);
                    let pivot = Pos2::new(*x + *width * 0.5, *y + *height * 0.5);
                    let rot = *rotation;
                    return Some(Self::new(pivot, Vec2::new(w, h), rot, stem_length));
                }
                crate::model::DrawObject::Image {
                    x,
                    y,
                    width,
                    height,
                    rotation,
                    ..
                } => {
                    let w = width.abs().max(4.0);
                    let h = height.abs().max(4.0);
                    let pivot = Pos2::new(*x + *width * 0.5, *y + *height * 0.5);
                    let rot = *rotation;
                    return Some(Self::new(pivot, Vec2::new(w, h), rot, stem_length));
                }
                crate::model::DrawObject::StickyNote {
                    x,
                    y,
                    width,
                    height,
                    rotation,
                    ..
                } => {
                    let w = width.abs().max(4.0);
                    let h = height.abs().max(4.0);
                    let pivot = Pos2::new(*x + *width * 0.5, *y + *height * 0.5);
                    let rot = *rotation;
                    return Some(Self::new(pivot, Vec2::new(w, h), rot, stem_length));
                }
                crate::model::DrawObject::Text {
                    x,
                    y,
                    font_size,
                    text,
                    rotation,
                    ..
                } => {
                    let lines: Vec<&str> = text.lines().collect();
                    let line_count = lines.len().max(1) as f32;
                    let max_line_len =
                        lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
                    let approx_w = (max_line_len * (font_size * 0.58).max(8.0)).max(20.0);
                    let approx_h = line_count * (font_size * 1.35).max(18.0);
                    let cos_a = rotation.cos();
                    let sin_a = rotation.sin();
                    let rot_v = |lx: f32, ly: f32| {
                        Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a)
                    };
                    let p0 = Pos2::new(*x, *y);
                    let pivot = p0 + rot_v(approx_w * 0.5, approx_h * 0.5);
                    let rot = *rotation;
                    return Some(Self::new(
                        pivot,
                        Vec2::new(approx_w, approx_h),
                        rot,
                        stem_length,
                    ));
                }
                crate::model::DrawObject::Connector { .. } => {
                    // Connector menggunakan waypoint handles tersendiri
                    return None;
                }
                crate::model::DrawObject::Stroke { .. } => {
                    let b = crate::geometry::object_bounds(obj);
                    if !b.is_finite() {
                        return None;
                    }
                    return Some(Self::new(b.center(), b.size(), 0.0, stem_length));
                }
            }
        }

        // Multi-selection:
        // Gunakan combined bounding box axis-aligned agar stabil dan dapat diprediksi
        let b = crate::geometry::combined_bounds(selected)?;
        if !b.is_finite() {
            return None;
        }
        Some(Self::new(b.center(), b.size(), 0.0, stem_length))
    }

    /// Konstruktor dari poros, dimensi lokal, dan rotasi
    pub fn new(pivot: Pos2, local_size: Vec2, rotation: f32, stem_length: f32) -> Self {
        let hw = local_size.x * 0.5;
        let hh = local_size.y * 0.5;
        let cos_a = rotation.cos();
        let sin_a = rotation.sin();
        let rot_v = |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);

        let corners = [
            pivot + rot_v(-hw, -hh), // NW
            pivot + rot_v(hw, -hh),  // NE
            pivot + rot_v(hw, hh),   // SE
            pivot + rot_v(-hw, hh),  // SW
        ];

        let handles = compute_transform_handles(
            pivot.x - hw,
            pivot.y - hh,
            local_size.x,
            local_size.y,
            rotation,
            stem_length,
        );

        Self {
            pivot,
            local_size,
            rotation,
            corners,
            handles,
        }
    }
}

/// Geometri kotak transformasi bentuk untuk perhitungan skala dan resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeTransformBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
}

impl ShapeTransformBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32, rotation: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            rotation,
        }
    }
}

/// Menghitung skala proporsional bentuk dari sudut berlawanan (A1: "scale").
/// Anchor adalah sudut yang berlawanan dan tetap diam di tempatnya.
/// Hasil dijamin memiliki ukuran minimal `min_dim` (misal 4.0 pt) dan tidak pernah menghasilkan NaN.
pub fn calculate_proportional_scale(
    corner: TransformHandleKind,
    cur_pos: Point2D,
    orig: ShapeTransformBox,
    min_dim: f32,
) -> (f32, f32, f32, f32) {
    let w0 = orig.width.abs().max(min_dim);
    let h0 = orig.height.abs().max(min_dim);
    let cx0 = orig.x + orig.width * 0.5;
    let cy0 = orig.y + orig.height * 0.5;

    let cos_r = orig.rotation.cos();
    let sin_r = orig.rotation.sin();

    let to_world = |lx: f32, ly: f32| -> Point2D {
        Point2D::new(cx0 + lx * cos_r - ly * sin_r, cy0 + lx * sin_r + ly * cos_r)
    };

    // Tentukan titik jangkar (sudut berlawanan yang tidak bergerak)
    let (anchor_world, sign_x, sign_y) = match corner {
        TransformHandleKind::SouthEast => (to_world(-w0 * 0.5, -h0 * 0.5), 1.0_f32, 1.0_f32),
        TransformHandleKind::NorthWest => (to_world(w0 * 0.5, h0 * 0.5), -1.0_f32, -1.0_f32),
        TransformHandleKind::NorthEast => (to_world(-w0 * 0.5, h0 * 0.5), 1.0_f32, -1.0_f32),
        TransformHandleKind::SouthWest => (to_world(w0 * 0.5, -h0 * 0.5), -1.0_f32, 1.0_f32),
        _ => (Point2D::new(orig.x, orig.y), 1.0, 1.0),
    };

    // Vektor dari anchor ke kursor saat ini dalam koordinat lokal bentuk
    let dx = cur_pos.x - anchor_world.x;
    let dy = cur_pos.y - anchor_world.y;
    let local_x = (dx * cos_r + dy * sin_r) * sign_x;
    let local_y = (-dx * sin_r + dy * cos_r) * sign_y;

    // Skala proporsional mempertahankan rasio aspek
    let scale_x = local_x / w0;
    let scale_y = local_y / h0;
    let mut scale = scale_x.max(scale_y);

    let min_scale = min_dim / w0.min(h0);
    if scale < min_scale {
        scale = min_scale;
    }

    let new_w = w0 * scale;
    let new_h = h0 * scale;

    // Hitung pusat baru dari titik jangkar
    let new_cx = anchor_world.x + (sign_x * new_w * 0.5) * cos_r - (sign_y * new_h * 0.5) * sin_r;
    let new_cy = anchor_world.y + (sign_x * new_w * 0.5) * sin_r + (sign_y * new_h * 0.5) * cos_r;

    (new_cx - new_w * 0.5, new_cy - new_h * 0.5, new_w, new_h)
}

/// Menghitung resize satu sumbu (A1: "resize" handle persegi biru di tengah sisi).
/// Sisi berlawanan tetap berada di posisinya.
pub fn calculate_single_axis_resize(
    edge: TransformHandleKind,
    cur_pos: Point2D,
    orig: ShapeTransformBox,
    min_dim: f32,
) -> (f32, f32, f32, f32) {
    let w0 = orig.width.abs().max(min_dim);
    let h0 = orig.height.abs().max(min_dim);
    let cx0 = orig.x + orig.width * 0.5;
    let cy0 = orig.y + orig.height * 0.5;

    let cos_r = orig.rotation.cos();
    let sin_r = orig.rotation.sin();

    // Proyeksikan pergeseran kursor ke sumbu lokal
    let dx = cur_pos.x - cx0;
    let dy = cur_pos.y - cy0;
    let local_x = dx * cos_r + dy * sin_r;
    let local_y = -dx * sin_r + dy * cos_r;

    let mut new_w = w0;
    let mut new_h = h0;
    let mut shift_lx = 0.0_f32;
    let mut shift_ly = 0.0_f32;

    match edge {
        TransformHandleKind::East => {
            let half_w = local_x.max(-w0 * 0.5 + min_dim);
            new_w = (half_w + w0 * 0.5).max(min_dim);
            shift_lx = (new_w - w0) * 0.5;
        }
        TransformHandleKind::West => {
            let half_w = (-local_x).max(-w0 * 0.5 + min_dim);
            new_w = (half_w + w0 * 0.5).max(min_dim);
            shift_lx = -(new_w - w0) * 0.5;
        }
        TransformHandleKind::South => {
            let half_h = local_y.max(-h0 * 0.5 + min_dim);
            new_h = (half_h + h0 * 0.5).max(min_dim);
            shift_ly = (new_h - h0) * 0.5;
        }
        TransformHandleKind::North => {
            let half_h = (-local_y).max(-h0 * 0.5 + min_dim);
            new_h = (half_h + h0 * 0.5).max(min_dim);
            shift_ly = -(new_h - h0) * 0.5;
        }
        _ => {}
    }

    let new_cx = cx0 + shift_lx * cos_r - shift_ly * sin_r;
    let new_cy = cy0 + shift_lx * sin_r + shift_ly * cos_r;

    (new_cx - new_w * 0.5, new_cy - new_h * 0.5, new_w, new_h)
}

/// Menghitung rotasi baru dari drag handle rotasi, dengan opsi snapping 15° (Shift).
pub fn calculate_rotation_angle(cx: f32, cy: f32, cur_pos: Point2D, snap_15_deg: bool) -> f32 {
    let dx = cur_pos.x - cx;
    let dy = cur_pos.y - cy;
    // Tangkai rotasi secara default berada di atas (vektor (0, -1), angle = -PI/2)
    let raw_angle = dy.atan2(dx) + std::f32::consts::FRAC_PI_2;
    let norm_angle = raw_angle.rem_euclid(std::f32::consts::TAU);

    if snap_15_deg {
        let step = std::f32::consts::TAU / 24.0; // 15 derajat
        (norm_angle / step).round() * step
    } else {
        norm_angle
    }
}

/// Mengubah jenis bentuk (Task 5.3) dengan mempertahankan posisi bounding box,
/// rotasi, gaya garis & isi, label teks, dan relasi connector.
pub fn change_shape_kind(shape: &mut DrawObject, new_kind: ShapeKind, default_corner_radius: f32) {
    if let DrawObject::Shape {
        shape_type,
        corner_radius,
        width,
        height,
        ..
    } = shape
    {
        *shape_type = new_kind;
        if new_kind == ShapeKind::Rectangle && default_corner_radius > 0.0 {
            *corner_radius = default_corner_radius;
        } else if new_kind == ShapeKind::Rectangle && *corner_radius > 0.0 {
            // Biarkan corner radius yang ada jika sudah diset
        } else {
            *corner_radius = 0.0;
        }
        // Minimal dimensi terjaga
        if *width < 4.0 {
            *width = 4.0;
        }
        if *height < 4.0 {
            *height = 4.0;
        }
    }
}

/// Menghitung area aman di dalam bentuk (safe inset) untuk tata letak teks label (Task 6.2).
/// Memperhitungkan jenis bentuk (elips, belah ketupat, segitiga, rounded rect).
pub fn calculate_shape_text_bounds(
    shape_kind: ShapeKind,
    width: f32,
    height: f32,
    corner_radius: f32,
    stroke_width: f32,
) -> (f32, f32) {
    let abs_w = width.abs();
    let abs_h = height.abs();
    let pad = stroke_width * 0.5 + 4.0;

    match shape_kind {
        ShapeKind::Rectangle => {
            if corner_radius > 0.0 {
                let r_cut = corner_radius.min(abs_w.min(abs_h) * 0.5) * 0.2929;
                let sw = (abs_w - 2.0 * (pad + r_cut)).max(10.0);
                let sh = (abs_h - 2.0 * (pad + r_cut)).max(10.0);
                (sw, sh)
            } else {
                let sw = (abs_w - 2.0 * pad).max(10.0);
                let sh = (abs_h - 2.0 * pad).max(10.0);
                (sw, sh)
            }
        }
        ShapeKind::Ellipse => {
            let sw = (abs_w * std::f32::consts::FRAC_1_SQRT_2 - 2.0 * pad).max(10.0);
            let sh = (abs_h * std::f32::consts::FRAC_1_SQRT_2 - 2.0 * pad).max(10.0);
            (sw, sh)
        }
        ShapeKind::Diamond => {
            let sw = (abs_w * 0.5 - 2.0 * pad).max(10.0);
            let sh = (abs_h * 0.5 - 2.0 * pad).max(10.0);
            (sw, sh)
        }
        ShapeKind::Triangle | ShapeKind::RightTriangle | ShapeKind::ScaleneTriangle { .. } => {
            let sw = (abs_w * 0.55 - 2.0 * pad).max(10.0);
            let sh = (abs_h * 0.45 - 2.0 * pad).max(10.0);
            (sw, sh)
        }
        ShapeKind::Parallelogram => {
            let sw = (abs_w * 0.65 - 2.0 * pad).max(10.0);
            let sh = (abs_h * 0.85 - 2.0 * pad).max(10.0);
            (sw, sh)
        }
        ShapeKind::Line | ShapeKind::Curve => (abs_w.max(60.0), abs_h.clamp(20.0, 40.0)),
    }
}

/// Menghitung ukuran font otomatis (Auto-Fit) agar teks tetap berada di dalam bentuk (Task 6.2).
/// Font diperkecil secara proporsional sampai batas minimum `min_font_size` (misal 8.0 pt).
pub fn calculate_auto_fit_font_size(
    base_font_size: f32,
    text: &str,
    safe_width: f32,
    safe_height: f32,
    min_font_size: f32,
) -> f32 {
    let clean = text.trim();
    if clean.is_empty() {
        return base_font_size;
    }
    let lines: Vec<&str> = clean.lines().collect();
    let num_lines = lines.len().max(1) as f32;
    let max_line_chars = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;

    let font_from_w = safe_width / (max_line_chars * 0.55);
    let font_from_h = safe_height / (num_lines * 1.35);

    let ideal_font = font_from_w.min(font_from_h);
    ideal_font.min(base_font_size).max(min_font_size)
}

/// Menghitung posisi dan dimensi bentuk baru serta titik sambung berlawanan untuk Quick Diagramming (Task 9.2, 9.5).
///
/// Menghitung kotak baru di arah `site` dengan jarak `gap`, memeriksa apakah tumpang tindih dengan
/// bentuk lain di `all_objects`, menggeser jika diperlukan (hingga `max_attempts`),
/// dan memastikan tidak keluar dari batas halaman kertas (`page_width`, `page_height`).
pub fn next_shape_rect(
    source_rect: (f32, f32, f32, f32),
    site: ConnectionSite,
    gap: f32,
    all_objects: &[DrawObject],
    page_size: (f32, f32),
) -> (f32, f32, f32, f32, ConnectionSite) {
    let (source_x, source_y, width, height) = source_rect;
    let (page_width, page_height) = page_size;
    let w = width.abs();
    let h = height.abs();
    let margin = 16.0_f32;
    let actual_gap = gap.max(20.0);

    let (opposite_site, dir_vec) = match site {
        ConnectionSite::Top => (ConnectionSite::Bottom, Point2D::new(0.0, -1.0)),
        ConnectionSite::Right | ConnectionSite::Auto => {
            (ConnectionSite::Left, Point2D::new(1.0, 0.0))
        }
        ConnectionSite::Bottom => (ConnectionSite::Top, Point2D::new(0.0, 1.0)),
        ConnectionSite::Left => (ConnectionSite::Right, Point2D::new(-1.0, 0.0)),
    };

    let step_x = if dir_vec.x > 0.0 {
        w + actual_gap
    } else if dir_vec.x < 0.0 {
        -(w + actual_gap)
    } else {
        0.0
    };

    let step_y = if dir_vec.y > 0.0 {
        h + actual_gap
    } else if dir_vec.y < 0.0 {
        -(h + actual_gap)
    } else {
        0.0
    };

    let overlaps = |cx: f32, cy: f32| -> bool {
        for obj in all_objects {
            if let DrawObject::Shape {
                x: ox,
                y: oy,
                width: ow,
                height: oh,
                ..
            } = obj
            {
                let o_min_x = if *ow >= 0.0 { *ox } else { *ox + *ow };
                let o_min_y = if *oh >= 0.0 { *oy } else { *oy + *oh };
                let o_w = ow.abs();
                let o_h = oh.abs();

                let has_overlap = cx < o_min_x + o_w + 4.0
                    && cx + w + 4.0 > o_min_x
                    && cy < o_min_y + o_h + 4.0
                    && cy + h + 4.0 > o_min_y;

                if has_overlap {
                    return true;
                }
            }
        }
        false
    };

    let mut chosen_x = source_x + step_x;
    let mut chosen_y = source_y + step_y;
    let mut found = false;

    for attempt in 0..5 {
        let test_x = source_x + step_x + (attempt as f32) * step_x * 0.5;
        let test_y = source_y + step_y + (attempt as f32) * step_y * 0.5;

        let in_bounds = test_x >= margin
            && test_x + w <= page_width - margin
            && test_y >= margin
            && test_y + h <= page_height - margin;

        if in_bounds && !overlaps(test_x, test_y) {
            chosen_x = test_x;
            chosen_y = test_y;
            found = true;
            break;
        }
    }

    if !found {
        chosen_x = chosen_x.clamp(margin, (page_width - w - margin).max(margin));
        chosen_y = chosen_y.clamp(margin, (page_height - h - margin).max(margin));
    }

    (chosen_x, chosen_y, w, h, opposite_site)
}
