//! `sound service`: a tray icon (`rune::tray`, StatusNotifierItem on Linux) whose click opens
//! the first screen as a small popover, and the full window on demand, in one process.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rune::{Align, KeyPolicy, OverlayInput, OverlayOptions, OverlayPlacement, OverlaySpec, ScreenEdge, Tray, TrayEvent, TrayIcon, TrayItem, TraySpec};

use super::{Meters, Mode, POPOVER, POPOVER_MARGIN, Ui};
use crate::model::{Audio, State};

/// Tall enough for the first screen with a row open; outside the card, clicks pass through.
const HEIGHT: f32 = 720.0;

/// A speaker with two waves, white on transparent, drawn at `size` pixels.
pub fn icon(size: u32) -> TrayIcon {
    let s = size as f32 / 16.0;
    let body = [[2.5, 6.0], [4.7, 6.0], [8.0, 3.2], [8.0, 12.8], [4.7, 10.0], [2.5, 10.0]];
    let inside = |x: f32, y: f32| {
        let (x, y) = (x / s, y / s);
        let mut odd = false;
        let mut j = body.len() - 1;
        for i in 0..body.len() {
            let ([xi, yi], [xj, yj]) = (body[i], body[j]);
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                odd = !odd;
            }
            j = i;
        }
        if odd {
            return true;
        }
        let (dx, dy) = (x - 8.6, y - 8.0);
        let r = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx).abs();
        dx > 0.0 && angle < 0.95 && ((r - 2.9).abs() < 0.65 || (r - 5.4).abs() < 0.65)
    };
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    if inside(px as f32 + (sx as f32 + 0.5) / 4.0, py as f32 + (sy as f32 + 0.5) / 4.0) {
                        hits += 1;
                    }
                }
            }
            rgba.extend_from_slice(&[255, 255, 255, (hits * 255 / 16) as u8]);
        }
    }
    TrayIcon { width: size, height: size, rgba }
}

fn tooltip(s: &State) -> String {
    match s.default_sink() {
        Some(d) if d.mute => format!("{} · muted", d.description),
        Some(d) => format!("{} · {}%", d.description, (d.volume.combined() * 100.0).round()),
        None => "Sound".into(),
    }
}

/// Runs the tray, the popover and the (hidden) window until Quit.
pub fn service(audio: Audio) {
    rune::set_app_id("hokum-sound");
    let meters = Meters::new(audio.clone());
    let mut app = rune::App::new(super::spec())
        .window_options(super::window())
        .fit_content_height()
        .windowless()
        .on_close(|s: &mut Ui, _| s.hidden())
        .args(Vec::new());
    let window = app.handle();

    let mut popover = Ui::new(Some(audio.clone()), Mode::Popover);
    popover.meters = Some(meters.clone());
    popover.visible = false;
    let spec = OverlaySpec::new("hokum-sound", (POPOVER + 2.0 * POPOVER_MARGIN, HEIGHT), super::view).options(OverlayOptions {
        placement: Some(OverlayPlacement::Edge { edge: ScreenEdge::Top, margin: 4.0, align: Align::End, inset: 4.0 }),
        keys: KeyPolicy::OnDemand,
        input: OverlayInput::Regions,
        ..OverlayOptions::default()
    });
    let spec = spec.on_key(super::on_key).on_focus(|s: &mut Ui, focused, _| {
        if !focused && s.visible
            && let Some(d) = &s.dismiss
        {
            d();
        }
    });
    let spec = spec.setup(super::setup);
    let overlay = app.overlay(spec, popover);

    let shown = Arc::new(AtomicBool::new(false));
    let hide: Arc<dyn Fn() + Send + Sync> = {
        let (overlay, shown) = (overlay.clone(), shown.clone());
        Arc::new(move || {
            shown.store(false, Ordering::SeqCst);
            overlay.hide();
            overlay.update(|ui, _| ui.hidden());
        })
    };
    let open_window: Arc<dyn Fn() + Send + Sync> = {
        let window = window.clone();
        Arc::new(move || {
            window.update(|ui, _| ui.visible = true);
            window.open();
            window.focus();
        })
    };
    {
        let (hide, open_window) = (hide.clone(), open_window.clone());
        overlay.update(move |ui, _| {
            ui.dismiss = Some(hide);
            ui.open_window = Some(open_window);
        });
    }
    let toggle = {
        let (overlay, shown, hide) = (overlay.clone(), shown.clone(), hide.clone());
        move |token: Option<String>| {
            if shown.swap(true, Ordering::SeqCst) {
                hide();
                return;
            }
            overlay.update(|ui, _| {
                ui.visible = true;
                ui.open.clear();
                ui.page = super::Page::Main;
            });
            overlay.show();
            let _ = token;
            overlay.request_keys();
        }
    };

    let tray = {
        let (open_window, window) = (open_window.clone(), window.clone());
        let spec = TraySpec {
            id: "hokum-sound".into(),
            title: "Sound".into(),
            tooltip: tooltip(&audio.state()),
            icon: icon(32),
            menu: vec![TrayItem::action("open", "Open Sound"), TrayItem::Separator, TrayItem::action("quit", "Quit")],
        };
        match Tray::new(spec, move |e| match e {
            TrayEvent::Activate { token } => toggle(token),
            TrayEvent::Menu(id) if id == "open" => open_window(),
            TrayEvent::Menu(id) if id == "quit" => window.quit(),
            TrayEvent::Menu(_) => {}
        }) {
            Ok(t) => Some(Arc::new(t)),
            Err(e) => {
                eprintln!("sound: no tray icon: {e}");
                None
            }
        }
    };

    let last = Mutex::new(String::new());
    let subscription = {
        let (window, overlay, tray) = (window.clone(), overlay.clone(), tray.clone());
        audio.on_change(move |s| {
            if let Some(t) = &tray {
                let tip = tooltip(s);
                let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                if *last != tip {
                    t.set_tooltip(tip.clone());
                    *last = tip;
                }
            }
            let (a, b) = (s.clone(), s.clone());
            window.update(move |ui, _| ui.receive(a));
            overlay.update(move |ui, _| ui.receive(b));
        })
    };
    {
        let window = window.clone();
        crate::update::spawn(move |a| window.update(move |ui, _| ui.update = Some(a)));
    }
    app.init(move || {
        let mut ui = Ui::new(Some(audio), Mode::Window);
        ui.meters = Some(meters);
        ui.visible = false;
        ui._subscription = Some(Arc::new(subscription));
        ui.tray = tray;
        ui
    })
    .run();
}
