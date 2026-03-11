use crate::config::{AppConfig, ContourConfig, ImageConfig};
use crate::types::{DrawingPoint, DrawingStroke};
use image::DynamicImage;
use imageproc::filter::gaussian_blur_f32;
use std::path::Path;

pub fn process_image(img_path: &Path, config: &AppConfig) -> Option<Vec<DrawingStroke>> {
    let img = image::open(img_path).ok()?;
    let (w, h, mut data) = preprocess_image(&img, &config.image);
    skeletonize_flat(&mut data, w, h);
    
    // NEW: Prune the skeleton to remove branches shorter than 5px (spurs)
    prune_skeleton(&mut data, w, h, 5);
    
    let strokes = extract_strokes(&data, w, h, &config.contour);
    if strokes.is_empty() { return Some(Vec::new()); }
    Some(strokes)
}

/// Preprocess: grayscale → blur → threshold → returns flat binary array (0/1)
fn preprocess_image(img: &DynamicImage, cfg: &ImageConfig) -> (usize, usize, Vec<u8>) {
    let mut gray = img.to_luma8();

    // Ensure blur size is odd
    let blur = if cfg.blur_size > 1 {
        if cfg.blur_size % 2 == 0 { cfg.blur_size + 1 } else { cfg.blur_size }
    } else { 0 };

    if blur > 1 {
        let sigma = (blur as f32) / 3.0;
        gray = gaussian_blur_f32(&gray, sigma);
    }

    let w = gray.width() as usize;
    let h = gray.height() as usize;

    // Build binary: local adaptive thresholding is much more robust than global Otsu
    let raw = gray.into_raw();
    let data = adaptive_threshold_local(&raw, w, h, 15, 10);
    
    // NEW: Morphological Closing (Dilate then Erode) to bridge small gaps
    let dilated = dilate_binary(&data, w, h);
    let closed = erode_binary(&dilated, w, h);

    (w, h, closed)
}

fn adaptive_threshold_local(raw: &[u8], w: usize, h: usize, win: usize, offset: i32) -> Vec<u8> {
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        let y_min = y.saturating_sub(win);
        let y_max = (y + win).min(h - 1);
        for x in 0..w {
            let x_min = x.saturating_sub(win);
            let x_max = (x + win).min(w - 1);
            
            let mut sum: u32 = 0;
            let mut count: u32 = 0;
            for py in y_min..=y_max {
                for px in x_min..=x_max {
                    sum += raw[py * w + px] as u32;
                    count += 1;
                }
            }
            let mean = (sum / count) as i32;
            if (raw[y * w + x] as i32) < (mean - offset) {
                out[y * w + x] = 1;
            } else {
                out[y * w + x] = 0;
            }
        }
    }
    out
}

fn dilate_binary(data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = data.to_vec();
    for y in 1..(h-1) {
        for x in 1..(w-1) {
            if data[y * w + x] ==0 {
                if data[(y-1)*w + x] > 0 || data[(y+1)*w + x] > 0 || data[y*w + x-1] > 0 || data[y*w + x+1] > 0 {
                    out[y * w + x] = 1;
                }
            }
        }
    }
    out
}

fn erode_binary(data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = data.to_vec();
    for y in 1..(h-1) {
        for x in 1..(w-1) {
            if data[y * w + x] == 1 {
                if data[(y-1)*w + x] == 0 || data[(y+1)*w + x] == 0 || data[y*w + x-1] == 0 || data[y*w + x+1] == 0 {
                    out[y * w + x] = 0;
                }
            }
        }
    }
    out
}

/// Zhang-Suen thinning on flat Vec<u8> (row-major, values 0 or 1).
fn skeletonize_flat(data: &mut [u8], w: usize, h: usize) {
    let mut changed = true;
    let mut to_clear: Vec<usize> = Vec::new();

    while changed {
        changed = false;

        // Step 1
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let idx = y * w + x;
                if data[idx] == 0 { continue; }

                let p2 = data[(y - 1) * w + x];
                let p3 = data[(y - 1) * w + x + 1];
                let p4 = data[y * w + x + 1];
                let p5 = data[(y + 1) * w + x + 1];
                let p6 = data[(y + 1) * w + x];
                let p7 = data[(y + 1) * w + x - 1];
                let p8 = data[y * w + x - 1];
                let p9 = data[(y - 1) * w + x - 1];

                let a = (p2 == 0 && p3 == 1) as u8 + (p3 == 0 && p4 == 1) as u8
                    + (p4 == 0 && p5 == 1) as u8 + (p5 == 0 && p6 == 1) as u8
                    + (p6 == 0 && p7 == 1) as u8 + (p7 == 0 && p8 == 1) as u8
                    + (p8 == 0 && p9 == 1) as u8 + (p9 == 0 && p2 == 1) as u8;
                let b = p2 + p3 + p4 + p5 + p6 + p7 + p8 + p9;

                if a == 1 && b >= 2 && b <= 6 && (p2 * p4 * p6) == 0 && (p4 * p6 * p8) == 0 {
                    to_clear.push(idx);
                }
            }
        }
        if !to_clear.is_empty() {
            changed = true;
            for &i in &to_clear { data[i] = 0; }
            to_clear.clear();
        }

        // Step 2
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let idx = y * w + x;
                if data[idx] == 0 { continue; }

                let p2 = data[(y - 1) * w + x];
                let p3 = data[(y - 1) * w + x + 1];
                let p4 = data[y * w + x + 1];
                let p5 = data[(y + 1) * w + x + 1];
                let p6 = data[(y + 1) * w + x];
                let p7 = data[(y + 1) * w + x - 1];
                let p8 = data[y * w + x - 1];
                let p9 = data[(y - 1) * w + x - 1];

                let a = (p2 == 0 && p3 == 1) as u8 + (p3 == 0 && p4 == 1) as u8
                    + (p4 == 0 && p5 == 1) as u8 + (p5 == 0 && p6 == 1) as u8
                    + (p6 == 0 && p7 == 1) as u8 + (p7 == 0 && p8 == 1) as u8
                    + (p8 == 0 && p9 == 1) as u8 + (p9 == 0 && p2 == 1) as u8;
                let b = p2 + p3 + p4 + p5 + p6 + p7 + p8 + p9;

                if a == 1 && b >= 2 && b <= 6 && (p2 * p4 * p8) == 0 && (p2 * p6 * p8) == 0 {
                    to_clear.push(idx);
                }
            }
        }
        if !to_clear.is_empty() {
            changed = true;
            for &i in &to_clear { data[i] = 0; }
            to_clear.clear();
        }
    }
}

/// Removes "spurs" (branches shorter than min_len that end in a dead end).
fn prune_skeleton(data: &mut [u8], w: usize, h: usize, min_len: usize) {
    let mut changed = true;
    while changed {
        changed = false;
        let mut to_remove = Vec::new();

        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                let idx = y * w + x;
                if data[idx] == 0 { continue; }

                // Count neighbors
                let mut neighbors = Vec::new();
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 { continue; }
                        let nix = ((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize;
                        if data[nix] > 0 { neighbors.push(nix); }
                    }
                }

                // If it's an endpoint (1 neighbor), trace the branch
                if neighbors.len() == 1 {
                    let mut branch = vec![idx];
                    let mut curr = neighbors[0];
                    let mut prev = idx;
                    
                    while branch.len() <= min_len {
                        branch.push(curr);
                        let mut next_neighbors = Vec::new();
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                if dx == 0 && dy == 0 { continue; }
                                let ny = (curr / w) as i32 + dy;
                                let nx = (curr % w) as i32 + dx;
                                let nix = (ny as usize) * w + (nx as usize);
                                if data[nix] > 0 && nix != prev { next_neighbors.push(nix); }
                            }
                        }
                        
                        if next_neighbors.len() == 1 {
                            prev = curr;
                            curr = next_neighbors[0];
                        } else {
                            // If it hits a junction (>1 neighbor) or a dead end (0 neighbors)
                            if next_neighbors.len() > 1 || next_neighbors.is_empty() {
                                if branch.len() < min_len {
                                    for &b_idx in &branch { to_remove.push(b_idx); }
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        if !to_remove.is_empty() {
            for &idx in &to_remove { data[idx] = 0; }
            changed = true;
        }
    }
}

fn extract_strokes(data: &[u8], w: usize, h: usize, cfg: &ContourConfig) -> Vec<DrawingStroke> {
    let mut visited = vec![false; w * h];
    let mut raw_strokes = Vec::new();
    let min_len = cfg.min_contour_length.max(1.0) as usize;

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if data[idx] > 0 && !visited[idx] {
                let path = trace_path(x, y, data, &mut visited, w, h);
                if path.len() >= min_len {
                    raw_strokes.push(path);
                }
            }
        }
    }

    // Simplify, close loops, then interpolate (matching original Python approach)
    let mut simplified = Vec::with_capacity(raw_strokes.len());
    for stroke in raw_strokes {
        let mut clean = rdp_simplify(&stroke, cfg.epsilon_ratio);
        if clean.len() < 2 { continue; }
        
        // NEW: Loop Closure Heuristic
        // If start and end are within 3 pixels, force a closed loop for circles.
        let start = clean.first().unwrap();
        let end = clean.last().unwrap();
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        if (dx * dx + dy * dy) < 9.0 && clean.len() > 3 {
             clean.push(start.clone());
        }

        // Interpolate: add points between distant vertices. 
        // 3.0px provides much smoother "continuous" lines in VRChat than 10px.
        let interp = interpolate_stroke(&clean, 3.0);
        if interp.len() > 1 {
            simplified.push(DrawingStroke { points: interp });
        }
    }

    reorder_strokes(simplified)
}

fn trace_path(
    sx: usize, sy: usize,
    data: &[u8], visited: &mut [bool], w: usize, h: usize,
) -> Vec<DrawingPoint> {
    const DIRS: [(i32, i32); 8] = [
        (0, -1), (1, 0), (0, 1), (-1, 0),
        (1, -1), (1, 1), (-1, 1), (-1, -1),
    ];

    let mut path = Vec::with_capacity(64);
    let wi = w as i32;
    let hi = h as i32;

    let mut current_pos = (sx as i32, sy as i32);
    let mut last_vec = (0.0_f64, 0.0_f64);

    loop {
        let (x, y) = current_pos;
        let idx = (y as usize) * w + (x as usize);
        if visited[idx] { break; }
        visited[idx] = true;
        path.push(DrawingPoint { x: x as f64, y: y as f64 });

        // Look for neighbors
        let mut neighbors = Vec::new();
        for &(dx, dy) in &DIRS {
            let nx = x + dx;
            let ny = y + dy;
            if nx >= 0 && nx < wi && ny >= 0 && ny < hi {
                let nidx = (ny as usize) * w + (nx as usize);
                if data[nidx] > 0 && !visited[nidx] {
                    neighbors.push((nx, ny));
                }
            }
        }

        if neighbors.is_empty() { break; }

        // Smart Junction Handling: Pick the neighbor that follows the straightest path
        let next_pos = if neighbors.len() == 1 {
            neighbors[0]
        } else {
            let mut best_idx = 0;
            let mut best_dot = -2.0;
            for (i, &(nx, ny)) in neighbors.iter().enumerate() {
                let dx = (nx - x) as f64;
                let dy = (ny - y) as f64;
                let mag = dx.hypot(dy);
                let dot = if mag > 0.0 {
                    (dx / mag) * last_vec.0 + (dy / mag) * last_vec.1
                } else { 0.0 };
                
                if dot > best_dot {
                    best_dot = dot;
                    best_idx = i;
                }
            }
            neighbors[best_idx]
        };

        // Update direction vector for next step
        let dx = (next_pos.0 - x) as f64;
        let dy = (next_pos.1 - y) as f64;
        let mag = dx.hypot(dy);
        if mag > 0.0 {
            last_vec = (dx / mag, dy / mag);
        }

        current_pos = next_pos;
    }
    path
}

/// RDP simplification with epsilon as direct pixel distance.
fn rdp_simplify(points: &[DrawingPoint], epsilon: f64) -> Vec<DrawingPoint> {
    if points.len() < 3 || epsilon <= 0.0 {
        return points.to_vec();
    }
    let eps_sq = epsilon * epsilon;

    fn rdp(pts: &[DrawingPoint], eps_sq: f64, result: &mut Vec<DrawingPoint>) {
        if pts.len() < 2 { return; }
        if pts.len() == 2 {
            result.push(pts[0].clone());
            result.push(pts[1].clone());
            return;
        }
        let end = pts.len() - 1;
        let (lx, ly) = (pts[0].x, pts[0].y);
        let (rx, ry) = (pts[end].x, pts[end].y);
        let line_len_sq = (rx - lx) * (rx - lx) + (ry - ly) * (ry - ly);

        let mut dmax = 0.0_f64;
        let mut index = 0;
        for i in 1..end {
            let d_sq = if line_len_sq < 1e-10 {
                let dx = pts[i].x - lx;
                let dy = pts[i].y - ly;
                dx * dx + dy * dy
            } else {
                let num = (ry - ly) * pts[i].x - (rx - lx) * pts[i].y + rx * ly - ry * lx;
                (num * num) / line_len_sq
            };
            if d_sq > dmax { index = i; dmax = d_sq; }
        }

        if dmax > eps_sq {
            let mut r1 = Vec::new();
            rdp(&pts[..=index], eps_sq, &mut r1);
            let mut r2 = Vec::new();
            rdp(&pts[index..=end], eps_sq, &mut r2);
            r1.pop();
            result.extend(r1);
            result.extend(r2);
        } else {
            result.push(pts[0].clone());
            result.push(pts[end].clone());
        }
    }

    let mut res = Vec::new();
    // Bypass RDP completely for very short detailed strokes (e.g. eyes/text)
    if points.len() < 6 {
        return points.to_vec();
    }
    
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for p in points {
        if p.x < min_x { min_x = p.x; }
        if p.x > max_x { max_x = p.x; }
        if p.y < min_y { min_y = p.y; }
        if p.y > max_y { max_y = p.y; }
    }
    
    // If the physical bounding box is tiny (e.g. under 5px), just keep the raw points
    if (max_x - min_x) < 5.0 && (max_y - min_y) < 5.0 {
        return points.to_vec();
    }
    
    rdp(points, eps_sq, &mut res);
    res
}

/// Interpolate points between distant vertices (matching original Python approach).
/// If two consecutive points are more than `max_dist` apart, sub-divide evenly.
fn interpolate_stroke(points: &[DrawingPoint], max_dist: f64) -> Vec<DrawingPoint> {
    if points.len() < 2 || max_dist <= 1.0 {
        return points.to_vec();
    }
    let mut result = Vec::with_capacity(points.len() * 2);
    result.push(points[0].clone());

    for i in 0..(points.len() - 1) {
        let p1 = &points[i];
        let p2 = &points[i + 1];
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > max_dist {
            let n = (dist / max_dist).ceil() as usize;
            for j in 1..n {
                let t = j as f64 / n as f64;
                result.push(DrawingPoint {
                    x: p1.x + t * dx,
                    y: p1.y + t * dy,
                });
            }
        }
        result.push(p2.clone());
    }
    result
}

/// Nearest-neighbor greedy reorder (squared distance, no sqrt).
fn reorder_strokes(strokes: Vec<DrawingStroke>) -> Vec<DrawingStroke> {
    if strokes.is_empty() { return Vec::new(); }

    let mut ordered = Vec::with_capacity(strokes.len());
    let mut remaining = strokes;

    remaining.sort_by(|a, b| {
        let da = a.points[0].x * a.points[0].x + a.points[0].y * a.points[0].y;
        let db = b.points[0].x * b.points[0].x + b.points[0].y * b.points[0].y;
        da.partial_cmp(&db).unwrap()
    });
    ordered.push(remaining.remove(0));

    while !remaining.is_empty() {
        let lp = ordered.last().unwrap().points.last().unwrap();
        let (lx, ly) = (lp.x, lp.y);
        let mut best_idx = 0;
        let mut best_sq = f64::INFINITY;
        let mut rev = false;

        for (i, s) in remaining.iter().enumerate() {
            let s0 = &s.points[0];
            let sn = s.points.last().unwrap();
            let ds = (s0.x - lx) * (s0.x - lx) + (s0.y - ly) * (s0.y - ly);
            let de = (sn.x - lx) * (sn.x - lx) + (sn.y - ly) * (sn.y - ly);
            if ds < best_sq { best_sq = ds; best_idx = i; rev = false; }
            if de < best_sq { best_sq = de; best_idx = i; rev = true; }
        }

        let mut next = remaining.remove(best_idx);
        if rev { next.points.reverse(); }
        ordered.push(next);
    }
    ordered
}
