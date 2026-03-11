#[cfg(windows)]
use windows_sys::Win32::Foundation::POINT;
#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MOVE, MOUSEINPUT,
};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetForegroundWindow, IsIconic, SetForegroundWindow, ShowWindow,
    SW_RESTORE,
};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::DrawingConfig;
use crate::types::DrawingStroke;

#[cfg(windows)]
fn send_mouse_event(flags: u32, dx: i32, dy: i32) {
    let mut input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { dx, dy, mouseData: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };
    unsafe { SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32); }
}

pub fn move_cursor_relative(dx: i32, dy: i32) {
    #[cfg(windows)]
    { if dx != 0 || dy != 0 { send_mouse_event(MOUSEEVENTF_MOVE, dx, dy); } }
    #[cfg(not(windows))]
    { let _ = (dx, dy); }
}

pub fn press_left() {
    #[cfg(windows)]
    send_mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0);
}

pub fn release_left() {
    #[cfg(windows)]
    send_mouse_event(MOUSEEVENTF_LEFTUP, 0, 0);
}

#[allow(unreachable_code)]
pub fn get_cursor() -> (i32, i32) {
    #[cfg(windows)]
    {
        let mut pt = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut pt); }
        return (pt.x, pt.y);
    }
    (0, 0)
}

#[allow(unreachable_code)]
pub fn focus_vrchat_window() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let name: Vec<u16> = std::ffi::OsStr::new("VRChat")
            .encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            let hwnd = FindWindowW(std::ptr::null(), name.as_ptr());
            if !hwnd.is_null() {
                if IsIconic(hwnd) != 0 { ShowWindow(hwnd, SW_RESTORE); }
                SetForegroundWindow(hwnd);
                thread::sleep(Duration::from_millis(500));
                return GetForegroundWindow() == hwnd;
            }
        }
        return false;
    }
    false
}

fn stroke_bounds(strokes: &[DrawingStroke]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for s in strokes {
        for p in &s.points {
            if p.x < min_x { min_x = p.x; }
            if p.x > max_x { max_x = p.x; }
            if p.y < min_y { min_y = p.y; }
            if p.y > max_y { max_y = p.y; }
        }
    }
    (min_x, min_y, max_x, max_y)
}

fn densify_stroke(points: &[(i32, i32)], max_step_px: i32) -> Vec<(i32, i32)> {
    if points.is_empty() || max_step_px <= 1 {
        return points.to_vec();
    }
    let mut dense = Vec::new();
    dense.push(points[0]);
    for target in points.iter().skip(1) {
        let last = *dense.last().unwrap();
        let dx = (target.0 - last.0) as f64;
        let dy = (target.1 - last.1) as f64;
        let distance = dx.hypot(dy);
        let mf = max_step_px as f64;
        if distance > mf {
            let steps = (distance / mf).floor() as i32;
            for step in 1..=steps {
                let ratio = (step as f64 * mf / distance).min(1.0);
                dense.push((
                    (last.0 as f64 + dx * ratio).round() as i32,
                    (last.1 as f64 + dy * ratio).round() as i32,
                ));
            }
        }
        dense.push(*target);
    }
    dense
}

fn order_strokes_by_proximity(strokes: Vec<Vec<(i32, i32)>>) -> Vec<Vec<(i32, i32)>> {
    let mut ordered = Vec::new();
    let mut remaining = strokes;
    if remaining.is_empty() { return ordered; }

    ordered.push(remaining.remove(0));
    while !remaining.is_empty() {
        let last_point = ordered.last().unwrap().last().unwrap();
        let mut best_index = 0;
        let mut best_distance = f64::INFINITY;
        let mut best_reversed = false;

        for (idx, stroke) in remaining.iter().enumerate() {
            if stroke.is_empty() { continue; }
            let s = stroke[0];
            let e = stroke.last().unwrap();

            let d_start = ((s.0 - last_point.0) as f64).hypot((s.1 - last_point.1) as f64);
            let d_end = ((e.0 - last_point.0) as f64).hypot((e.1 - last_point.1) as f64);

            if d_start < best_distance {
                best_distance = d_start;
                best_index = idx;
                best_reversed = false;
            }
            if d_end < best_distance {
                best_distance = d_end;
                best_index = idx;
                best_reversed = true;
            }
        }

        let mut next = remaining.remove(best_index);
        if best_reversed { next.reverse(); }
        ordered.push(next);
    }
    ordered
}

fn move_relatively(
    current_x: i32,
    current_y: i32,
    target: (i32, i32),
    cfg: &DrawingConfig,
    active: &Arc<AtomicBool>,
    delay: bool,
    draw_delay_sec: f64,
) -> (i32, i32) {
    let dx = target.0 - current_x;
    let dy = target.1 - current_y;
    let distance = (dx as f64).hypot(dy as f64);
    if distance == 0.0 {
        return (current_x, current_y);
    }

    let max_step = cfg.max_step_px.max(1) as f64;
    let steps = (distance / max_step).ceil().max(1.0) as i32;
    let step_dx = dx as f64 / steps as f64;
    let step_dy = dy as f64 / steps as f64;

    let mut accum_x = current_x as f64;
    let mut accum_y = current_y as f64;
    let mut last_int_x = current_x;
    let mut last_int_y = current_y;

    for _ in 0..steps {
        if !active.load(Ordering::SeqCst) { break; }
        accum_x += step_dx;
        accum_y += step_dy;
        let next_int_x = accum_x.round() as i32;
        let next_int_y = accum_y.round() as i32;
        
        let rel_dx = next_int_x - last_int_x;
        let rel_dy = next_int_y - last_int_y;

        if rel_dx != 0 || rel_dy != 0 {
            move_cursor_relative(rel_dx, rel_dy);
            last_int_x += rel_dx;
            last_int_y += rel_dy;
        }

        if delay {
            thread::sleep(Duration::from_micros((draw_delay_sec * 1_000_000.0) as u64));
        }
    }

    (last_int_x, last_int_y)
}

pub struct Drawer {
    pub active: Arc<AtomicBool>,
}

impl Drawer {
    pub fn new() -> Self { Self { active: Arc::new(AtomicBool::new(false)) } }

    pub fn start_drawing(&self, strokes: Vec<DrawingStroke>, cfg: DrawingConfig) -> Option<thread::JoinHandle<()>> {
        if self.active.load(Ordering::SeqCst) || strokes.is_empty() { return None; }
        if !focus_vrchat_window() { eprintln!("Warn: Could not focus VRChat."); }
        
        self.active.store(true, Ordering::SeqCst);
        let flag = self.active.clone();

        Some(thread::spawn(move || Self::draw_strokes_thread(strokes, cfg, flag)))
    }

    pub fn stop_drawing(&self) {
        self.active.store(false, Ordering::SeqCst);
        release_left();
    }

    fn draw_strokes_thread(strokes: Vec<DrawingStroke>, cfg: DrawingConfig, active: Arc<AtomicBool>) {
        if strokes.is_empty() { return; }

        let (start_x, start_y) = get_cursor();
        let (min_x, min_y, max_x, max_y) = stroke_bounds(&strokes);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;

        let scale = cfg.pixel_to_screen_scale;
        let stretch = cfg.vertical_stretch;

        let offset_x = start_x as f64 - center_x * scale;
        // Apply stretch implicitly to y:
        let offset_y = start_y as f64 - center_y * scale * stretch;

        thread::sleep(Duration::from_secs_f64(cfg.start_delay));

        let mut scaled_strokes = Vec::new();
        for stroke in strokes {
            if !active.load(Ordering::SeqCst) { break; }
            if stroke.points.is_empty() { continue; }
            
            let mut screen_points = Vec::new();
            for p in &stroke.points {
                let screen_x = (p.x * scale + offset_x).round() as i32;
                let screen_y = (p.y * scale * stretch + offset_y).round() as i32;
                screen_points.push((screen_x, screen_y));
            }
            scaled_strokes.push(densify_stroke(&screen_points, cfg.max_step_px));
        }

        let ordered = order_strokes_by_proximity(scaled_strokes);

        let mut current_x = start_x;
        let mut current_y = start_y;
        
        let draw_delay = cfg.draw_speed; // No hardcoded 0.09 floor! Let it go fast!
        let lift_delay = cfg.lift_pen_delay;

        for stroke in ordered {
            if !active.load(Ordering::SeqCst) || stroke.is_empty() { break; }

            // Move to start with no delay (fast)
            let (nx, ny) = move_relatively(
                current_x, current_y, stroke[0], &cfg, &active, false, draw_delay
            );
            current_x = nx; current_y = ny;
            
            if !active.load(Ordering::SeqCst) { break; }

            // NEW: Arrival Sync to prevent "Early Pressing" on large jumps
            // Flush the input queue with a 0-pixel relative move and a landing delay
            crate::drawer::move_cursor_relative(0, 0); 
            thread::sleep(Duration::from_millis(60));
            
            // Dynamic timing: if a stroke has very few points, VRChat will drop it due to polling rate.
            // A 60ms minimum total drawing time ensures the game engine processes the left-down event.
            let stroke_pts = stroke.len().max(1) as f64;
            let default_duration = stroke_pts * draw_delay;
            let target_min_duration = 0.060; // 60ms magic minimum for VRChat
            
            let dynamic_delay = if default_duration < target_min_duration {
                target_min_duration / stroke_pts
            } else {
                draw_delay
            };

            thread::sleep(Duration::from_micros((draw_delay * 1_000_000.0) as u64));
            press_left();
            thread::sleep(Duration::from_micros((dynamic_delay * 1_000_000.0) as u64));

            for point in stroke.iter().skip(1) {
                if !active.load(Ordering::SeqCst) { break; }
                let (nx, ny) = move_relatively(
                    current_x, current_y, *point, &cfg, &active, true, dynamic_delay
                );
                current_x = nx; current_y = ny;
            }

            release_left();
            
            // Hard Sync: Wait for the game to process the release before moving elsewhere
            thread::sleep(Duration::from_millis(30));
            
            // Wait for the requested lift delay (min 40ms total)
            let actual_lift_delay = lift_delay.max(0.040);
            thread::sleep(Duration::from_micros((actual_lift_delay * 1_000_000.0) as u64));
            
            // Final settle pause before jumping to next stroke to prevent drag
            thread::sleep(Duration::from_millis(20));
        }

        active.store(false, Ordering::SeqCst);
        release_left();
    }
}
