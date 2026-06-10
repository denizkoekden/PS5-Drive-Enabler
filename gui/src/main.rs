#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use egui::Color32;
use ps5de_core::{disk::Risk, Disk, ProgressEvent};
use std::sync::mpsc::Receiver;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([780.0, 620.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PS5 Drive Enabler",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()) as Box<dyn eframe::App>)),
    )
}

#[derive(PartialEq)]
enum Step {
    Intro,
    Select,
    Confirm,
    Working,
    Done,
    Failed,
}

enum Msg {
    Stage(String),
    Progress(f32),
    Finished,
    Failed(String),
}

struct App {
    step: Step,
    disks: Vec<Disk>,
    selected: Option<usize>,
    refresh_error: Option<String>,
    confirm_understood: bool,
    confirm_correct: bool,
    stage: String,
    progress: f32,
    rx: Option<Receiver<Msg>>,
    error: Option<String>,
}

impl App {
    fn new() -> Self {
        App {
            step: Step::Intro,
            disks: Vec::new(),
            selected: None,
            refresh_error: None,
            confirm_understood: false,
            confirm_correct: false,
            stage: String::new(),
            progress: 0.0,
            rx: None,
            error: None,
        }
    }

    fn refresh(&mut self) {
        self.selected = None;
        match ps5de_core::list_disks() {
            Ok(d) => {
                self.disks = d;
                self.refresh_error = None;
            }
            Err(e) => {
                self.disks.clear();
                self.refresh_error = Some(e.to_string());
            }
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let idx = match self.selected {
            Some(i) => i,
            None => return,
        };
        let disk = self.disks[idx].clone();
        let image = ps5de_core::embedded_image().to_vec();
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.progress = 0.0;
        self.stage = "Starting".into();
        self.error = None;
        self.step = Step::Working;

        let tx_cb = tx.clone();
        let ctx_cb = ctx.clone();
        let ctx_done = ctx.clone();
        std::thread::spawn(move || {
            let mut cb = move |ev: ProgressEvent| {
                let m = match ev {
                    ProgressEvent::Stage(s) => Msg::Stage(s.to_string()),
                    ProgressEvent::Progress { done, total } => {
                        Msg::Progress(if total == 0 { 1.0 } else { done as f32 / total as f32 })
                    }
                };
                let _ = tx_cb.send(m);
                ctx_cb.request_repaint();
            };
            let outcome = ps5de_core::flash_and_verify(&disk, &image, &mut cb);
            let _ = match outcome {
                Ok(()) => tx.send(Msg::Finished),
                Err(e) => tx.send(Msg::Failed(e.to_string())),
            };
            ctx_done.request_repaint();
        });
    }

    fn drain(&mut self) {
        let mut msgs = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(m) = rx.try_recv() {
                msgs.push(m);
            }
        }
        for m in msgs {
            match m {
                Msg::Stage(s) => self.stage = s,
                Msg::Progress(f) => self.progress = f,
                Msg::Finished => {
                    self.rx = None;
                    self.step = Step::Done;
                }
                Msg::Failed(e) => {
                    self.error = Some(e);
                    self.rx = None;
                    self.step = Step::Failed;
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("🎮  PS5 Drive Enabler");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                });
            });
            ui.label("Make any NVMe drive work in the PS5 by bypassing the PCIe Gen4 speed check.");
            ui.add_space(8.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.step {
            Step::Intro => self.ui_intro(ui),
            Step::Select => self.ui_select(ui, ctx),
            Step::Confirm => self.ui_confirm(ui, ctx),
            Step::Working => self.ui_working(ui),
            Step::Done => self.ui_done(ui),
            Step::Failed => self.ui_failed(ui),
        });
    }
}

impl App {
    fn ui_intro(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label("Before you start:");
        ui.add_space(4.0);
        bullet(ui, "Put your NVMe SSD into a USB-to-M.2 enclosure and connect it to this computer.");
        bullet(ui, "Make sure the drive is empty; its first 2 MB will be overwritten.");
        bullet(ui, "Run this program as Administrator (Windows) or with sudo (macOS/Linux).");
        ui.add_space(10.0);
        warn_box(
            ui,
            "Writing to the wrong drive can destroy data. You will choose the drive and \
             confirm it on the next screens.",
        );
        ui.add_space(16.0);
        if big_button(ui, "Start  ▶").clicked() {
            self.refresh();
            self.step = Step::Select;
        }
    }

    fn ui_select(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("Step 1: Choose the drive");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⟳ Refresh").clicked() {
                    self.refresh();
                }
            });
        });
        ui.label("Select the EXTERNAL drive whose size matches your NVMe.");
        ui.add_space(6.0);

        if let Some(err) = &self.refresh_error {
            warn_box(ui, err);
        }

        egui::ScrollArea::vertical()
            .max_height(330.0)
            .show(ui, |ui| {
                for i in 0..self.disks.len() {
                    let d = &self.disks[i];
                    let selected = self.selected == Some(i);
                    let (label, color) = match d.risk() {
                        Risk::Removable => ("USB / external", Color32::from_rgb(80, 200, 120)),
                        Risk::Internal => ("internal disk", Color32::from_rgb(230, 170, 60)),
                        Risk::System => ("SYSTEM, do not use", Color32::from_rgb(230, 90, 90)),
                    };
                    let text = format!(
                        "{}   {}   ·   {}   ·   {}",
                        d.id,
                        if d.model.is_empty() { "(unknown model)" } else { &d.model },
                        d.size_pretty(),
                        d.bus
                    );
                    let resp = ui.add(
                        egui::SelectableLabel::new(selected, egui::RichText::new(text).size(15.0)),
                    );
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.colored_label(color, format!("● {label}"));
                    });
                    if resp.clicked() {
                        self.selected = Some(i);
                    }
                    ui.separator();
                }
                if self.disks.is_empty() && self.refresh_error.is_none() {
                    ui.label("No disks detected. Connect your drive and press Refresh.");
                }
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("◀ Back").clicked() {
                self.step = Step::Intro;
            }
            let can_next = self.selected.is_some();
            ui.add_enabled_ui(can_next, |ui| {
                if big_button(ui, "Next  ▶").clicked() {
                    self.confirm_understood = false;
                    self.confirm_correct = false;
                    self.step = Step::Confirm;
                }
            });
        });
    }

    fn ui_confirm(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Step 2: Confirm");
        let d = match self.selected.and_then(|i| self.disks.get(i)) {
            Some(d) => d.clone(),
            None => {
                self.step = Step::Select;
                return;
            }
        };

        ui.add_space(6.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            kv(ui, "Drive", &d.id);
            kv(ui, "Model", if d.model.is_empty() { "(unknown)" } else { &d.model });
            kv(ui, "Size", &d.size_pretty());
            kv(ui, "Bus", &d.bus);
        });

        ui.add_space(8.0);
        match d.risk() {
            Risk::System => {
                warn_box(ui, "This is a SYSTEM disk. The tool will NOT write to it. Go back and pick your external NVMe.");
            }
            Risk::Internal => {
                warn_box(ui, "This is an internal disk, not a USB drive. This is probably the wrong choice. Go back unless you are absolutely sure.");
            }
            Risk::Removable => {
                warn_box(ui, "The first 2 MB of this drive will be ERASED and replaced with the enabler image. This cannot be undone.");
            }
        }

        ui.add_space(8.0);
        let blocked = d.risk() == Risk::System;
        ui.add_enabled_ui(!blocked, |ui| {
            ui.checkbox(&mut self.confirm_correct, "I have verified this is the correct drive (size and model match my NVMe).");
            ui.checkbox(&mut self.confirm_understood, "I understand the first 2 MB will be erased.");
        });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.button("◀ Back").clicked() {
                self.step = Step::Select;
            }
            let ready = !blocked && self.confirm_correct && self.confirm_understood;
            ui.add_enabled_ui(ready, |ui| {
                if big_button(ui, "Flash now  ⚡").clicked() {
                    self.start(ctx);
                }
            });
        });
    }

    fn ui_working(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 3: Writing");
        ui.add_space(20.0);
        ui.label(&self.stage);
        ui.add_space(6.0);
        ui.add(egui::ProgressBar::new(self.progress).show_percentage().desired_width(f32::INFINITY));
        ui.add_space(10.0);
        ui.label("Do not unplug the drive.");
        ui.spinner();
    }

    fn ui_done(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.colored_label(Color32::from_rgb(80, 200, 120), egui::RichText::new("✔ Success!").size(22.0));
        ui.add_space(10.0);
        ui.label("The enabler image was written and verified. Next steps:");
        ui.add_space(6.0);
        bullet(ui, "1. Eject the drive and take it out of the USB enclosure.");
        bullet(ui, "2. Install it into the PS5's M.2 expansion slot and screw it down.");
        bullet(ui, "3. Power on the PS5. The SSD appears under Settings > Storage, ready to use.");
        ui.add_space(18.0);
        if big_button(ui, "Flash another drive").clicked() {
            self.refresh();
            self.step = Step::Select;
        }
    }

    fn ui_failed(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.colored_label(Color32::from_rgb(230, 90, 90), egui::RichText::new("✖ Something went wrong").size(20.0));
        ui.add_space(10.0);
        if let Some(e) = &self.error {
            warn_box(ui, e);
        }
        ui.add_space(8.0);
        ui.label("Common fixes: run the program as Administrator / with sudo, and make sure the drive is connected and not in use.");
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("◀ Back to drive list").clicked() {
                self.refresh();
                self.step = Step::Select;
            }
        });
    }
}

// ---- small UI helpers ----

fn bullet(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label("•");
        ui.label(text);
    });
}

fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 18.0], egui::Label::new(egui::RichText::new(key).strong()));
        ui.label(value);
    });
}

fn warn_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(60, 45, 20))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::from_rgb(240, 200, 100), "⚠");
                ui.label(text);
            });
        });
}

fn big_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add_sized(
        [150.0, 34.0],
        egui::Button::new(egui::RichText::new(text).size(16.0)),
    )
}
