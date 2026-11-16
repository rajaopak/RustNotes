use crate::geometry::should_reject_palm_input;
use crate::model::StrokePoint;
use eframe::egui::{self, PointerButton, Pos2};

/// Sumber perangkat input pointer yang menghasilkan sampel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerSource {
    Mouse,
    Touch {
        device_id: egui::TouchDeviceId,
        id: egui::TouchId,
    },
}

/// Fase siklus hidup sampel pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerSamplePhase {
    Press,
    Move,
    Release,
    Cancel,
}

/// Sampel input pointer mentah (raw input sample) yang menyimpan koordinat layar,
/// koordinat halaman, tekanan perangkat, sumber input, urutan, dan selang waktu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawPointerSample {
    pub screen_pos: Pos2,
    pub page_pos: Pos2,
    pub raw_pressure: Option<f32>,
    pub effective_pressure: f32,
    pub phase: PointerSamplePhase,
    pub source: PointerSource,
    pub sequence: u64,
    pub dt_secs: f32,
}

impl RawPointerSample {
    #[inline]
    pub fn to_stroke_point(&self) -> StrokePoint {
        StrokePoint {
            x: self.page_pos.x,
            y: self.page_pos.y,
            pressure: self.effective_pressure,
        }
    }
}

/// Representasi event pointer dalam koordinat layar yang diekstrak secara berurutan dari satu frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawScreenEvent {
    pub screen_pos: Pos2,
    pub raw_pressure: Option<f32>,
    pub phase: PointerSamplePhase,
    pub source: PointerSource,
}

/// Kumpulan seluruh event pointer berurutan dalam satu frame beserta status sentuhan/pembatalan.
#[derive(Clone, Debug, Default)]
pub struct FramePointerBatch {
    pub active_touch_id: Option<egui::TouchId>,
    pub events: Vec<RawScreenEvent>,
    pub cancelled: bool,
}

/// Mengambil TouchId dan gaya tekanan (pressure/force) dari daftar event egui.
/// Jika `active_id` sudah ada, prioritaskan sentuhan dengan ID tersebut.
/// Jika belum ada, cari sentuhan yang paling dekat dengan `reference_pos` (atau sentuhan pertama).
pub fn extract_touch_pressure(
    events: &[egui::Event],
    active_id: Option<egui::TouchId>,
    reference_pos: Option<Pos2>,
) -> (Option<egui::TouchId>, Option<f32>) {
    if let Some(target_id) = active_id {
        for event in events.iter().rev() {
            if let egui::Event::Touch {
                id, phase, force, ..
            } = event
                && *id == target_id
                && *phase != egui::TouchPhase::Cancel
            {
                return (Some(target_id), *force);
            }
        }
        return (Some(target_id), None);
    }

    let mut best_candidate: Option<(egui::TouchId, Option<f32>, f32)> = None;
    for event in events.iter().rev() {
        if let egui::Event::Touch {
            id,
            phase,
            pos,
            force,
            ..
        } = event
            && *phase != egui::TouchPhase::Cancel
        {
            let dist = reference_pos.map(|p| (*pos - p).length()).unwrap_or(0.0);
            if let Some((_, _, best_dist)) = best_candidate {
                if dist < best_dist {
                    best_candidate = Some((*id, *force, dist));
                }
            } else {
                best_candidate = Some((*id, *force, dist));
            }
        }
    }

    if let Some((id, force, _)) = best_candidate {
        (Some(id), force)
    } else {
        (active_id, None)
    }
}

/// Mengekstrak seluruh event pergerakan/tekanan pointer dalam satu frame secara berurutan (kronologis)
/// dari `i.raw.events`. Memastikan posisi dan tekanan (pressure) tetap berpasangan secara konsisten,
/// serta mendeteksi pembatalan input (`PointerGone` atau `TouchPhase::Cancel`).
pub fn collect_frame_pointer_batch(
    raw_events: &[egui::Event],
    active_touch_id: Option<egui::TouchId>,
    reference_pos: Option<Pos2>,
    fallback_pos: Option<Pos2>,
) -> FramePointerBatch {
    let (mut resolved_touch_id, initial_force) =
        extract_touch_pressure(raw_events, active_touch_id, reference_pos.or(fallback_pos));

    let mut current_force: Option<f32> = initial_force;
    let mut current_source = PointerSource::Mouse;
    for ev in raw_events {
        if let egui::Event::Touch {
            device_id,
            id,
            phase,
            force,
            ..
        } = ev
            && Some(*id) == resolved_touch_id
            && *phase != egui::TouchPhase::Cancel
        {
            current_source = PointerSource::Touch {
                device_id: *device_id,
                id: *id,
            };
            if force.is_some() {
                current_force = *force;
            }
            break;
        }
    }

    let mut batch = FramePointerBatch {
        active_touch_id: resolved_touch_id,
        events: Vec::with_capacity(raw_events.len().max(1)),
        cancelled: false,
    };

    for ev in raw_events {
        match ev {
            egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                force,
            } => {
                if resolved_touch_id.is_none() && *phase != egui::TouchPhase::Cancel {
                    resolved_touch_id = Some(*id);
                    batch.active_touch_id = resolved_touch_id;
                }
                if resolved_touch_id == Some(*id) || resolved_touch_id.is_none() {
                    let src = PointerSource::Touch {
                        device_id: *device_id,
                        id: *id,
                    };
                    current_source = src;
                    current_force = *force;

                    let sample_phase = match phase {
                        egui::TouchPhase::Start => PointerSamplePhase::Press,
                        egui::TouchPhase::Move => PointerSamplePhase::Move,
                        egui::TouchPhase::End => PointerSamplePhase::Release,
                        egui::TouchPhase::Cancel => {
                            batch.cancelled = true;
                            PointerSamplePhase::Cancel
                        }
                    };
                    push_or_merge_screen_event(
                        &mut batch.events,
                        RawScreenEvent {
                            screen_pos: *pos,
                            raw_pressure: *force,
                            phase: sample_phase,
                            source: src,
                        },
                    );
                }
            }
            egui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                ..
            } => {
                let phase = if *pressed {
                    PointerSamplePhase::Press
                } else {
                    PointerSamplePhase::Release
                };
                // Cari apakah ada event Touch dengan ID aktif di sekitar titik tombol ini
                let matched_touch = raw_events.iter().rev().find_map(|e| {
                    if let egui::Event::Touch {
                        device_id,
                        id,
                        phase: t_phase,
                        pos: t_pos,
                        force,
                    } = e
                        && Some(*id) == resolved_touch_id
                        && *t_phase != egui::TouchPhase::Cancel
                        && (*t_pos - *pos).length_sq() <= 4.0
                    {
                        Some((
                            PointerSource::Touch {
                                device_id: *device_id,
                                id: *id,
                            },
                            *force,
                        ))
                    } else {
                        None
                    }
                });
                let (src, force) = matched_touch.unwrap_or((current_source, current_force));
                push_or_merge_screen_event(
                    &mut batch.events,
                    RawScreenEvent {
                        screen_pos: *pos,
                        raw_pressure: force,
                        phase,
                        source: src,
                    },
                );
            }
            egui::Event::PointerMoved(pos) => {
                // Jika ada event Touch untuk ID aktif pada koordinat yang sama di frame ini, gunakan datanya
                let matched_touch = raw_events.iter().rev().find_map(|e| {
                    if let egui::Event::Touch {
                        device_id,
                        id,
                        phase: t_phase,
                        pos: t_pos,
                        force,
                    } = e
                        && Some(*id) == resolved_touch_id
                        && *t_phase != egui::TouchPhase::Cancel
                        && (*t_pos - *pos).length_sq() <= 1e-4
                    {
                        Some((
                            PointerSource::Touch {
                                device_id: *device_id,
                                id: *id,
                            },
                            *force,
                        ))
                    } else {
                        None
                    }
                });
                let (src, force) = matched_touch.unwrap_or((current_source, current_force));
                push_or_merge_screen_event(
                    &mut batch.events,
                    RawScreenEvent {
                        screen_pos: *pos,
                        raw_pressure: force,
                        phase: PointerSamplePhase::Move,
                        source: src,
                    },
                );
            }
            egui::Event::PointerGone => {
                batch.cancelled = true;
                let last_pos = batch
                    .events
                    .last()
                    .map(|e| e.screen_pos)
                    .or(fallback_pos)
                    .or(reference_pos)
                    .unwrap_or(Pos2::ZERO);
                batch.events.push(RawScreenEvent {
                    screen_pos: last_pos,
                    raw_pressure: None,
                    phase: PointerSamplePhase::Cancel,
                    source: current_source,
                });
            }
            _ => {}
        }
    }

    if !batch.cancelled
        && batch.events.is_empty()
        && let Some(pos) = fallback_pos
    {
        batch.events.push(RawScreenEvent {
            screen_pos: pos,
            raw_pressure: current_force,
            phase: PointerSamplePhase::Move,
            source: current_source,
        });
    }

    batch
}

/// Menambahkan `RawScreenEvent` ke daftar frame dengan menggabungkan event identik berurutan
/// (misalnya `Touch` dan `PointerMoved` pada koordinat yang tepat sama) tanpa menghilangkan
/// pergerakan kecil atau perubahan tekanan.
fn push_or_merge_screen_event(events: &mut Vec<RawScreenEvent>, candidate: RawScreenEvent) {
    if let Some(last) = events.last_mut() {
        let same_pos = (last.screen_pos - candidate.screen_pos).length_sq() <= 1e-6;
        if same_pos {
            let same_pressure = match (last.raw_pressure, candidate.raw_pressure) {
                (Some(a), Some(b)) => (a - b).abs() <= 1e-4,
                (None, None) => true,
                _ => false,
            };

            // Gabungkan jika tekanan sama atau salah satu adalah event pendamping (Touch + PointerMoved/Button)
            if same_pressure || last.raw_pressure.is_none() || candidate.raw_pressure.is_none() {
                if candidate.raw_pressure.is_some() {
                    last.raw_pressure = candidate.raw_pressure;
                }
                if matches!(candidate.source, PointerSource::Touch { .. }) {
                    last.source = candidate.source;
                }
                if matches!(
                    candidate.phase,
                    PointerSamplePhase::Press
                        | PointerSamplePhase::Release
                        | PointerSamplePhase::Cancel
                ) {
                    last.phase = candidate.phase;
                }
                return;
            }
        }
    }
    events.push(candidate);
}

/// Menambahkan sampel pointer ke buffer raw samples dan `draft_stroke` dengan perlindungan
/// Palm Rejection, tanpa membuang pergerakan pendek/kecil yang valid.
/// Mengembalikan `true` jika sampel diterima dan memperbarui goresan.
pub fn append_stroke_sample(
    raw_samples: &mut Vec<RawPointerSample>,
    draft_stroke: &mut Vec<StrokePoint>,
    sample: RawPointerSample,
    palm_rejection_enabled: bool,
    multi_touch_active: bool,
) -> bool {
    if palm_rejection_enabled
        && should_reject_palm_input(
            draft_stroke,
            sample.page_pos,
            sample.dt_secs,
            multi_touch_active,
        )
    {
        return false;
    }

    // Hindari duplikasi sampel yang benar-benar identik (posisi dan tekanan sama persis)
    if let Some(prev_raw) = raw_samples.last_mut() {
        let same_screen = (prev_raw.screen_pos - sample.screen_pos).length_sq() <= 1e-6;
        let same_page = (prev_raw.page_pos - sample.page_pos).length_sq() <= 1e-6;
        let same_pressure = (prev_raw.effective_pressure - sample.effective_pressure).abs() <= 1e-4;
        if same_screen && same_page && same_pressure {
            if matches!(
                sample.phase,
                PointerSamplePhase::Release | PointerSamplePhase::Cancel
            ) {
                prev_raw.phase = sample.phase;
            }
            return false;
        }
    }
    raw_samples.push(sample);

    if let Some(last_pt) = draft_stroke.last_mut() {
        let dist_sq = (sample.page_pos - last_pt.to_pos2()).length_sq();
        if dist_sq > 1e-6 {
            draft_stroke.push(sample.to_stroke_point());
            true
        } else if (sample.effective_pressure - last_pt.pressure).abs() > 1e-4 {
            last_pt.pressure = sample.effective_pressure;
            true
        } else {
            false
        }
    } else {
        draft_stroke.push(sample.to_stroke_point());
        true
    }
}

/// Menambahkan sampel koordinat ke jalur Lasso (`lasso_points`) dan `raw_samples`
/// dengan mempertahankan seluruh detail gerakan dan arah tanpa duplikasi titik berhimpit.
pub fn append_lasso_sample(
    raw_samples: &mut Vec<RawPointerSample>,
    lasso_points: &mut Vec<Pos2>,
    sample: RawPointerSample,
) -> bool {
    if let Some(prev_raw) = raw_samples.last_mut() {
        let same_page = (prev_raw.page_pos - sample.page_pos).length_sq() <= 1e-6;
        if same_page {
            if matches!(
                sample.phase,
                PointerSamplePhase::Release | PointerSamplePhase::Cancel
            ) {
                prev_raw.phase = sample.phase;
            }
            return false;
        }
    }
    raw_samples.push(sample);

    if let Some(&last_p) = lasso_points.last()
        && (sample.page_pos - last_p).length_sq() <= 1e-6
    {
        return false;
    }
    lasso_points.push(sample.page_pos);
    true
}
