use crate::model::StrokePoint;
use eframe::egui::Pos2;

/// Fase 8.5: Palm Rejection — menyaring sentuhan telapak tangan saat menulis dengan pena.
/// Menolak input jika terdeteksi multi-touch aktif saat goresan berlangsung atau
/// jika terjadi lompatan koordinat mendadak yang tidak wajar (> 180 pt dalam < 80 ms)
/// akibat telapak tangan menyentuh sisi lain kanvas.
pub fn should_reject_palm_input(
    existing_points: &[StrokePoint],
    candidate: Pos2,
    dt_secs: f32,
    multi_touch_active: bool,
) -> bool {
    if multi_touch_active {
        return true;
    }
    if let Some(last) = existing_points.last() {
        let dist = (candidate - last.to_pos2()).length();
        if dist > 180.0 && dt_secs < 0.08 {
            return true;
        }
    }
    false
}
