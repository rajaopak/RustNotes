use image::{ImageBuffer, Rgba};
use std::fs::File;
use std::io::Write;

fn main() {
    let size = 256u32;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(size, size);

    // Render a stylish notebook icon:
    // Navy blue rounded cover with spine, white center label, and gold bookmark
    let bg_color = Rgba([40, 72, 114, 255]); // #284872 Navy
    let spine_color = Rgba([30, 55, 90, 255]); // Darker spine
    let label_bg = Rgba([245, 245, 248, 255]); // White/off-white label
    let gold_star = Rgba([255, 204, 0, 255]); // Gold accent

    let left = 32.0;
    let right = 224.0;
    let top = 24.0;
    let bottom = 232.0;
    let radius = 24.0;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32;
            let py = y as f32;

            // Check if inside rounded notebook rect
            if px >= left && px <= right && py >= top && py <= bottom {
                // Rounded corners on right edge
                let mut in_rect = true;
                if px > right - radius && py < top + radius {
                    let d =
                        ((px - (right - radius)).powi(2) + (py - (top + radius)).powi(2)).sqrt();
                    if d > radius {
                        in_rect = false;
                    }
                }
                if px > right - radius && py > bottom - radius {
                    let d =
                        ((px - (right - radius)).powi(2) + (py - (bottom - radius)).powi(2)).sqrt();
                    if d > radius {
                        in_rect = false;
                    }
                }

                if in_rect {
                    // Spine strip on the left (width: 24px)
                    if px <= left + 24.0 {
                        img.put_pixel(x, y, spine_color);
                    } else if (px - (left + 24.0)).abs() < 1.5 {
                        img.put_pixel(x, y, Rgba([20, 40, 70, 255])); // shadow line
                    } else if (80.0..=185.0).contains(&px) && (90.0..=150.0).contains(&py) {
                        // Label badge in center
                        img.put_pixel(x, y, label_bg);
                    } else if (165.0..=185.0).contains(&px) && (top..=top + 42.0).contains(&py) {
                        // Gold bookmark ribbon hanging from top
                        img.put_pixel(x, y, gold_star);
                    } else {
                        img.put_pixel(x, y, bg_color);
                    }
                }
            }
        }
    }

    // Save PNG
    let png_path = "assets/icon.png";
    img.save(png_path).expect("Failed to save icon.png");
    println!("Saved {}", png_path);

    // Also build a valid Windows .ico file containing PNG data
    let mut png_bytes = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut png_bytes);
        img.write_to(
            &mut std::io::BufWriter::new(cursor),
            image::ImageFormat::Png,
        )
        .expect("Failed to encode PNG for ICO");
    }

    let mut ico_file = File::create("assets/icon.ico").expect("create icon.ico");
    // ICO Header
    ico_file.write_all(&[0, 0]).unwrap(); // Reserved
    ico_file.write_all(&[1, 0]).unwrap(); // Type 1 = Icon
    ico_file.write_all(&[1, 0]).unwrap(); // 1 Image

    // Icon Entry (16 bytes)
    ico_file.write_all(&[0]).unwrap(); // Width 0 = 256
    ico_file.write_all(&[0]).unwrap(); // Height 0 = 256
    ico_file.write_all(&[0]).unwrap(); // Color count
    ico_file.write_all(&[0]).unwrap(); // Reserved
    ico_file.write_all(&[1, 0]).unwrap(); // Color planes
    ico_file.write_all(&[32, 0]).unwrap(); // Bit per pixel
    ico_file
        .write_all(&(png_bytes.len() as u32).to_le_bytes())
        .unwrap(); // Size of image data
    ico_file.write_all(&(22u32).to_le_bytes()).unwrap(); // Offset of image data (6 header + 16 entry = 22)

    // Image payload (PNG bytes)
    ico_file.write_all(&png_bytes).unwrap();
    println!("Saved assets/icon.ico ({} bytes)", png_bytes.len() + 22);
}
