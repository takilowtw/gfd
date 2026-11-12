//! Look and feel shared by the app and the installer: colours, bundled fonts and custom widgets.
//! The installer includes this file with `#[path]`, so it must not refer to other app modules.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Mesh, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Base colours of one theme. The accent colour is chosen separately.
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_2: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Darker edge under keycaps.
    pub key_edge: Color32,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x0B, 0x0B, 0x10),
    panel: Color32::from_rgb(0x14, 0x14, 0x1C),
    panel_2: Color32::from_rgb(0x1B, 0x1B, 0x26),
    border: Color32::from_rgb(0x26, 0x26, 0x33),
    text: Color32::from_rgb(0xED, 0xED, 0xF3),
    muted: Color32::from_rgb(0x8B, 0x8B, 0x9E),
    key_edge: Color32::from_rgb(0x3A, 0x3A, 0x4D),
};

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF5, 0xF5, 0xF8),
    panel: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    panel_2: Color32::from_rgb(0xEC, 0xEC, 0xF2),
    border: Color32::from_rgb(0xDD, 0xDD, 0xE6),
    text: Color32::from_rgb(0x16, 0x16, 0x1E),
    muted: Color32::from_rgb(0x6A, 0x6A, 0x7C),
    key_edge: Color32::from_rgb(0xC4, 0xC4, 0xD2),
};

static LIGHT_THEME: AtomicBool = AtomicBool::new(false);

pub fn set_light_theme(light: bool) {
    LIGHT_THEME.store(light, Ordering::Relaxed);
}

pub fn is_light_theme() -> bool {
    LIGHT_THEME.load(Ordering::Relaxed)
}

pub fn palette() -> &'static Palette {
    if is_light_theme() { &LIGHT } else { &DARK }
}

pub fn bg() -> Color32 {
    palette().bg
}
pub fn panel() -> Color32 {
    palette().panel
}
pub fn panel_2() -> Color32 {
    palette().panel_2
}
pub fn border() -> Color32 {
    palette().border
}
pub fn text_color() -> Color32 {
    palette().text
}
pub fn muted_color() -> Color32 {
    palette().muted
}

pub const OK: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x97);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xB5, 0x47);
pub const ERR: Color32 = Color32::from_rgb(0xFF, 0x5C, 0x70);

/// Accent presets: id, English name, gradient start and end.
pub const ACCENTS: &[(&str, &str, Color32, Color32)] = &[
    ("blue", "Blue", Color32::from_rgb(0x25, 0x63, 0xEB), Color32::from_rgb(0x38, 0xBD, 0xF8)),
    ("violet", "Violet", Color32::from_rgb(0x7C, 0x3A, 0xED), Color32::from_rgb(0xC0, 0x84, 0xFC)),
    ("pink", "Pink", Color32::from_rgb(0xDB, 0x27, 0x77), Color32::from_rgb(0xF4, 0x72, 0xB6)),
    ("red", "Red", Color32::from_rgb(0xDC, 0x26, 0x26), Color32::from_rgb(0xFB, 0x71, 0x85)),
    ("orange", "Orange", Color32::from_rgb(0xEA, 0x58, 0x0C), Color32::from_rgb(0xFB, 0xBF, 0x24)),
    ("green", "Green", Color32::from_rgb(0x05, 0x96, 0x69), Color32::from_rgb(0x4A, 0xDE, 0x80)),
    ("teal", "Teal", Color32::from_rgb(0x0D, 0x94, 0x88), Color32::from_rgb(0x22, 0xD3, 0xEE)),
];

// Index into ACCENTS. Atomic because the overlay thread reads it too.
static ACCENT: AtomicUsize = AtomicUsize::new(0);

pub fn set_accent(id: &str) {
    ACCENT.store(ACCENTS.iter().position(|a| a.0 == id).unwrap_or(0), Ordering::Relaxed);
}

pub fn accent_1() -> Color32 {
    ACCENTS[ACCENT.load(Ordering::Relaxed)].2
}

pub fn accent_2() -> Color32 {
    ACCENTS[ACCENT.load(Ordering::Relaxed)].3
}

/// Corner radius of the app and installer windows.
pub const WINDOW_RADIUS: f32 = 16.0;

// Wispr Flow's typefaces: Figtree for the interface, EB Garamond for display headings.
// Figtree has no Cyrillic, so Onest (a close geometric match) supplies those glyphs.
// All are SIL Open Font License; see assets/fonts/OFL-*.txt.
const FIGTREE: &[u8] = include_bytes!("../assets/fonts/Figtree-Regular.ttf");
const FIGTREE_SB: &[u8] = include_bytes!("../assets/fonts/Figtree-SemiBold.ttf");
const ONEST: &[u8] = include_bytes!("../assets/fonts/Onest-Regular.ttf");
const ONEST_SB: &[u8] = include_bytes!("../assets/fonts/Onest-SemiBold.ttf");
pub const GARAMOND: &[u8] = include_bytes!("../assets/fonts/EBGaramond-Regular.ttf");
pub const GARAMOND_IT: &[u8] = include_bytes!("../assets/fonts/EBGaramond-Italic.ttf");
/// SemiBold faces for code that rasterises text itself (the overlay pill).
pub const UI_SEMIBOLD: [&[u8]; 2] = [FIGTREE_SB, ONEST_SB];

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn serif(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("serif".into()))
}

pub fn serif_italic(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("serif-italic".into()))
}

pub fn setup_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("figtree", FIGTREE),
        ("figtree-sb", FIGTREE_SB),
        ("onest", ONEST),
        ("onest-sb", ONEST_SB),
        ("garamond", GARAMOND),
        ("garamond-it", GARAMOND_IT),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }

    // System fonts only as fallbacks for symbols and scripts the bundled fonts lack.
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\Fonts\\";
    let mut fallbacks = Vec::new();
    for (name, file) in [
        ("segoe", "segoeui.ttf"),
        ("segoe-sym", "seguisym.ttf"),
        ("yahei", "msyh.ttc"),
        ("yugothic", "YuGothM.ttc"),
        ("malgun", "malgun.ttf"),
        ("nirmala", "Nirmala.ttc"),
        ("nirmala2", "Nirmala.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(format!("{dir}{file}")) {
            fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            fallbacks.push(name.to_string());
        }
    }
    let default_prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |first: &[&str]| -> Vec<String> {
        first.iter().map(|s| s.to_string()).chain(fallbacks.iter().cloned()).chain(default_prop.iter().cloned()).collect()
    };
    fonts.families.insert(FontFamily::Proportional, family(&["figtree", "onest"]));
    fonts.families.insert(FontFamily::Name("semibold".into()), family(&["figtree-sb", "onest-sb"]));
    fonts.families.insert(FontFamily::Name("serif".into()), family(&["garamond", "onest"]));
    fonts.families.insert(FontFamily::Name("serif-italic".into()), family(&["garamond-it", "garamond", "onest"]));
    ctx.set_fonts(fonts);
    apply_visuals(ctx);
}

/// Spacing and colours. Called again when the accent colour changes.
pub fn apply_visuals(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(14.0, 8.0);
    style.spacing.interact_size.y = 34.0;
    style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.5));
    style.text_styles.insert(egui::TextStyle::Button, semibold(14.0));
    let v = &mut style.visuals;
    *v = if is_light_theme() { egui::Visuals::light() } else { egui::Visuals::dark() };
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = panel_2();
    v.window_stroke = Stroke::new(1.0, border());
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(12);
    v.extreme_bg_color = panel_2();
    v.override_text_color = Some(text_color());
    v.selection.bg_fill = accent_1();
    v.selection.stroke = Stroke::new(1.0, text_color());
    v.hyperlink_color = accent_2();
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(10);
        w.bg_fill = panel_2();
        w.weak_bg_fill = panel_2();
        w.bg_stroke = Stroke::new(1.0, border());
        w.fg_stroke = Stroke::new(1.2, text_color());
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, accent_1());
    v.widgets.active.bg_stroke = Stroke::new(1.0, accent_2());
    v.widgets.open.bg_stroke = Stroke::new(1.0, accent_1());
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border());
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_color());
    ctx.set_global_style(style);
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Rounded rectangle with a diagonal two-colour gradient. The edge gets a one-pixel feathered
/// ring, as egui does for its own shapes, so the corners are smooth instead of jagged.
pub fn gradient_rect(ui: &Ui, rect: Rect, radius: f32, c0: Color32, c1: Color32) {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let feather = 1.0 / ui.ctx().pixels_per_point();
    const SEGMENTS: usize = 16;
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -90.0f32),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    // (point on the outline, outward normal)
    let mut outline: Vec<(Pos2, egui::Vec2)> = Vec::new();
    for (c, start) in corners {
        for i in 0..=SEGMENTS {
            let a = (start + i as f32 * 90.0 / SEGMENTS as f32).to_radians();
            let n = vec2(a.cos(), a.sin());
            outline.push((c + n * r, n));
        }
    }
    let diag = (rect.width() + rect.height()).max(1.0);
    let color_at = |p: Pos2| lerp_color(c0, c1, ((p.x - rect.left()) + (p.y - rect.top())) / diag);

    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), color_at(rect.center()));
    for (p, n) in &outline {
        let inner = *p - *n * (feather / 2.0);
        mesh.colored_vertex(inner, color_at(inner));
    }
    for (p, n) in &outline {
        mesh.colored_vertex(*p + *n * (feather / 2.0), Color32::TRANSPARENT);
    }
    let n = outline.len() as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(0, 1 + i, 1 + j);
        mesh.add_triangle(1 + i, 1 + n + i, 1 + n + j);
        mesh.add_triangle(1 + i, 1 + n + j, 1 + j);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), semibold(14.0), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(36.0, 18.0), Sense::click());
    let k = ui.ctx().animate_bool(resp.id.with("hover"), resp.hovered());
    let c0 = lerp_color(accent_1(), Color32::WHITE, 0.15 * k);
    let c1 = lerp_color(accent_2(), Color32::WHITE, 0.15 * k);
    gradient_rect(ui, rect, 10.0, c0, c1);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn ghost_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).font(semibold(13.0))).fill(panel_2()).corner_radius(10))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = vec2(46.0, 26.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let p = ui.painter();
    p.rect_filled(rect, 13.0, panel_2());
    p.rect_stroke(rect, 13.0, Stroke::new(1.0, border()), egui::StrokeKind::Inside);
    if t > 0.01 {
        let a = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (t * 255.0) as u8);
        gradient_rect(ui, rect, 13.0, a(accent_1()), a(accent_2()));
    }
    let x = rect.left() + 13.0 + t * (rect.width() - 26.0);
    ui.painter().circle_filled(pos2(x, rect.center().y), 10.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Rounded window background for frameless, transparent windows.
pub fn window_background(ui: &Ui) {
    let ctx = ui.ctx();
    let full = ctx.content_rect();
    let ppp = ctx.pixels_per_point();
    // The top pixel row is clipped away (see clip_window_corners), so start one pixel lower.
    let shape = Rect::from_min_max(full.min + vec2(0.0, 1.0 / ppp), full.max);
    ui.painter().rect(shape, WINDOW_RADIUS, bg(), Stroke::new(1.0, border()), egui::StrokeKind::Inside);

    // Re-clip whenever the physical size or scale changes.
    let key = [(full.width() * ppp).round(), (full.height() * ppp).round(), ppp];
    let id = ui.make_persistent_id("window_region");
    if ctx.data(|d| d.get_temp::<[f32; 3]>(id)) != Some(key) && clip_window_corners(WINDOW_RADIUS * ppp) {
        ctx.data_mut(|d| d.insert_temp(id, key));
    }
}

#[cfg(windows)]
static HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Tells Windows 11 not to round or outline the window itself: the app draws its own rounded
/// edge, and a second system outline is what made the corners look doubled.
#[cfg(windows)]
pub fn disable_system_frame(title: &str) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return;
        }
        HWND.store(hwnd as isize, Ordering::Relaxed);
        let set = |attr: i32, value: u32| {
            DwmSetWindowAttribute(hwnd, attr as u32, &value as *const u32 as *const _, 4);
        };
        set(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND as u32);
        set(DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
    }
}

/// Clips the window to a rounded shape that also leaves out the top pixel row. Windows draws a 1px
/// frame line along the top of frameless windows, and a shadow around the square bounds; both
/// showed past the drawn corners. The region's radius is a little tighter than the drawn one so
/// the anti-aliased edge pixels stay inside. Returns false until the window is known.
#[cfg(windows)]
fn clip_window_corners(radius_px: f32) -> bool {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let hwnd = HWND.load(Ordering::Relaxed) as windows_sys::Win32::Foundation::HWND;
    if hwnd.is_null() {
        return false;
    }
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return false;
        }
        let d = (radius_px * 2.0).round() as i32 - 2;
        let rgn = CreateRoundRectRgn(0, 1, r.right - r.left + 1, r.bottom - r.top + 1, d, d);
        if rgn.is_null() {
            return false;
        }
        // On success the system owns the region.
        if SetWindowRgn(hwnd, rgn, 1) == 0 {
            DeleteObject(rgn);
            return false;
        }
    }
    true
}

#[cfg(not(windows))]
fn clip_window_corners(_radius_px: f32) -> bool {
    true
}
