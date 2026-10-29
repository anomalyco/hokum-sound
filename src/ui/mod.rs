//! The window, drawn with Rune in Hokum's look (`rune_theme_opencode`). It reads
//! [`State`] snapshots from an [`Audio`] handle and sends [`Command`](crate::model::Command)s;
//! it never touches PipeWire.
//!
//! A placeholder for now: the UI agent replaces the view. Keep `spec()`, `run()` and the
//! `mock` scene (scripts/build-linux.sh renders it as the release smoke test).

use std::sync::Arc;

use rune::{AppSpec, Cx, El, WindowOptions, col, text};
use rune_kit::Kit;

use crate::config::{Appearance, Config};
use crate::model::{Audio, State, Subscription};
use crate::update::Available;

pub const SIZE: (f32, f32) = (480.0, 640.0);

pub struct Ui {
    pub audio: Arc<State>,
    /// `None` in headless scenes.
    #[allow(dead_code)]
    pub handle: Option<Audio>,
    pub config: Config,
    /// A newer build, once the check-in found one: the UI's "update available" affordance.
    pub update: Option<Available>,
    _subscription: Option<Subscription>,
}

pub fn init() -> Ui {
    Ui { audio: Arc::new(crate::mock::sample()), handle: None, config: crate::config::load(), update: None, _subscription: None }
}

fn kit(s: &Ui, cx: &Cx<Ui>) -> &'static Kit {
    match s.config.appearance {
        Appearance::System => rune_theme_opencode::kit(cx.system_dark()),
        Appearance::Light => rune_theme_opencode::kit(false),
        Appearance::Dark => rune_theme_opencode::kit(true),
    }
}

fn view(s: &Ui, cx: &mut Cx<Ui>) -> El {
    let k = kit(s, cx);
    let output = s.audio.default_sink().map_or_else(|| "No output".to_owned(), |d| d.description.clone());
    col()
        .key("window")
        .w_full()
        .h_full()
        .padding(24.0)
        .gap(8.0)
        .fill(k.c.page)
        .child(text("Hokum Sound").key("title").font(k.ty.ui_medium.lh(16.0)).color(k.c.text_muted))
        .child(text(output).key("output").font(k.ty.ui_medium.lh(16.0)))
}

fn scene_mock(s: &mut Ui) {
    s.audio = Arc::new(crate::mock::sample());
}

pub fn spec() -> AppSpec<Ui> {
    AppSpec {
        scenes: &[("mock", scene_mock)],
        fonts: rune_theme_opencode::FONTS,
        families: rune_theme_opencode::FAMILIES,
        ..AppSpec::new("Hokum Sound", SIZE, init, view)
    }
}

pub fn window() -> WindowOptions {
    WindowOptions { min_size: Some((360.0, 360.0)), ..WindowOptions::default() }
}

/// Opens the window on `audio` until it closes. Update check-ins land in `Ui::update`.
pub fn run(audio: Audio) {
    rune::set_app_id("hokum-sound");
    let app = rune::App::new(spec()).window_options(window()).args(Vec::new());
    let handle = app.handle();
    let sub = {
        let handle = handle.clone();
        audio.on_change(move |s| {
            let s = s.clone();
            handle.update(move |ui, _| ui.audio = s);
        })
    };
    {
        let handle = handle.clone();
        crate::update::spawn(move |a| handle.update(move |ui, _| ui.update = Some(a)));
    }
    app.init(move || Ui { audio: audio.state(), handle: Some(audio), config: crate::config::load(), update: None, _subscription: Some(sub) }).run();
}
