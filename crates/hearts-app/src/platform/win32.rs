use std::cell::RefCell;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use crate::controller::GameController;
use hearts_core::model::card::Card as ModelCard;
use hearts_core::model::player::PlayerPosition;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
// (No transform imports required; rotation for east/west hands will be handled later.)
use windows::Win32::Graphics::Direct2D::{
    D2D1_FACTORY_OPTIONS, D2D1_FACTORY_TYPE_MULTI_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT, D2D1CreateFactory,
    ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, HBRUSH, PAINTSTRUCT};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmap, IWICBitmapFrameDecode, IWICFormatConverter,
    IWICImagingFactory, IWICStream, WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateAcceleratorTableW, CreateMenu, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, DrawMenuBar, GetClientRect, GetMessageW, LoadCursorW, LoadIconW,
    MessageBoxW, PostQuitMessage, RegisterClassExW, SetMenu, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    TranslateAcceleratorW, TranslateMessage, ACCEL, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, FCONTROL,
    FVIRTKEY, GWLP_USERDATA, HACCEL, HMENU, IDC_ARROW, IDI_APPLICATION, MSG, WNDCLASSEXW,
    WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCCREATE, WM_PAINT, WM_SIZE, WM_TIMER, WS_OVERLAPPEDWINDOW,
    MF_POPUP, MF_SEPARATOR, MF_STRING,
};
use windows::core::{w, PCWSTR, Result};

const D2DERR_RECREATE_TARGET: i32 = 0x8899_000C_u32 as i32;

const VK_F2: u32 = 0x71;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;

const ID_GAME_NEW: u32 = 1001;
const ID_GAME_RESTART: u32 = 1002;
const ID_GAME_EXIT: u32 = 1003;
const ID_HELP_ABOUT: u32 = 1301;

pub fn run() -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?; }

    let module = unsafe { GetModuleHandleW(None)? };
    let hinstance = HINSTANCE(module.0);
    let class_name = w!("MDHEARTS_WIN32");

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: std::mem::size_of::<*const RefCell<AppState>>() as i32,
        hInstance: hinstance,
        hIcon: unsafe { LoadIconW(None, IDI_APPLICATION)? },
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        hbrBackground: HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: class_name,
        hIconSm: Default::default(),
    };
    unsafe { RegisterClassExW(&wc); }

    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            class_name,
            w!("mdhearts"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1024,
            768,
            None,
            None,
            Some(hinstance),
            None,
        )
    }?;

    let haccel = init_menu_and_accels(hwnd);
    unsafe { let _ = ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_SHOW); }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if TranslateAcceleratorW(hwnd, haccel, &mut msg) != 0 { continue; }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

struct AppState {
    factory: ID2D1Factory,
    text_format: IDWriteTextFormat,
    render_target: Option<ID2D1HwndRenderTarget>,
    controller: GameController,
    passing_select: Vec<ModelCard>,
    wic: IWICImagingFactory,
    cards_bitmap: Option<ID2D1Bitmap>,
    atlas: AtlasMeta,
    card_back_bitmap: Option<ID2D1Bitmap>,
    card_back_bitmap_rot90: Option<ID2D1Bitmap>,
    anim: Option<PlayAnim>,
    collect: Option<CollectAnim>,
    rotate_sides: bool,
}

struct PlayAnim {
    seat: PlayerPosition,
    card: ModelCard,
    from: D2D_RECT_F,
    to: D2D_RECT_F,
    start: std::time::Instant,
    dur_ms: u64,
}

struct CollectAnim {
    winner: PlayerPosition,
    cards: Vec<(PlayerPosition, ModelCard)>,
    start: std::time::Instant,
    delay_ms: u64,
    dur_ms: u64,
}

impl AppState {
    fn new() -> Result<Self> {
        let factory: ID2D1Factory = unsafe {
            D2D1CreateFactory::<ID2D1Factory>(
                D2D1_FACTORY_TYPE_MULTI_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?
        };
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)? };
        let text_format: IDWriteTextFormat = unsafe {
            let format = dwrite.CreateTextFormat(
                w!("Segoe UI"), None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                18.0,
                w!("en-us"),
            )?;
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format
        };
        let controller = GameController::new_with_seed(Some(0), PlayerPosition::North);
        let wic: IWICImagingFactory = unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
        let atlas = AtlasMeta::load_from_assets().unwrap_or_default();
        // Default ON now that rotation is fast; disable with MDH_ROTATE_SIDES=0 if desired
        let rotate_sides = std::env::var("MDH_ROTATE_SIDES").map(|v| v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true")).unwrap_or(true);
        debug_out("mdhearts: ", &format!("rotate_sides={} (env MDH_ROTATE_SIDES)", rotate_sides));
        Ok(Self { factory, text_format, render_target: None, controller, passing_select: Vec::new(), wic, cards_bitmap: None, atlas, card_back_bitmap: None, card_back_bitmap_rot90: None, anim: None, collect: None, rotate_sides })
    }

    fn ensure_render_target(&mut self, hwnd: HWND) -> Result<()> {
        if self.render_target.is_none() {
            let rect = client_rect(hwnd);
            if rect.right <= rect.left || rect.bottom <= rect.top { return Ok(()); }
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U { width: (rect.right - rect.left) as u32, height: (rect.bottom - rect.top) as u32 },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let rt: ID2D1HwndRenderTarget = unsafe {
                self.factory.CreateHwndRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES::default(), &hwnd_props)?
            };
            self.render_target = Some(rt);
        }
        Ok(())
    }

    fn resize(&mut self, hwnd: HWND) -> Result<()> {
        if let Some(rt) = self.render_target.as_ref() {
            let rect = client_rect(hwnd);
            let width = if rect.right > rect.left { (rect.right - rect.left) as u32 } else { 0 };
            let height = if rect.bottom > rect.top { (rect.bottom - rect.top) as u32 } else { 0 };
            unsafe {
                match rt.Resize(&D2D_SIZE_U { width, height }) {
                    Ok(_) => {}
                    Err(err) => { if err.code().0 == D2DERR_RECREATE_TARGET { self.render_target = None; } else { return Err(err); } }
                }
            }
        }
        Ok(())
    }

    fn ensure_cards_bitmap(&mut self, rt: &ID2D1HwndRenderTarget) -> Result<()> {
        if self.cards_bitmap.is_some() { return Ok(()); }
        // Load faces atlas from assets/cards.png
        let stream: IWICStream = unsafe { self.wic.CreateStream()? };
        let bytes = std::fs::read("assets/cards.png").map_err(|_| windows::core::Error::from(windows::core::HRESULT(0x80004005u32 as i32)))?;
        unsafe {
            stream.InitializeFromMemory(bytes.as_slice())?;
            let decoder = self.wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)?;
            let frame: IWICBitmapFrameDecode = decoder.GetFrame(0)?;
            let converter: IWICFormatConverter = self.wic.CreateFormatConverter()?;
            converter.Initialize(&frame, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            let bmp = rt.CreateBitmapFromWicBitmap(&converter, None)?;
            self.cards_bitmap = Some(bmp);
        }
        Ok(())
    }

    fn ensure_card_back_bitmap(&mut self, rt: &ID2D1HwndRenderTarget) -> Result<()> {
        if self.card_back_bitmap.is_some() { return Ok(()); }
        debug_out("mdhearts: ", "ensure_card_back_bitmap begin");
        const BACK_PNG: &[u8] = include_bytes!("../../../../assets/card_back.png");
        let stream: IWICStream = unsafe { self.wic.CreateStream()? };
        unsafe {
            stream.InitializeFromMemory(BACK_PNG)?;
            let decoder = self.wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)?;
            let frame: IWICBitmapFrameDecode = decoder.GetFrame(0)?;
            let converter: IWICFormatConverter = self.wic.CreateFormatConverter()?;
            converter.Initialize(&frame, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            let bmp = rt.CreateBitmapFromWicBitmap(&converter, None)?;
            self.card_back_bitmap = Some(bmp);
        }
        debug_out("mdhearts: ", "ensure_card_back_bitmap ok");
        Ok(())
    }

    fn ensure_card_back_bitmap_rot90(&mut self, rt: &ID2D1HwndRenderTarget) -> Result<()> {
        if self.card_back_bitmap_rot90.is_some() { return Ok(()); }
        debug_out("mdhearts: ", "ensure_card_back_bitmap_rot90 begin (CPU rotate)");
        const BACK_PNG: &[u8] = include_bytes!("../../../../assets/card_back.png");
        let stream: IWICStream = unsafe { self.wic.CreateStream()? };
        unsafe {
            // Decode to 32bpp premultiplied BGRA
            stream.InitializeFromMemory(BACK_PNG)?;
            let decoder = self.wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)?;
            let frame: IWICBitmapFrameDecode = decoder.GetFrame(0)?;
            let converter: IWICFormatConverter = self.wic.CreateFormatConverter()?;
            converter.Initialize(&frame, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            let mut w: u32 = 0; let mut h: u32 = 0; converter.GetSize(&mut w, &mut h)?;
            if w == 0 || h == 0 { return Ok(()); }
            let src_stride = (w as usize) * 4;
            let mut src = vec![0u8; src_stride * (h as usize)];
            converter.CopyPixels(std::ptr::null(), src_stride as u32, &mut src)?;

            // Rotate 90 degrees clockwise into dst (h x w)
            let rw = h as usize; let rh = w as usize; // rotated width/height
            let dst_stride = rw * 4;
            let mut dst = vec![0u8; dst_stride * rh];
            for y in 0..(h as usize) {
                for x in 0..(w as usize) {
                    let src_idx = y * src_stride + x * 4;
                    let nx = y; // new x
                    let ny = (w as usize) - 1 - x; // new y
                    let dst_idx = ny * dst_stride + nx * 4;
                    dst[dst_idx..dst_idx+4].copy_from_slice(&src[src_idx..src_idx+4]);
                }
            }

            // Wrap rotated memory as an IWICBitmap, then hand to D2D
            let rot_wic: IWICBitmap = self.wic.CreateBitmapFromMemory(
                rw as u32, rh as u32,
                &GUID_WICPixelFormat32bppPBGRA,
                dst_stride as u32,
                &dst,
            )?;
            let bmp = rt.CreateBitmapFromWicBitmap(&rot_wic, None)?;
            self.card_back_bitmap_rot90 = Some(bmp);
        }
        debug_out("mdhearts: ", "ensure_card_back_bitmap_rot90 ok (CPU rotate)");
        Ok(())
    }

    fn draw(&mut self, hwnd: HWND) -> Result<()> {
        self.ensure_render_target(hwnd)?;
        let Some(rt) = self.render_target.as_ref().cloned() else { return Ok(()); };
        let size = client_size(hwnd);
        if size.width == 0 || size.height == 0 { return Ok(()); }

        debug_out("mdhearts: ", "draw: begin");
        let status = self.controller.status_text().replace("Ã¢â‚¬Â¢", "â€¢");
        // Touch methods to avoid dead_code warnings until used in UI
        let _ = self.controller.standings();
        let _ = self.controller.trick_leader();
        let _ = self.controller.trick_plays();
        // Title bar update
        let title = string_to_wide_z(&format!("MD Hearts - {}", status));
        unsafe { let _ = SetWindowTextW(hwnd, PCWSTR(title.as_ptr())); }

        let south_hand = self.controller.hand(PlayerPosition::South);
        let south_legal_set = self.controller.legal_moves_set(PlayerPosition::South);
        let south_labels: Vec<Vec<u16>> = south_hand.iter().map(|&c| card_label_wide(c)).collect();
        let south_legal: Vec<bool> = south_hand.iter().map(|c| south_legal_set.contains(c)).collect();

        unsafe {
            rt.BeginDraw();
            // Background
            rt.Clear(Some(&D2D1_COLOR_F { r: 0.05, g: 0.15, b: 0.10, a: 1.0 }));
            let table = D2D_RECT_F { left: size.width as f32 * 0.08, top: size.height as f32 * 0.12, right: size.width as f32 * 0.92, bottom: size.height as f32 * 0.88 };
            let felt = rt.CreateSolidColorBrush(&D2D1_COLOR_F { r: 0.06, g: 0.22, b: 0.12, a: 1.0 }, None)?;
            let border = rt.CreateSolidColorBrush(&D2D1_COLOR_F { r: 0.2, g: 0.5, b: 0.3, a: 1.0 }, None)?;
            let text_brush = rt.CreateSolidColorBrush(&D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 0.95 }, None)?;
            rt.FillRectangle(&table, &felt);
            rt.DrawRectangle(&table, &border, 2.0, None);

            // South hand faces
            debug_out("mdhearts: ", "draw: ensure_cards_bitmap");
            self.ensure_cards_bitmap(&rt)?;
            let atlas_bmp_opt = self.cards_bitmap.clone();
            let rects = compute_south_hand_rects(size, south_labels.len());
            for (i, rect) in rects.iter().enumerate() {
                let rounded = D2D1_ROUNDED_RECT { rect: *rect, radiusX: 6.0, radiusY: 6.0 };
                let mut drew_face = false;
                if let Some(ref bmp) = atlas_bmp_opt {
                    if let Some(card) = south_hand.get(i).copied() {
                        if let Some(src) = self.atlas.src_rect_for(card) {
                            rt.DrawBitmap(bmp, Some(rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, Some(&src));
                            drew_face = true;
                        }
                    }
                }
                if !drew_face {
                    let placeholder = rt.CreateSolidColorBrush(&D2D1_COLOR_F { r: 0.93, g: 0.94, b: 0.98, a: 0.6 }, None)?;
                    rt.FillRoundedRectangle(&rounded, &placeholder);
                }
                let selected = self.passing_select.iter().any(|c| Some(c) == south_hand.get(i));
                let legal = *south_legal.get(i).unwrap_or(&false);
                let border_brush = if selected || legal { &border } else { &text_brush };
                rt.DrawRoundedRectangle(&rounded, border_brush, 2.0, None);
            }

            // Opponents' backs
            debug_out("mdhearts: ", "draw: ensure_card_back_bitmap");
            self.ensure_card_back_bitmap(&rt)?;
            let back_rot = if self.rotate_sides {
                debug_out("mdhearts: ", "draw: ensure_card_back_bitmap_rot90");
                if let Err(_) = self.ensure_card_back_bitmap_rot90(&rt) { debug_out("mdhearts: ", "rotated back load failed; using unrotated backs"); }
                // prefer rotated if present, else fall back
                self.card_back_bitmap_rot90.as_ref().or(self.card_back_bitmap.as_ref())
            } else {
                self.card_back_bitmap.as_ref()
            };
            let back = self.card_back_bitmap.as_ref(); // unrotated for North
            let north_count = self.controller.hand(PlayerPosition::North).len();
            let east_count = self.controller.hand(PlayerPosition::East).len();
            let west_count = self.controller.hand(PlayerPosition::West).len();
            let back_fallback = rt.CreateSolidColorBrush(&D2D1_COLOR_F { r: 0.15, g: 0.12, b: 0.25, a: 1.0 }, None)?;
            for rect in compute_north_hand_rects(size, north_count) {
                let rounded = D2D1_ROUNDED_RECT { rect, radiusX: 6.0, radiusY: 6.0 };
                if let Some(bmp) = back { rt.DrawBitmap(bmp, Some(&rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None); }
                else { rt.FillRoundedRectangle(&rounded, &back_fallback); rt.DrawRoundedRectangle(&rounded, &border, 1.5, None); }
            }
            for rect in compute_east_hand_rects(size, east_count) {
                let rounded = D2D1_ROUNDED_RECT { rect, radiusX: 6.0, radiusY: 6.0 };
                if let Some(bmp) = back_rot { rt.DrawBitmap(bmp, Some(&rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None); }
                else { rt.FillRoundedRectangle(&rounded, &back_fallback); rt.DrawRoundedRectangle(&rounded, &border, 1.5, None); }
            }
            for rect in compute_west_hand_rects(size, west_count) {
                let rounded = D2D1_ROUNDED_RECT { rect, radiusX: 6.0, radiusY: 6.0 };
                if let Some(bmp) = back_rot { rt.DrawBitmap(bmp, Some(&rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None); }
                else { rt.FillRoundedRectangle(&rounded, &back_fallback); rt.DrawRoundedRectangle(&rounded, &border, 1.5, None); }
            }

            // Current trick (center cards face-up). If the trick just completed
            // and the last-card animation is still running (collect not yet started),
            // draw the prior trick's cards so they don't pop away.
            if let Some(ref bmp) = atlas_bmp_opt {
                let mut plays = self.controller.trick_plays();
                if plays.is_empty() && self.collect.is_none() {
                    if let Some(summary) = self.controller.last_trick() {
                        plays = summary.plays.clone();
                    }
                }
                let leading_so_far = current_trick_leader_so_far(&plays);
                for (pos, card) in plays {
                    if let Some(anim) = &self.anim { if anim.seat == pos { continue; } }
                    if let Some(src) = self.atlas.src_rect_for(card) {
                        let rect = compute_trick_rect_for(size, pos);
                        let rounded = D2D1_ROUNDED_RECT { rect, radiusX: 6.0, radiusY: 6.0 };
                        rt.DrawBitmap(bmp, Some(&rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, Some(&src));
                        let hl = if Some(pos) == leading_so_far { &text_brush } else { &border };
                        rt.DrawRoundedRectangle(&rounded, hl, 2.0, None);
                    }
                }
            }

            // Active animation overlay (draw last so it is visible)
            if let (Some(bmp), Some(anim)) = (atlas_bmp_opt.as_ref(), &self.anim) {
                let t = (std::time::Instant::now() - anim.start).as_millis() as u64;
                let u = (t as f32 / anim.dur_ms as f32).clamp(0.0, 1.0);
                let rect = lerp_rect(anim.from, anim.to, ease_out(u));
                if let Some(src) = self.atlas.src_rect_for(anim.card) {
                    let rounded = D2D1_ROUNDED_RECT { rect, radiusX: 6.0, radiusY: 6.0 };
                    rt.DrawBitmap(bmp, Some(&rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, Some(&src));
                    rt.DrawRoundedRectangle(&rounded, &border, 2.0, None);
                }
                if t >= anim.dur_ms { self.anim = None; }
            }

            // Trick collect overlay: pause, then sweep cards to winner
            if let (Some(bmp), Some(coll)) = (atlas_bmp_opt.as_ref(), &self.collect) {
                let elapsed = (std::time::Instant::now() - coll.start).as_millis() as u64;
                let moving = elapsed.saturating_sub(coll.delay_ms);
                let u = (moving as f32 / coll.dur_ms as f32).clamp(0.0, 1.0);
                let to_rect = compute_collect_target_rect_for(size, &self.controller, coll.winner);
                for (seat, card) in &coll.cards {
                    if let Some(src) = self.atlas.src_rect_for(*card) {
                        let from_rect = compute_trick_rect_for(size, *seat);
                        let rect = if elapsed < coll.delay_ms { from_rect } else { lerp_rect(from_rect, to_rect, ease_out(u)) };
                        let rounded = D2D1_ROUNDED_RECT { rect, radiusX: 6.0, radiusY: 6.0 };
                        rt.DrawBitmap(bmp, Some(&rect), 1.0, windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, Some(&src));
                        rt.DrawRoundedRectangle(&rounded, &border, 2.0, None);
                    }
                }
                if elapsed >= coll.delay_ms + coll.dur_ms { self.collect = None; }
            }

            // Status text
            let status_rect = D2D_RECT_F { left: 0.0, top: size.height as f32 * 0.02, right: size.width as f32, bottom: size.height as f32 * 0.08 };
            let status_wide = string_to_wide(&status);
            rt.DrawText(status_wide.as_slice(), &self.text_format, &status_rect, &text_brush, Default::default(), DWRITE_MEASURING_MODE::default());

            // On-screen hint for current action
            let hint = if self.collect.is_some() {
                "Collecting trickâ€¦".to_string()
            } else if self.controller.in_passing_phase() {
                format!("Passing: select 3 cards ({} selected) and press Enter", self.passing_select.len())
            } else {
                let turn = self.controller.expected_to_play();
                if turn == PlayerPosition::South {
                    "Your turn: click a highlighted card".to_string()
                } else {
                    let who = match turn { PlayerPosition::North => "North", PlayerPosition::East => "East", PlayerPosition::South => "South", PlayerPosition::West => "West" };
                    format!("Waiting for {}…", who)
                }
            };
            let hint_rect = D2D_RECT_F { left: 0.0, top: size.height as f32 * 0.86, right: size.width as f32, bottom: size.height as f32 * 0.96 };
            let hint_wide = string_to_wide(&hint);
            rt.DrawText(hint_wide.as_slice(), &self.text_format, &hint_rect, &text_brush, Default::default(), DWRITE_MEASURING_MODE::default());

            if let Err(err) = rt.EndDraw(None, None) {
                if err.code().0 == D2DERR_RECREATE_TARGET { self.render_target = None; } else { return Err(err); }
            }
        }
        debug_out("mdhearts: ", "draw: end");
        Ok(())
    }
}

fn init_menu_and_accels(hwnd: HWND) -> HACCEL {
    let hmenu: HMENU = unsafe { CreateMenu().expect("menu") };
    let game = unsafe { CreatePopupMenu().expect("game") };
    let _ = unsafe { AppendMenuW(game, MF_STRING, ID_GAME_NEW as usize, w!("&New Game\tCtrl+N")) };
    let _ = unsafe { AppendMenuW(game, MF_STRING, ID_GAME_RESTART as usize, w!("&Restart Round\tF5")) };
    let _ = unsafe { AppendMenuW(game, MF_SEPARATOR, 0, None) };
    let _ = unsafe { AppendMenuW(game, MF_STRING, ID_GAME_EXIT as usize, w!("E&xit")) };
    let _ = unsafe { AppendMenuW(hmenu, MF_POPUP, game.0 as usize, w!("&Game")) };

    let help = unsafe { CreatePopupMenu().expect("help") };
    let _ = unsafe { AppendMenuW(help, MF_STRING, ID_HELP_ABOUT as usize, w!("&About MD Hearts...")) };
    let _ = unsafe { AppendMenuW(hmenu, MF_POPUP, help.0 as usize, w!("&Help")) };
    let _ = unsafe { SetMenu(hwnd, Some(hmenu)) };
    let _ = unsafe { DrawMenuBar(hwnd) };

    let accels = [
        ACCEL { fVirt: FVIRTKEY | FCONTROL, key: b'N' as u16, cmd: ID_GAME_NEW as u16 },
        ACCEL { fVirt: FVIRTKEY, key: 0x74, cmd: ID_GAME_RESTART as u16 },
    ];
    unsafe { CreateAcceleratorTableW(&accels).expect("accel") }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => match AppState::new() {
            Ok(state) => {
                debug_out("mdhearts: ", "WM_NCCREATE -> AppState::new OK");
                let boxed = Box::new(RefCell::new(state));
                let ptr = Box::into_raw(boxed);
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize); }
                unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(Some(hwnd), 1, 16, None); }
                LRESULT(1)
            }
            Err(_) => { debug_out("mdhearts: ", "WM_NCCREATE -> AppState::new ERR"); LRESULT(0) },
        },
        WM_PAINT => {
            debug_out("mdhearts: ", "WM_PAINT begin");
            let mut ps = PAINTSTRUCT::default();
            let _ = unsafe { BeginPaint(hwnd, &mut ps) };
            if let Some(cell) = state_cell(hwnd) { let _ = cell.borrow_mut().draw(hwnd); }
            let _ = unsafe { EndPaint(hwnd, &ps) };
            debug_out("mdhearts: ", "WM_PAINT end");
            LRESULT(0)
        }
        WM_SIZE => { if let Some(cell) = state_cell(hwnd) { let _ = cell.borrow_mut().resize(hwnd); } LRESULT(0) }
        WM_KEYDOWN => {
            if let Some(cell) = state_cell(hwnd) {
                let mut state = cell.borrow_mut();
                let key = wparam.0 as u32;
                if key == VK_F2 {
                    state.controller = GameController::new_with_seed(None, PlayerPosition::North);
                    unsafe { let _ = InvalidateRect(Some(hwnd), None, true); }
                } else if key == VK_ESCAPE {
                    if !state.passing_select.is_empty() { state.passing_select.clear(); unsafe { let _ = InvalidateRect(Some(hwnd), None, true); } }
                } else if key == VK_RETURN {
                    if state.controller.in_passing_phase() && state.passing_select.len() == 3 {
                        let cards = [state.passing_select[0], state.passing_select[1], state.passing_select[2]];
                        debug_out("mdhearts: ", &format!("Submitting pass: {}, {}, {}", cards[0], cards[1], cards[2]));
                        if state.controller.submit_pass(PlayerPosition::South, cards).is_ok() {
                            let _ = state.controller.submit_auto_passes_for_others(PlayerPosition::South);
                            let _ = state.controller.resolve_passes();
                            state.passing_select.clear();
                            debug_out("mdhearts: ", &format!("Pass resolved. Leader now {:?}", state.controller.trick_leader()));
                            unsafe { let _ = InvalidateRect(Some(hwnd), None, true); }
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            if let Some(cell) = state_cell(hwnd) {
                let mut state = cell.borrow_mut();
                if state.collect.is_some() || state.anim.is_some() { return LRESULT(0); }
                let size = client_size(hwnd);
                let south_hand = state.controller.hand(PlayerPosition::South);
                let rects = compute_south_hand_rects(size, south_hand.len());
                let xf = x as f32; let yf = y as f32;
                for (i, r) in rects.iter().enumerate() {
                    if xf >= r.left && xf <= r.right && yf >= r.top && yf <= r.bottom {
                        if state.controller.in_passing_phase() {
                            if let Some(card) = south_hand.get(i).copied() {
                                if let Some(pos) = state.passing_select.iter().position(|c| *c == card) { state.passing_select.remove(pos); }
                                else if state.passing_select.len() < 3 { state.passing_select.push(card); }
                                unsafe { let _ = InvalidateRect(Some(hwnd), None, true); }
                            }
                        } else {
                            let legal = state.controller.legal_moves_set(PlayerPosition::South);
                            if let Some(card) = south_hand.get(i).copied() {
                                if legal.contains(&card) {
                                    debug_out("mdhearts: ", &format!("South plays {}", card));
                                    let from = *r;
                                    let to = compute_trick_rect_for(size, PlayerPosition::South);
                                    let _ = state.controller.play(PlayerPosition::South, card);
                                    state.anim = Some(PlayAnim { seat: PlayerPosition::South, card, from, to, start: std::time::Instant::now(), dur_ms: 260 });
                                    unsafe { let _ = InvalidateRect(Some(hwnd), None, true); }
                                }
                            }
                        }
                        break;
                    }
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if let Some(cell) = state_cell(hwnd) {
                let mut state = cell.borrow_mut();
                debug_out("mdhearts: ", &format!("TICK anim={} collect={} passing={}", state.anim.is_some(), state.collect.is_some(), state.controller.in_passing_phase()));
                // First, if a trick just completed and no animation is running, start the collect
                if state.anim.is_none() && state.collect.is_none() {
                    if let Some(summary) = state.controller.take_last_trick_summary() {
                        debug_out("mdhearts: ", &format!("Collect start winner {:?}", summary.winner));
                        state.collect = Some(CollectAnim { winner: summary.winner, cards: summary.plays, start: std::time::Instant::now(), delay_ms: 350, dur_ms: 320 });
                    }
                }
                // Only advance AI if not passing, no play animation, and not collecting
                if state.anim.is_none() && state.collect.is_none() && !state.controller.in_passing_phase() {
                    let turn = state.controller.expected_to_play();
                    if turn != PlayerPosition::South {
                        if let Some((seat, card)) = state.controller.autoplay_one(PlayerPosition::South) {
                            let size = client_size(hwnd);
                            let from = approx_from_rect_for_seat(size, &state.controller, seat);
                            let to = compute_trick_rect_for(size, seat);
                            state.anim = Some(PlayAnim { seat, card, from, to, start: std::time::Instant::now(), dur_ms: 260 });
                        }
                    }
                }
                // After collect completes, finish the round if we just played the 13th trick
                if state.anim.is_none() && state.collect.is_none() {
                    debug_out("mdhearts: ", "Collect end / ready check");
                    state.controller.finish_round_if_ready();
                }
                unsafe { let _ = InvalidateRect(Some(hwnd), None, true); }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as u32;
            if let Some(cell) = state_cell(hwnd) {
                let mut state = cell.borrow_mut();
                match id {
                    ID_GAME_NEW => { 
                        state.controller = GameController::new_with_seed(None, PlayerPosition::North); 
                        state.passing_select.clear(); 
                        unsafe { let _ = InvalidateRect(Some(hwnd), None, true); } 
                    }
                    ID_GAME_RESTART => { 
                        state.controller.restart_round(); 
                        unsafe { let _ = InvalidateRect(Some(hwnd), None, true); } 
                    }
                    ID_GAME_EXIT => unsafe { PostQuitMessage(0) },
                    ID_HELP_ABOUT => {
                        let title = string_to_wide_z("About MD Hearts");
                        let text = string_to_wide_z(&format!("{} - {}", hearts_core::AppInfo::name(), hearts_core::AppInfo::version()));
                        unsafe { let _ = MessageBoxW(Some(hwnd), PCWSTR(text.as_ptr()), PCWSTR(title.as_ptr()), windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE(0)); }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => { unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), 1); PostQuitMessage(0) }; LRESULT(0) }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn state_ptr(hwnd: HWND) -> Option<*mut RefCell<AppState>> {
    let ptr = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if ptr == 0 { None } else { Some(ptr as *mut RefCell<AppState>) }
}
fn state_cell(hwnd: HWND) -> Option<&'static RefCell<AppState>> { state_ptr(hwnd).map(|p| unsafe { &*p }) }

fn string_to_wide(value: &str) -> Vec<u16> { OsStr::new(value).encode_wide().collect() }
fn string_to_wide_z(value: &str) -> Vec<u16> { let mut v: Vec<u16> = OsStr::new(value).encode_wide().collect(); v.push(0); v }

fn debug_out(prefix: &str, msg: &str) {
    let full = format!("{}{}\n", prefix, msg);
    let mut w = string_to_wide(&full);
    w.push(0);
    unsafe { OutputDebugStringW(PCWSTR(w.as_ptr())); }
}

// Helper: short label for a card (e.g., "10H", "QS") as UTF-16
fn card_label_wide(card: ModelCard) -> Vec<u16> {
    let rank = card.rank.to_string();
    let suit = card.suit.to_string();
    string_to_wide(&format!("{}{}", rank, suit))
}

fn client_rect(hwnd: HWND) -> RECT { unsafe { let mut rect = RECT::default(); let _ = GetClientRect(hwnd, &mut rect); rect } }
fn client_size(hwnd: HWND) -> D2D_SIZE_U {
    let rect = client_rect(hwnd);
    let width = if rect.right > rect.left { (rect.right - rect.left) as u32 } else { 0 };
    let height = if rect.bottom > rect.top { (rect.bottom - rect.top) as u32 } else { 0 };
    D2D_SIZE_U { width, height }
}

fn compute_south_hand_rects(size: D2D_SIZE_U, count: usize) -> Vec<D2D_RECT_F> {
    let width = size.width as f32;
    let height = size.height as f32;
    let min_edge = width.min(height);
    let card_w = (min_edge * 0.14).clamp(80.0, 180.0);
    let card_h = card_w * 1.4;
    let spacing = card_w * 0.5;
    let total_w = card_w + spacing * (count.saturating_sub(1) as f32);
    let left_start = (width - total_w) * 0.5;
    let top = height - card_h - (height * 0.06);
    (0..count).map(|i| { let left = left_start + (i as f32) * spacing; D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h } }).collect()
}

fn compute_north_hand_rects(size: D2D_SIZE_U, count: usize) -> Vec<D2D_RECT_F> {
    let width = size.width as f32;
    let height = size.height as f32;
    let min_edge = width.min(height);
    // AI backs smaller than player hand
    let card_w = (min_edge * 0.10).clamp(60.0, 120.0);
    let card_h = card_w * 1.4;
    let spacing = card_w * 0.40;
    let total_w = card_w + spacing * (count.saturating_sub(1) as f32);
    let left_start = (width - total_w) * 0.5;
    // Nudge down to clear status text at the top
    let top = height * 0.10;
    (0..count).map(|i| { let left = left_start + (i as f32) * spacing; D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h } }).collect()
}

fn compute_east_hand_rects(size: D2D_SIZE_U, count: usize) -> Vec<D2D_RECT_F> {
    let width = size.width as f32;
    let height = size.height as f32;
    let min_edge = width.min(height);
    // AI backs smaller than player hand
    let card_w = (min_edge * 0.10).clamp(60.0, 120.0);
    let card_h = card_w * 1.4;
    // After rotation, visual vertical height becomes card_w, so space by card_w
    let spacing = card_w * 0.40;
    let total_h = card_w + spacing * (count.saturating_sub(1) as f32);
    let top_start = (height - total_h) * 0.5;
    // Position so that the rotated width (card_h) stays inside a side margin
    let margin = width * 0.06;
    let cx = width - margin - card_h * 0.5;
    let left = cx - card_w * 0.5;
    (0..count).map(|i| { let top = top_start + (i as f32) * spacing; D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h } }).collect()
}

fn compute_west_hand_rects(size: D2D_SIZE_U, count: usize) -> Vec<D2D_RECT_F> {
    let width = size.width as f32;
    let height = size.height as f32;
    let min_edge = width.min(height);
    // AI backs smaller than player hand
    let card_w = (min_edge * 0.10).clamp(60.0, 120.0);
    let card_h = card_w * 1.4;
    let spacing = card_w * 0.40;
    let total_h = card_w + spacing * (count.saturating_sub(1) as f32);
    let top_start = (height - total_h) * 0.5;
    let margin = width * 0.06;
    let cx = margin + card_h * 0.5;
    let left = cx - card_w * 0.5;
    (0..count).map(|i| { let top = top_start + (i as f32) * spacing; D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h } }).collect()
}

fn compute_trick_rect_for(size: D2D_SIZE_U, seat: PlayerPosition) -> D2D_RECT_F {
    let width = size.width as f32;
    let height = size.height as f32;
    let min_edge = width.min(height);
    let card_w = (min_edge * 0.14).clamp(90.0, 200.0);
    let card_h = card_w * 1.4;
    let cx = width * 0.5;
    let cy = height * 0.5;
    match seat {
        PlayerPosition::North => {
            let left = cx - card_w * 0.5;
            let top = cy - card_h * 0.95;
            D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h }
        }
        PlayerPosition::South => {
            let left = cx - card_w * 0.5;
            let top = cy + card_h * 0.05;
            D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h }
        }
        PlayerPosition::East => {
            let left = cx + card_w * 0.55;
            let top = cy - card_h * 0.5;
            D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h }
        }
        PlayerPosition::West => {
            let left = cx - card_w * 1.55;
            let top = cy - card_h * 0.5;
            D2D_RECT_F { left, top, right: left + card_w, bottom: top + card_h }
        }
    }
}

fn approx_from_rect_for_seat(size: D2D_SIZE_U, controller: &GameController, seat: PlayerPosition) -> D2D_RECT_F {
    match seat {
        PlayerPosition::North => {
            let count = controller.hand(PlayerPosition::North).len() + 1;
            compute_north_hand_rects(size, count).last().copied().unwrap_or(compute_trick_rect_for(size, seat))
        }
        PlayerPosition::East => {
            let count = controller.hand(PlayerPosition::East).len() + 1;
            compute_east_hand_rects(size, count).last().copied().unwrap_or(compute_trick_rect_for(size, seat))
        }
        PlayerPosition::West => {
            let count = controller.hand(PlayerPosition::West).len() + 1;
            compute_west_hand_rects(size, count).last().copied().unwrap_or(compute_trick_rect_for(size, seat))
        }
        PlayerPosition::South => compute_trick_rect_for(size, seat),
    }
}

fn compute_collect_target_rect_for(
    size: D2D_SIZE_U,
    controller: &GameController,
    seat: PlayerPosition,
) -> D2D_RECT_F {
    match seat {
        PlayerPosition::South => {
            let count = controller.hand(PlayerPosition::South).len().max(1);
            let rects = compute_south_hand_rects(size, count);
            rects.get(count / 2).copied().unwrap_or_else(|| compute_trick_rect_for(size, seat))
        }
        PlayerPosition::North => {
            let count = controller.hand(PlayerPosition::North).len().max(1);
            let rects = compute_north_hand_rects(size, count);
            rects.get(count / 2).copied().unwrap_or_else(|| compute_trick_rect_for(size, seat))
        }
        PlayerPosition::East => {
            let count = controller.hand(PlayerPosition::East).len().max(1);
            let rects = compute_east_hand_rects(size, count);
            rects.get(count / 2).copied().unwrap_or_else(|| compute_trick_rect_for(size, seat))
        }
        PlayerPosition::West => {
            let count = controller.hand(PlayerPosition::West).len().max(1);
            let rects = compute_west_hand_rects(size, count);
            rects.get(count / 2).copied().unwrap_or_else(|| compute_trick_rect_for(size, seat))
        }
    }
}

fn current_trick_leader_so_far(plays: &[(PlayerPosition, ModelCard)]) -> Option<PlayerPosition> {
    if plays.is_empty() { return None; }
    let lead_suit = plays.first().map(|(_, c)| c.suit)?;
    plays
        .iter()
        .filter(|(_, c)| c.suit == lead_suit)
        .max_by(|a, b| a.1.rank.cmp(&b.1.rank))
        .map(|(p, _)| *p)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
fn lerp_rect(a: D2D_RECT_F, b: D2D_RECT_F, t: f32) -> D2D_RECT_F {
    D2D_RECT_F { left: lerp(a.left, b.left, t), top: lerp(a.top, b.top, t), right: lerp(a.right, b.right, t), bottom: lerp(a.bottom, b.bottom, t) }
}
fn ease_out(t: f32) -> f32 { 1.0 - (1.0 - t) * (1.0 - t) }

#[derive(Default, serde::Deserialize, Clone, Debug)]
struct AtlasMeta {
    cols: u32,
    rows: u32,
    card_w: u32,
    card_h: u32,
    order: Vec<String>,
}

impl AtlasMeta {
    fn load_from_assets() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string("assets/cards.json")?;
        Ok(serde_json::from_str(&text)?)
    }
    fn suit_row_index(&self, suit: hearts_core::model::suit::Suit) -> Option<u32> {
        let key = match suit {
            hearts_core::model::suit::Suit::Spades => "spades",
            hearts_core::model::suit::Suit::Hearts => "hearts",
            hearts_core::model::suit::Suit::Diamonds => "diamonds",
            hearts_core::model::suit::Suit::Clubs => "clubs",
        };
        self.order.iter().position(|s| s.eq_ignore_ascii_case(key)).map(|i| i as u32)
    }
    fn rank_col_index(&self, rank: hearts_core::model::rank::Rank) -> Option<u32> {
        hearts_core::model::rank::Rank::ORDERED.iter().position(|r| *r == rank).map(|i| i as u32)
    }
    fn src_rect_for(&self, card: ModelCard) -> Option<D2D_RECT_F> {
        // Touch cols/rows to avoid dead_code lint until needed elsewhere
        let _ = self.cols;
        let _ = self.rows;
        let row = self.suit_row_index(card.suit)?;
        let col = self.rank_col_index(card.rank)?;
        let left = (col * self.card_w) as f32;
        let top = (row * self.card_h) as f32;
        Some(D2D_RECT_F { left, top, right: left + self.card_w as f32, bottom: top + self.card_h as f32 })
    }
}
