//! Look and feel shared by the app and the installer: colours, bundled fonts and custom widgets.
//! The installer includes this file with `#[path]`, so it must not refer to other app modules.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Mesh, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x0B, 0x0B, 0x10);
pub const PANEL: Color32 = Color32::from_rgb(0x14, 0x14, 0x1C);
pub const PANEL_2: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x26);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x26, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xED, 0xED, 0xF3);
pub const MUTED: Color32 = Color32::from_rgb(0x8B, 0x8B, 0x9E);
pub const ACCENT_1: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
pub const ACCENT_2: Color32 = Color32::from_rgb(0x38, 0xBD, 0xF8);
pub const OK: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x97);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xB5, 0x47);
pub const ERR: Color32 = Color32::from_rgb(0xFF, 0x5C, 0x70);

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

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(14.0, 8.0);
    style.spacing.interact_size.y = 34.0;
    style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.5));
    style.text_styles.insert(egui::TextStyle::Button, semibold(14.0));
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = PANEL_2;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(12);
    v.extreme_bg_color = PANEL_2;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT_1;
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.hyperlink_color = ACCENT_2;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(10);
        w.bg_fill = PANEL_2;
        w.weak_bg_fill = PANEL_2;
        w.bg_stroke = Stroke::new(1.0, BORDER);
        w.fg_stroke = Stroke::new(1.2, TEXT);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_1);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_2);
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_1);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
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
    let c0 = lerp_color(ACCENT_1, Color32::from_rgb(0x3B, 0x76, 0xF6), k);
    let c1 = lerp_color(ACCENT_2, Color32::from_rgb(0x5C, 0xCB, 0xFA), k);
    gradient_rect(ui, rect, 10.0, c0, c1);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn ghost_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).font(semibold(13.0))).fill(PANEL_2).corner_radius(10))
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
    p.rect_filled(rect, 13.0, PANEL_2);
    p.rect_stroke(rect, 13.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    if t > 0.01 {
        let a = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (t * 255.0) as u8);
        gradient_rect(ui, rect, 13.0, a(ACCENT_1), a(ACCENT_2));
    }
    let x = rect.left() + 13.0 + t * (rect.width() - 26.0);
    ui.painter().circle_filled(pos2(x, rect.center().y), 10.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Rounded window background for frameless, transparent windows.
pub fn window_background(ui: &Ui) {
    let full = ui.ctx().content_rect();
    ui.painter().rect(full, WINDOW_RADIUS, BG, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
}

/// Tells Windows 11 not to round or outline the window itself: the app draws its own rounded
/// edge, and a second system outline is what made the corners look doubled.
#[cfg(windows)]
pub fn disable_system_frame(title: &str) {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute};
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return;
        }
        let set = |attr: i32, value: u32| {
            DwmSetWindowAttribute(hwnd, attr as u32, &value as *const u32 as *const _, 4);
        };
        set(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND as u32);
        set(DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
    }
}
