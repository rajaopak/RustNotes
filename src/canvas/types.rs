use crate::model::ShapeKind;
use eframe::egui::Pos2;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActiveTool {
    Pen,
    Pencil,
    Highlighter,
    Eraser,
    Lasso,
    Shape(ShapeKind),
    Text,
    StickyNote,
    Laser,
    Hand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EraserMode {
    Partial,
    WholeStroke,
}

#[derive(Clone, Debug)]
pub struct LaserTrailPoint {
    pub pos: Pos2,
    pub time: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResizeCorner {
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
    Rotation,
    LineStart,
    LineEnd,
}
