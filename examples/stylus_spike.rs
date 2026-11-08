use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RustNotes - Stylus & Pointer Spike")
            .with_inner_size([900.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Stylus Spike",
        native_options,
        Box::new(|_cc| Ok(Box::new(StylusSpikeApp::default()))),
    )
}

#[derive(Default)]
struct StylusSpikeApp {
    last_log: Vec<String>,
}

impl eframe::App for StylusSpikeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Stylus & Pointer Diagnostics (winit / egui 0.31)");
            ui.label("Draw or hover in the box below with mouse, pen/stylus, or touch.");

            let (rect, resp) = ui.allocate_exact_size(egui::Vec2::new(850.0, 300.0), egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(25, 25, 25));

            ctx.input(|i| {
                for event in &i.raw.events {
                    match event {
                        egui::Event::PointerMoved(pos) => {
                            if rect.contains(*pos) {
                                self.add_log(format!("PointerMoved: pos=({:.1}, {:.1})", pos.x, pos.y));
                            }
                        }
                        egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                            self.add_log(format!("PointerButton: pos=({:.1}, {:.1}), button={:?}, pressed={}, mod={:?}", pos.x, pos.y, button, pressed, modifiers));
                        }
                        egui::Event::Touch { device_id, id, phase, pos, force } => {
                            self.add_log(format!("Touch: dev={:?}, id={:?}, phase={:?}, pos=({:.1}, {:.1}), force={:?}", device_id, id, phase, pos.x, pos.y, force));
                        }
                        egui::Event::MouseMoved(v) => {
                            self.add_log(format!("MouseMoved: delta=({:.1}, {:.1})", v.x, v.y));
                        }
                        other => {
                            let s = format!("{:?}", other);
                            if !s.starts_with("Text(") && !s.starts_with("Key(") {
                                self.add_log(format!("Event: {}", s));
                            }
                        }
                    }
                }
            });

            if resp.hovered() || resp.dragged() {
                ui.ctx().request_repaint();
            }

            ui.add_space(10.0);
            ui.label(egui::RichText::new("Event Log:").strong());
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for line in self.last_log.iter().rev() {
                    ui.label(line);
                }
            });
        });
    }
}

impl StylusSpikeApp {
    fn add_log(&mut self, s: String) {
        if self.last_log.len() > 60 {
            self.last_log.remove(0);
        }
        self.last_log.push(s);
    }
}
