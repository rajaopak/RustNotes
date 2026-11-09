use crate::model::{KeyBinding, ShortcutAction, ShortcutSettings};
use eframe::egui::{self, Key};

/// Konversi nama string tombol ke `egui::Key`.
pub fn name_to_egui_key(name: &str) -> Option<Key> {
    match name.to_uppercase().as_str() {
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "0" | "NUM0" => Some(Key::Num0),
        "1" | "NUM1" => Some(Key::Num1),
        "2" | "NUM2" => Some(Key::Num2),
        "3" | "NUM3" => Some(Key::Num3),
        "4" | "NUM4" => Some(Key::Num4),
        "5" | "NUM5" => Some(Key::Num5),
        "6" | "NUM6" => Some(Key::Num6),
        "7" | "NUM7" => Some(Key::Num7),
        "8" | "NUM8" => Some(Key::Num8),
        "9" | "NUM9" => Some(Key::Num9),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "DELETE" | "DEL" => Some(Key::Delete),
        "BACKSPACE" => Some(Key::Backspace),
        "SPACE" => Some(Key::Space),
        "ENTER" => Some(Key::Enter),
        "TAB" => Some(Key::Tab),
        "DOWN" | "ARROWDOWN" => Some(Key::ArrowDown),
        "LEFT" | "ARROWLEFT" => Some(Key::ArrowLeft),
        "RIGHT" | "ARROWRIGHT" => Some(Key::ArrowRight),
        "UP" | "ARROWUP" => Some(Key::ArrowUp),
        "PAGEUP" => Some(Key::PageUp),
        "PAGEDOWN" => Some(Key::PageDown),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "INSERT" => Some(Key::Insert),
        _ => None,
    }
}

/// Konversi `egui::Key` ke representasi nama string yang seragam.
pub fn egui_key_to_name(key: Key) -> Option<&'static str> {
    match key {
        Key::A => Some("A"),
        Key::B => Some("B"),
        Key::C => Some("C"),
        Key::D => Some("D"),
        Key::E => Some("E"),
        Key::F => Some("F"),
        Key::G => Some("G"),
        Key::H => Some("H"),
        Key::I => Some("I"),
        Key::J => Some("J"),
        Key::K => Some("K"),
        Key::L => Some("L"),
        Key::M => Some("M"),
        Key::N => Some("N"),
        Key::O => Some("O"),
        Key::P => Some("P"),
        Key::Q => Some("Q"),
        Key::R => Some("R"),
        Key::S => Some("S"),
        Key::T => Some("T"),
        Key::U => Some("U"),
        Key::V => Some("V"),
        Key::W => Some("W"),
        Key::X => Some("X"),
        Key::Y => Some("Y"),
        Key::Z => Some("Z"),
        Key::Num0 => Some("0"),
        Key::Num1 => Some("1"),
        Key::Num2 => Some("2"),
        Key::Num3 => Some("3"),
        Key::Num4 => Some("4"),
        Key::Num5 => Some("5"),
        Key::Num6 => Some("6"),
        Key::Num7 => Some("7"),
        Key::Num8 => Some("8"),
        Key::Num9 => Some("9"),
        Key::F1 => Some("F1"),
        Key::F2 => Some("F2"),
        Key::F3 => Some("F3"),
        Key::F4 => Some("F4"),
        Key::F5 => Some("F5"),
        Key::F6 => Some("F6"),
        Key::F7 => Some("F7"),
        Key::F8 => Some("F8"),
        Key::F9 => Some("F9"),
        Key::F10 => Some("F10"),
        Key::F11 => Some("F11"),
        Key::F12 => Some("F12"),
        Key::Delete => Some("Delete"),
        Key::Backspace => Some("Backspace"),
        Key::Space => Some("Space"),
        Key::Enter => Some("Enter"),
        Key::Tab => Some("Tab"),
        Key::ArrowDown => Some("Down"),
        Key::ArrowLeft => Some("Left"),
        Key::ArrowRight => Some("Right"),
        Key::ArrowUp => Some("Up"),
        Key::PageUp => Some("PageUp"),
        Key::PageDown => Some("PageDown"),
        Key::Home => Some("Home"),
        Key::End => Some("End"),
        Key::Insert => Some("Insert"),
        _ => None,
    }
}

/// Memeriksa apakah suatu aksi dipicu berdasarkan konfigurasi shortcut aktif dan input frame egui.
pub fn is_action_triggered(
    action: ShortcutAction,
    settings: &ShortcutSettings,
    input: &egui::InputState,
) -> bool {
    let binding = settings.get(action);
    let Some(target_key) = name_to_egui_key(&binding.key) else {
        return false;
    };

    if !input.key_pressed(target_key) {
        return false;
    }

    let is_ctrl = input.modifiers.command || input.modifiers.ctrl;
    let is_alt = input.modifiers.alt;
    let is_shift = input.modifiers.shift;

    binding.ctrl == is_ctrl && binding.alt == is_alt && binding.shift == is_shift
}

/// Mendeteksi penekanan tombol baru saat pengguna dalam mode rebinding keyboard shortcut.
pub fn capture_rebinding_key(input: &egui::InputState) -> Option<KeyBinding> {
    for event in &input.raw.events {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            // Abaikan tombol escape (digunakan untuk pembatalan)
            if *key == Key::Escape {
                continue;
            }

            if let Some(name) = egui_key_to_name(*key) {
                let is_ctrl = modifiers.command || modifiers.ctrl;
                return Some(KeyBinding::new(
                    name,
                    is_ctrl,
                    modifiers.alt,
                    modifiers.shift,
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_and_egui_key_bidirectional() {
        assert_eq!(name_to_egui_key("P"), Some(Key::P));
        assert_eq!(name_to_egui_key("delete"), Some(Key::Delete));
        assert_eq!(name_to_egui_key("F6"), Some(Key::F6));
        assert_eq!(name_to_egui_key("0"), Some(Key::Num0));
        assert_eq!(name_to_egui_key("unknown_key_xyz"), None);

        assert_eq!(egui_key_to_name(Key::P), Some("P"));
        assert_eq!(egui_key_to_name(Key::Delete), Some("Delete"));
        assert_eq!(egui_key_to_name(Key::F6), Some("F6"));
        assert_eq!(egui_key_to_name(Key::Num0), Some("0"));
    }
}
