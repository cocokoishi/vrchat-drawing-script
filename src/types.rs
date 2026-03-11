#[derive(Debug, Clone, PartialEq)]
pub struct DrawingPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default)]
pub struct DrawingStroke {
    pub points: Vec<DrawingPoint>,
}
