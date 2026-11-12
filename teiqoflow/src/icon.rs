use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point, Shader, SpreadMode, Transform,
};

pub const ACCENT_1: (u8, u8, u8) = (0x25, 0x63, 0xEB);
pub const ACCENT_2: (u8, u8, u8) = (0x38, 0xBD, 0xF8);

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn gradient(x0: f32, y0: f32, x1: f32, y1: f32, alpha: f32) -> Shader<'static> {
    gradient_between(ACCENT_1, ACCENT_2, x0, y0, x1, y1, alpha)
}

pub fn gradient_between(c0: (u8, u8, u8), c1: (u8, u8, u8), x0: f32, y0: f32, x1: f32, y1: f32, alpha: f32) -> Shader<'static> {
    let c = |(r, g, b): (u8, u8, u8)| Color::from_rgba8(r, g, b, (alpha * 255.0) as u8);
    LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        vec![GradientStop::new(0.0, c(c0)), GradientStop::new(1.0, c(c1))],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(c(c0)))
}

pub fn paint(shader: Shader<'static>) -> Paint<'static> {
    let mut p = Paint::default();
    p.shader = shader;
    p.anti_alias = true;
    p
}

/// The app logo: gradient squircle with a white waveform. Returns straight RGBA.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let mut pm = Pixmap::new(size, size).unwrap();
    let s = size as f32;
    if let Some(path) = rounded_rect(0.0, 0.0, s, s, s * 0.28) {
        pm.fill_path(&path, &paint(gradient(0.0, 0.0, s, s, 1.0)), FillRule::Winding, Transform::identity(), None);
    }
    let white = paint(Shader::SolidColor(Color::WHITE));
    let bars = [0.28, 0.55, 0.8, 0.5, 0.3];
    let bw = s * 0.09;
    let gap = s * 0.06;
    let total = bars.len() as f32 * bw + (bars.len() - 1) as f32 * gap;
    let mut x = (s - total) / 2.0;
    for h in bars {
        let bh = s * h * 0.75;
        if let Some(path) = rounded_rect(x, (s - bh) / 2.0, bw, bh, bw / 2.0) {
            pm.fill_path(&path, &white, FillRule::Winding, Transform::identity(), None);
        }
        x += bw + gap;
    }
    // tiny-skia stores premultiplied colour; icons expect straight alpha.
    pm.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}
