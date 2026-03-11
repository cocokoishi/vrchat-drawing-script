#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub grayscale: bool,
    pub blur_size: u32,
    pub threshold_type: String,
    pub threshold_value: u8,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            grayscale: true,
            blur_size: 1,             // No blur by default (input is usually clean B&W art)
            threshold_type: "binary".to_string(),  // Binary threshold, not otsu (more predictable)
            threshold_value: 128,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContourConfig {
    pub min_contour_length: f64,
    pub epsilon_ratio: f64,
}

impl Default for ContourConfig {
    fn default() -> Self {
        Self {
            min_contour_length: 5.0,
            epsilon_ratio: 1.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawingConfig {
    pub draw_speed: f64,         // seconds per point
    pub pixel_to_screen_scale: f64,
    pub max_step_px: i32,
    pub lift_pen_delay: f64,
    pub start_delay: f64,
    pub sensitivity: f64,
    pub vertical_stretch: f64,
    pub lift_pen_speed: f64,
}

impl Default for DrawingConfig {
    fn default() -> Self {
        Self {
            draw_speed: 0.005,          // 5ms per point (vs original 90ms!)
            pixel_to_screen_scale: 1.0,
            max_step_px: 4,
            lift_pen_delay: 0.05,       // 50ms lift delay (too short causes bleeding)
            start_delay: 1.5,
            sensitivity: 1.2,
            vertical_stretch: 1.4,
            lift_pen_speed: 100.0,      // Instant pen-up movement
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub image: ImageConfig,
    pub contour: ContourConfig,
    pub drawing: DrawingConfig,
}
