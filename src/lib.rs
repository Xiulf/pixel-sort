pub mod img;
// pub mod vid;

pub struct Opts {
    pub sort_type: SortType,
    pub sort_fn: fn(&[u8]) -> u8,
    pub interval: IntervalType,
    pub mask_alpha: bool,
    pub invert: bool,
    pub reverse: bool,
    pub split: bool,
    pub min: usize,
    pub max: usize,
    pub angle: f64,
    pub vertical: bool,
    pub resize: Option<Scale>,
    pub internal_scale: Option<Scale>,
}

pub enum SortType {
    Linear,
    Spiral,
    Circle { cx: u32, cy: u32 },
    Sine { amp: f64, lam: f64, offset: f64 },
}

pub enum IntervalType {
    Random,
    Threshold,
}

#[derive(Clone, Copy)]
pub enum Scale {
    Pixels(u32, u32),
    Multiply(f32),
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            sort_type: SortType::Linear,
            sort_fn: pixel_max,
            interval: IntervalType::Random,
            mask_alpha: false,
            invert: false,
            reverse: false,
            split: false,
            min: 0,
            max: 255,
            angle: 0.0,
            vertical: false,
            resize: None,
            internal_scale: None,
        }
    }
}

impl Scale {
    pub fn calc(self, w: u32, h: u32) -> (u32, u32) {
        match self {
            Scale::Pixels(w, h) => (w, h),
            Scale::Multiply(m) => ((w as f32 * m) as u32, (h as f32 * m) as u32),
        }
    }
}

pub fn pixel_red(p: &[u8]) -> u8 {
    p[0]
}

pub fn pixel_green(p: &[u8]) -> u8 {
    p[1]
}

pub fn pixel_blue(p: &[u8]) -> u8 {
    p[2]
}

pub fn pixel_max(p: &[u8]) -> u8 {
    p[..3].iter().max().cloned().unwrap_or_default()
}

pub fn pixel_min(p: &[u8]) -> u8 {
    p[..3].iter().min().cloned().unwrap_or_default()
}

pub fn pixel_chroma(p: &[u8]) -> u8 {
    pixel_max(p) - pixel_min(p)
}

pub fn pixel_hue(p: &[u8]) -> u8 {
    let c = pixel_chroma(p);

    if c == 0 {
        return 0;
    }

    match p[..3].iter().enumerate().max_by_key(|&(_, e)| e) {
        Some((0, _)) => (i16::from(p[1]) - i16::from(p[2])).abs() as u8 / c * 43,
        Some((1, _)) => (i16::from(p[2]) - i16::from(p[0])).abs() as u8 / c * 43 + 85,
        Some((2, _)) => (i16::from(p[0]) - i16::from(p[1])).abs() as u8 / c * 43 + 171,
        _ => 0,
    }
}

pub fn pixel_saturation(p: &[u8]) -> u8 {
    match pixel_max(p) {
        0 => 0,
        v => pixel_chroma(p) / v,
    }
}

pub fn pixel_brightness(p: &[u8]) -> u8 {
    p[0] / 3 + p[1] / 3 + p[2] / 3 + (p[0] % 3 + p[1] % 3 + p[2] % 3) / 2
}

pub fn pixel_luma(p: &[u8]) -> u8 {
    ((u16::from(p[0]) * 2 + u16::from(p[1]) + u16::from(p[2]) * 4) >> 3) as u8
}
