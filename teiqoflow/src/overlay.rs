//! The floating "pill" at the bottom of the screen (like Wispr Flow).
//!
//! It is a native layered Win32 window that is click-through and never takes focus, so the
//! chat you are dictating into keeps the cursor. It runs on its own thread so it keeps
//! animating while the main window is hidden in the tray.

use crate::icon::{gradient, paint, rounded_rect};
use crate::state::{Phase, Shared};
use crate::text::TextRenderer;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tiny_skia::{Color, FillRule, LineCap, PathBuilder, Pixmap, Shader, Stroke, Transform};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, GetDC, ReleaseDC, SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, MA_NOACTIVATE, MSG, PM_REMOVE, PeekMessageW, RegisterClassW,
    SPI_GETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow, SystemParametersInfoW, TranslateMessage, ULW_ALPHA,
    UpdateLayeredWindow, WM_MOUSEACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

const BARS: usize = 26;
// Logical sizes; multiplied by the DPI scale.
const WIN_W: f32 = 620.0;
const WIN_H: f32 = 96.0;
const PILL_H: f32 = 52.0;
const LISTEN_W: f32 = 236.0;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

pub fn spawn(shared: Arc<Shared>) {
    std::thread::Builder::new().name("overlay".into()).spawn(move || run(shared)).unwrap();
}

struct Anim {
    alpha: f32,
    width: f32,
    level: f32,
    levels: VecDeque<f32>,
    start: Instant,
}

fn run(shared: Arc<Shared>) {
    let scale = unsafe { GetDpiForSystem() } as f32 / 96.0;
    let (w, h) = ((WIN_W * scale) as i32, (WIN_H * scale) as i32);
    let text = TextRenderer::load();

    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("teiqoflow-overlay");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);

        let mut work: RECT = std::mem::zeroed();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut _, 0);
        let pos = POINT { x: (work.left + work.right - w) / 2, y: work.bottom - h - (8.0 * scale) as i32 };

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("teiqoflow overlay").as_ptr(),
            WS_POPUP,
            pos.x,
            pos.y,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }

        let screen_dc = GetDC(std::ptr::null_mut());
        let mem_dc = CreateCompatibleDC(screen_dc);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(screen_dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        SelectObject(mem_dc, dib);
        ReleaseDC(std::ptr::null_mut(), screen_dc);

        let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
        let mut anim = Anim { alpha: 0.0, width: LISTEN_W, level: 0.0, levels: VecDeque::from(vec![0.0; BARS]), start: Instant::now() };
        let mut shown = false;
        let mut msg: MSG = std::mem::zeroed();

        loop {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let (phase, since) = shared.phase();
            let visible = match &phase {
                Phase::Idle => false,
                Phase::Listening | Phase::Processing(_) => true,
                Phase::Done(_) => since.elapsed() < Duration::from_millis(1700),
                Phase::Error(_) => since.elapsed() < Duration::from_millis(3200),
            };
            if phase == Phase::Listening && since.elapsed() < Duration::from_millis(30) {
                anim.levels.iter_mut().for_each(|l| *l = 0.0);
            }

            let target = if visible { 1.0 } else { 0.0 };
            anim.alpha += (target - anim.alpha) * 0.22;
            if !visible && anim.alpha < 0.02 {
                anim.alpha = 0.0;
                if shown {
                    ShowWindow(hwnd, SW_HIDE);
                    shown = false;
                }
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }

            let target_w = match &phase {
                Phase::Done(t) | Phase::Error(t) => (text.width(t, 14.0) + 84.0).clamp(220.0, WIN_W - 40.0),
                _ => LISTEN_W,
            };
            anim.width += (target_w - anim.width) * 0.25;
            anim.level += (shared.level() - anim.level) * 0.45;
            if phase == Phase::Listening {
                anim.levels.pop_front();
                anim.levels.push_back(anim.level);
            }

            pm.fill(Color::TRANSPARENT);
            draw(&mut pm, &text, &phase, &anim, scale);

            // tiny-skia: premultiplied RGBA -> GDI: premultiplied BGRA.
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
            for (d, s) in dst.chunks_exact_mut(4).zip(pm.data().chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: (anim.alpha * 255.0) as u8,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let size = SIZE { cx: w, cy: h };
            let src = POINT { x: 0, y: 0 };
            UpdateLayeredWindow(hwnd, std::ptr::null_mut(), &pos, &size, mem_dc, &src, 0, &blend, ULW_ALPHA);
            if !shown {
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                shown = true;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }
}

fn draw(pm: &mut Pixmap, text: &TextRenderer, phase: &Phase, anim: &Anim, s: f32) {
    let t = anim.start.elapsed().as_secs_f32();
    let id = Transform::from_scale(s, s);
    let win_w = pm.width() as f32 / s;
    let w = anim.width;
    let h = PILL_H;
    let x = (win_w - w) / 2.0;
    let y = WIN_H - h - 14.0;

    // Soft shadow.
    for i in 0..6 {
        let g = i as f32 * 2.0;
        if let Some(p) = rounded_rect(x - g, y - g + 4.0, w + 2.0 * g, h + 2.0 * g, h / 2.0 + g) {
            pm.fill_path(&p, &paint(Shader::SolidColor(Color::from_rgba8(0, 0, 0, 14))), FillRule::Winding, id, None);
        }
    }
    let Some(pill) = rounded_rect(x, y, w, h, h / 2.0) else { return };
    pm.fill_path(&pill, &paint(Shader::SolidColor(Color::from_rgba8(16, 16, 22, 240))), FillRule::Winding, id, None);
    let active = matches!(phase, Phase::Listening | Phase::Processing(_));
    let border = if active { gradient(x, y, x + w, y, 0.9) } else { Shader::SolidColor(Color::from_rgba8(0x2E, 0x2E, 0x3D, 255)) };
    pm.stroke_path(&pill, &paint(border), &Stroke { width: 1.3, ..Stroke::default() }, id, None);

    let cy = y + h / 2.0;
    match phase {
        Phase::Listening => {
            let pulse = 0.6 + 0.4 * (t * 6.0).sin();
            circle(pm, x + 27.0, cy, 11.0, Color::from_rgba8(255, 80, 110, (70.0 * pulse) as u8), id);
            circle(pm, x + 27.0, cy, 6.0, Color::from_rgba8(0xFF, 0x5C, 0x70, 255), id);
            let left = x + 50.0;
            let right = x + w - 22.0;
            let step = (right - left) / BARS as f32;
            let bw = 3.6;
            let shader = gradient(left, 0.0, right, 0.0, 1.0);
            let mut pb = PathBuilder::new();
            for (i, &lvl) in anim.levels.iter().enumerate() {
                let idle = 0.08 + 0.05 * (t * 4.0 + i as f32 * 0.6).sin();
                let bh = (lvl.max(idle) * (h - 16.0)).max(4.0);
                let bx = left + i as f32 * step + (step - bw) / 2.0;
                if let Some(r) = tiny_skia::Rect::from_xywh(bx, cy - bh / 2.0, bw, bh) {
                    pb.push_rect(r);
                }
            }
            if let Some(p) = pb.finish() {
                pm.fill_path(&p, &paint(shader), FillRule::Winding, id, None);
            }
        }
        Phase::Processing(label) => {
            text.draw(pm, label, (x + 26.0) * s, cy * s, 14.0 * s, [0xED, 0xED, 0xF3, 255]);
            let colors = [(0x25, 0x63, 0xEB), (0x2E, 0x90, 0xF2), (0x38, 0xBD, 0xF8)];
            for (i, (r, g, b)) in colors.into_iter().enumerate() {
                let a = 0.3 + 0.7 * (t * 6.0 - i as f32 * 0.7).sin().max(0.0);
                circle(pm, x + w - 60.0 + i as f32 * 15.0, cy - 3.0 * a, 4.0, Color::from_rgba8(r, g, b, (a * 255.0) as u8), id);
            }
        }
        Phase::Done(msg) | Phase::Error(msg) => {
            let ok = matches!(phase, Phase::Done(_));
            let c = if ok { Color::from_rgba8(0x3D, 0xDC, 0x97, 255) } else { Color::from_rgba8(0xFF, 0x5C, 0x70, 255) };
            let ix = x + 28.0;
            circle(pm, ix, cy, 10.0, c, id);
            let mut pb = PathBuilder::new();
            if ok {
                pb.move_to(ix - 4.5, cy);
                pb.line_to(ix - 1.0, cy + 3.5);
                pb.line_to(ix + 5.0, cy - 4.0);
            } else {
                pb.move_to(ix - 4.0, cy - 4.0);
                pb.line_to(ix + 4.0, cy + 4.0);
                pb.move_to(ix + 4.0, cy - 4.0);
                pb.line_to(ix - 4.0, cy + 4.0);
            }
            if let Some(p) = pb.finish() {
                let stroke = Stroke { width: 2.2, line_cap: LineCap::Round, ..Stroke::default() };
                pm.stroke_path(&p, &paint(Shader::SolidColor(Color::from_rgba8(11, 11, 16, 255))), &stroke, id, None);
            }
            let shown = text.elide(&msg.to_lowercase(), 14.0, w - 70.0);
            text.draw(pm, &shown, (x + 48.0) * s, cy * s, 14.0 * s, [0xED, 0xED, 0xF3, 255]);
        }
        Phase::Idle => {}
    }
}

fn circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, c: Color, t: Transform) {
    if let Some(p) = PathBuilder::from_circle(cx, cy, r) {
        pm.fill_path(&p, &paint(Shader::SolidColor(c)), FillRule::Winding, t, None);
    }
}
