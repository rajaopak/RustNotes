use eframe::egui::{Color32, Pos2};
use rustnotes::geometry::{SpatialGrid, hit_test_object, smooth_stroke_points, tessellate_stroke};
use rustnotes::model::{ColorData, DrawObject, StrokePoint, StrokeStyle};
use std::time::Instant;

fn generate_realistic_stroke(
    id: usize,
    start_x: f32,
    start_y: f32,
    num_points: usize,
) -> DrawObject {
    let mut points = Vec::with_capacity(num_points);
    let mut x = start_x;
    let mut y = start_y;

    for i in 0..num_points {
        let angle = (i as f32 * 0.25).sin() * 1.5;
        let step = 3.0 + ((i % 5) as f32 * 0.5);
        x += angle.cos() * step;
        y += angle.sin() * step + 1.2;
        let pressure = 0.3 + (i as f32 / num_points as f32) * 0.7;
        points.push(StrokePoint { x, y, pressure });
    }

    DrawObject::Stroke {
        id: format!("stroke_{id}"),
        points,
        color: ColorData {
            r: (id % 255) as u8,
            g: ((id * 3) % 255) as u8,
            b: ((id * 7) % 255) as u8,
            a: 255,
        },
        width: 3.0,
        is_highlighter: id.is_multiple_of(5),
        style: StrokeStyle::Solid,
    }
}

fn run_benchmark(stroke_count: usize, pts_per_stroke: usize) {
    println!("\n=======================================================");
    println!("Benchmark: {stroke_count} strokes ({pts_per_stroke} points each)");
    println!("=======================================================");

    // 1. Generation
    let gen_start = Instant::now();
    let mut strokes = Vec::with_capacity(stroke_count);
    for i in 0..stroke_count {
        let x = (i % 25) as f32 * 30.0 + 50.0;
        let y = (i / 25) as f32 * 20.0 + 50.0;
        strokes.push(generate_realistic_stroke(i, x, y, pts_per_stroke));
    }
    let gen_duration = gen_start.elapsed();
    println!("Generated {stroke_count} strokes in: {:.2?}", gen_duration);

    // 2. Smoothing & Tessellation
    let tess_start = Instant::now();
    let mut total_vertices = 0;
    let mut total_triangles = 0;
    for stroke in &strokes {
        if let DrawObject::Stroke { points, width, .. } = stroke {
            let smoothed = smooth_stroke_points(points, 3);
            let mesh = tessellate_stroke(&smoothed, *width, Color32::BLACK, |p| p);
            total_vertices += mesh.vertices.len();
            total_triangles += mesh.indices.len() / 3;
        }
    }
    let tess_duration = tess_start.elapsed();
    let avg_tess_per_stroke = tess_duration.as_micros() as f64 / stroke_count as f64;
    println!(
        "Tessellation & Smoothing Total: {:.2?} (Avg: {:.2} µs/stroke)",
        tess_duration, avg_tess_per_stroke
    );
    println!(
        "Total Vertices: {}, Triangles: {}",
        total_vertices, total_triangles
    );

    // 3. SpatialGrid Rebuild
    let grid_start = Instant::now();
    let mut grid = SpatialGrid::new(100.0);
    grid.rebuild(&strokes);
    let grid_build_duration = grid_start.elapsed();
    println!("SpatialGrid Rebuild Time: {:.2?}", grid_build_duration);

    // 4. Hit Testing Comparison: Brute-Force O(N) vs SpatialGrid O(k)
    let test_points: Vec<Pos2> = (0..500)
        .map(|i| Pos2::new((i * 17 % 800) as f32, (i * 31 % 1100) as f32))
        .collect();

    // Brute force:
    let bf_start = Instant::now();
    let mut bf_hits = 0;
    for &p in &test_points {
        for obj in &strokes {
            if hit_test_object(obj, p, 16.0) {
                bf_hits += 1;
            }
        }
    }
    let bf_duration = bf_start.elapsed();

    // SpatialGrid:
    let grid_query_start = Instant::now();
    let mut grid_hits = 0;
    for &p in &test_points {
        let candidates = grid.query_point(p, 16.0);
        for &idx in &candidates {
            if hit_test_object(&strokes[idx], p, 16.0) {
                grid_hits += 1;
            }
        }
    }
    let grid_query_duration = grid_query_start.elapsed();

    println!("\n--- Hit Test Query Comparison (500 queries) ---");
    println!(
        "Brute Force O(N):   {:.2?} (Hits: {})",
        bf_duration, bf_hits
    );
    println!(
        "SpatialGrid O(k):   {:.2?} (Hits: {})",
        grid_query_duration, grid_hits
    );
    let speedup = bf_duration.as_micros() as f64 / grid_query_duration.as_micros().max(1) as f64;
    println!(">>> Speedup: {:.1}x faster with SpatialGrid!", speedup);
}

fn main() {
    println!("=======================================================");
    println!("      RUSTNOTES INKING & SPATIAL PERFORMANCE BENCH     ");
    println!("=======================================================");

    run_benchmark(500, 40);
    run_benchmark(2000, 60);
    run_benchmark(5000, 80);
}
