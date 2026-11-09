//! `aod-gui`: desktop window for aodm. Every phone image is a card with a live
//! thumbnail of its screen (a running phone's screenshot every few seconds; a stopped
//! phone shows its last one, dimmed), its state, and Start / Stop / Save / Clone.
//! Actions run through the `aodm` CLI, so they get the same checks and audit trail.

mod worker;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use aod_core::card::ImageRow;
use aod_core::env::Env;
use aod_core::log::Logger;
use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use worker::{Msg, Thumb, running};

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("AODM_GIT"), ")");
/// Width a thumbnail is drawn at.
const CARD_W: f32 = 180.0;
/// Height of a card's picture area (a phone screen is about 9:20).
const PIC_H: f32 = 400.0;
const ACTIVITY: usize = 8;

fn main() -> std::process::ExitCode {
    if let Some(a) = std::env::args().nth(1) {
        return match a.as_str() {
            "--version" | "-V" => {
                println!("aod-gui {VERSION}");
                std::process::ExitCode::SUCCESS
            }
            _ => {
                eprintln!("usage: aod-gui [--version]   (opens the phone images window)");
                std::process::ExitCode::from(2)
            }
        };
    }
    let env = Env::load();
    let log = Logger::new(&env);
    log.info(&format!(
        "gui: start {VERSION} (aodm {})",
        worker::aodm_bin().display()
    ));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Phone images (aodm)")
            .with_app_id("aod-gui")
            .with_inner_size([1000.0, 720.0]),
        ..Default::default()
    };
    let l = log.clone();
    let r = eframe::run_native(
        "aod-gui",
        options,
        Box::new(move |cc| {
            let (tx, rx) = mpsc::channel();
            let (wake, woken) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            let (e, wl) = (env.clone(), l.clone());
            std::thread::spawn(move || worker::poll(e, wl, tx, woken, || ctx.request_repaint()));
            Ok(Box::new(App::new(env, l, rx, wake)))
        }),
    );
    match r {
        Ok(()) => {
            log.info("gui: closed");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            // From the menu there is no terminal: the log is the only trace.
            log.error(&format!("gui: cannot open the window: {e}"));
            eprintln!("aod-gui: cannot open the window: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// A dialog waiting for an answer.
enum Ask {
    Stop(String),
    Clone {
        src: String,
        name: String,
    },
    /// Closing while actions run.
    Close,
}

struct App {
    env: Env,
    log: Logger,
    rx: Receiver<Msg>,
    wake: Sender<()>,
    /// Finished actions come back here (the poller has its own channel).
    done: (Sender<Msg>, Receiver<Msg>),
    cards: Vec<ImageRow>,
    error: Option<String>,
    tex: HashMap<String, TextureHandle>,
    /// Running phones whose screen is off.
    dark: HashSet<String>,
    /// Image name → action running on it.
    busy: BTreeMap<String, &'static str>,
    activity: Vec<(bool, String)>,
    ask: Option<Ask>,
    close_ok: bool,
}

fn color_image(t: &Thumb) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba)
}

impl App {
    fn new(env: Env, log: Logger, rx: Receiver<Msg>, wake: Sender<()>) -> App {
        App {
            env,
            log,
            rx,
            wake,
            done: mpsc::channel(),
            cards: Vec::new(),
            error: None,
            tex: HashMap::new(),
            dark: HashSet::new(),
            busy: BTreeMap::new(),
            activity: Vec::new(),
            ask: None,
            close_ok: false,
        }
    }

    fn set_thumb(&mut self, ctx: &egui::Context, name: &str, t: &Thumb) {
        let img = color_image(t);
        match self.tex.get_mut(name) {
            Some(h) => h.set(img, TextureOptions::LINEAR),
            None => {
                let h = ctx.load_texture(format!("thumb-{name}"), img, TextureOptions::LINEAR);
                self.tex.insert(name.to_string(), h);
            }
        }
    }

    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Cards(Ok(c)) => {
                    // Images that are gone (deleted, renamed) drop their screen.
                    self.tex.retain(|n, _| c.iter().any(|r| &r.name == n));
                    self.dark
                        .retain(|n| c.iter().any(|r| &r.name == n && running(r)));
                    // First sight: the stored last screen until a live one arrives.
                    for r in &c {
                        if !self.tex.contains_key(&r.name)
                            && let Some(t) = aod_core::images::thumb(&self.env, &r.name)
                                .and_then(|p| worker::load(&p))
                        {
                            self.set_thumb(ctx, &r.name, &t);
                        }
                    }
                    self.cards = c;
                    self.error = None;
                }
                Msg::Cards(Err(e)) => self.error = Some(e),
                Msg::Thumb(name, Some(t)) => {
                    self.dark.remove(&name);
                    self.set_thumb(ctx, &name, &t);
                }
                Msg::Thumb(name, None) => {
                    self.dark.insert(name);
                }
                Msg::Done { .. } => {}
            }
        }
        while let Ok(Msg::Done { name, text, ok }) = self.done.1.try_recv() {
            self.busy.remove(&name);
            self.activity.insert(0, (ok, text));
            self.activity.truncate(ACTIVITY);
        }
    }

    /// Runs `aodm ARGS` for `name` in the background.
    fn act(&mut self, ctx: &egui::Context, name: &str, label: &'static str, args: Vec<String>) {
        self.busy.insert(name.to_string(), label);
        self.log
            .info(&format!("gui: {label} {name}: aodm {}", args.join(" ")));
        let (tx, wake, log, ctx) = (
            self.done.0.clone(),
            self.wake.clone(),
            self.log.clone(),
            ctx.clone(),
        );
        let name = name.to_string();
        std::thread::spawn(move || {
            let t = Instant::now();
            let a: Vec<&str> = args.iter().map(String::as_str).collect();
            let (ok, msg) = worker::aodm(&a);
            let secs = t.elapsed().as_secs();
            let text = format!(
                "{name}: {label} {} ({secs} s): {msg}",
                if ok { "ok" } else { "failed" }
            );
            if ok {
                log.info(&format!("gui: {text}"))
            } else {
                log.warn(&format!("gui: {text}"))
            }
            let _ = wake.send(());
            let _ = tx.send(Msg::Done { name, text, ok });
            ctx.request_repaint();
        });
    }

    fn card(&mut self, ui: &mut egui::Ui, c: &ImageRow) {
        let up = running(c);
        let busy = self.busy.get(&c.name).copied();
        egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(CARD_W);
                ui.vertical(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(CARD_W, PIC_H), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 6.0, Color32::from_gray(24));
                    let dark = self.dark.contains(&c.name);
                    if let Some(t) = self.tex.get(&c.name) {
                        let s = t.size_vec2();
                        let k = (CARD_W / s.x).min(PIC_H / s.y);
                        let r = egui::Rect::from_center_size(rect.center(), s * k);
                        // Stopped or screen off: the last screen, dimmed.
                        let tint = if up && !dark {
                            Color32::WHITE
                        } else {
                            Color32::from_gray(90)
                        };
                        egui::Image::new(t).tint(tint).paint_at(ui, r);
                    }
                    let note = match (up, dark, self.tex.contains_key(&c.name)) {
                        (true, true, _) => Some("screen off"),
                        (true, false, false) => Some("waiting for screen…"),
                        (false, _, false) => Some("no screenshot yet"),
                        _ => None,
                    };
                    if let Some(n) = note {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            n,
                            egui::FontId::proportional(13.0),
                            Color32::LIGHT_GRAY,
                        );
                    }
                    ui.label(RichText::new(&c.name).strong().size(15.0));
                    let (state, col) = match (busy, up) {
                        (Some(b), _) => (format!("{b}…"), Color32::from_rgb(230, 180, 60)),
                        (None, true) => (c.state.clone(), Color32::from_rgb(90, 200, 120)),
                        (None, false) => (c.state.clone(), Color32::GRAY),
                    };
                    ui.horizontal(|ui| {
                        if busy.is_some() {
                            ui.spinner();
                        }
                        ui.label(RichText::new(state).color(col));
                    });
                    let saves = if c.saves == "-" { "0" } else { &c.saves };
                    ui.label(
                        RichText::new(format!(
                            "{} · API {} · {} · {saves} save(s)",
                            c.device, c.api, c.size
                        ))
                        .small()
                        .weak(),
                    );
                    if !c.groups.is_empty() {
                        ui.label(
                            RichText::new(format!("groups: {}", c.groups))
                                .small()
                                .weak(),
                        );
                    }
                    ui.add_enabled_ui(busy.is_none(), |ui| {
                        ui.horizontal(|ui| {
                            let n = c.name.clone();
                            if up {
                                if ui.button("Stop…").clicked() {
                                    self.ask = Some(Ask::Stop(n.clone()));
                                }
                            } else if ui.button("Start").clicked() {
                                self.act(ui.ctx(), &n, "starting", args(&["start", "--"], &n));
                            }
                            let hint = "stop the phone first";
                            let save = ui.add_enabled(!up, egui::Button::new("Save"));
                            if save.on_disabled_hover_text(hint).clicked() {
                                self.act(
                                    ui.ctx(),
                                    &n,
                                    "saving",
                                    args(&["save", "create", "--"], &n),
                                );
                            }
                            let clone = ui.add_enabled(!up, egui::Button::new("Clone…"));
                            if clone.on_disabled_hover_text(hint).clicked() {
                                self.ask = Some(Ask::Clone {
                                    name: format!("{n}-copy"),
                                    src: n,
                                });
                            }
                        });
                    });
                });
            });
    }

    /// The open dialog, if any.
    fn dialog(&mut self, ctx: &egui::Context) {
        let Some(ask) = self.ask.take() else { return };
        let title = match &ask {
            Ask::Stop(n) => format!("Stop {n}?"),
            Ask::Clone { src, .. } => format!("Clone {src}"),
            Ask::Close => "Actions still running".into(),
        };
        let mut keep = true;
        let mut answer: Option<bool> = None;
        let mut ask = ask;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut keep)
            .show(ctx, |ui| {
                match &mut ask {
                    Ask::Stop(n) => {
                        ui.label(format!(
                            "Apps running on {n} are interrupted. Its quick-boot snapshot is saved."
                        ));
                    }
                    Ask::Clone { name, .. } => {
                        ui.label("Name of the copy:");
                        let r = ui.text_edit_singleline(name);
                        let bad = aod_core::avd::validate_name(name.trim()).err();
                        if let Some(e) = &bad {
                            ui.colored_label(Color32::LIGHT_RED, e.to_string());
                        }
                        ui.label(RichText::new("The copy's first boot is a cold boot.").small().weak());
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if bad.is_none() && enter {
                            answer = Some(true);
                        }
                    }
                    Ask::Close => {
                        ui.label(format!(
                            "Still running: {}. Closing the window does not stop them, but you won't see how they end (check `aodm audit`).",
                            self.busy.keys().cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                ui.horizontal(|ui| {
                    let (yes, no) = match &ask {
                        Ask::Stop(_) => ("Stop", "Cancel"),
                        Ask::Clone { .. } => ("Clone", "Cancel"),
                        Ask::Close => ("Close anyway", "Keep open"),
                    };
                    let ok = !matches!(&ask, Ask::Clone { name, .. } if aod_core::avd::validate_name(name.trim()).is_err());
                    if ui.add_enabled(ok, egui::Button::new(yes)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(no).clicked() {
                        answer = Some(false);
                    }
                });
            });
        match (answer, keep) {
            (Some(true), _) => match ask {
                Ask::Stop(n) => self.act(ctx, &n, "stopping", args(&["stop", "--"], &n)),
                Ask::Clone { src, name } => {
                    let a = vec!["clone".into(), "--".into(), src.clone(), name.trim().into()];
                    self.act(ctx, &src, "cloning", a);
                }
                Ask::Close => {
                    self.close_ok = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            },
            (Some(false), _) | (None, false) => {}
            (None, true) => self.ask = Some(ask),
        }
    }
}

fn args(head: &[&str], name: &str) -> Vec<String> {
    head.iter()
        .map(|s| s.to_string())
        .chain([name.to_string()])
        .collect()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.receive(ctx);
        // Closing during an action: the child aodm keeps running (its output goes to a
        // pipe nobody reads, so it runs with --quiet), but ask first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.busy.is_empty() && !self.close_ok
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.ask = Some(Ask::Close);
        }
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Phone images");
                let up = self.cards.iter().filter(|c| running(c)).count();
                ui.label(format!("{up} running of {}", self.cards.len()));
                if ui.button("Refresh").clicked() {
                    let _ = self.wake.send(());
                }
                if let Some(e) = &self.error {
                    ui.colored_label(Color32::LIGHT_RED, e);
                }
            });
        });
        egui::Panel::bottom("activity").show(ui, |ui| {
            ui.label(RichText::new("Activity").strong());
            if self.activity.is_empty() {
                ui.label(
                    RichText::new(
                        "Actions you take show here; all are in adx.log and `aodm audit`.",
                    )
                    .weak(),
                );
            }
            for (ok, t) in &self.activity {
                let col = if *ok {
                    Color32::from_rgb(90, 200, 120)
                } else {
                    Color32::LIGHT_RED
                };
                ui.colored_label(col, t);
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in self.cards.clone() {
                        self.card(ui, &c);
                    }
                });
            });
        });
        self.dialog(ctx);
    }
}
