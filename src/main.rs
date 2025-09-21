mod config;
mod db;
mod model;
mod auth;
mod ui;
mod services;

use tokio::sync::mpsc;
use eframe::{egui, App, Frame};
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::services::updater;

const GH_OWNER: &str = "enzel-org";
const GH_REPO:  &str = "BestellDesk";

#[derive(Default)]
struct BestellDeskState {
    // Order/Admin sub-state
    order_state: ui::order::OrderState,
    admin_state: ui::admin::AdminState,

    // Setup / Connect
    server_input: String,          // MongoDB URI (direct or filled from agent)
    remember_server: bool,
    connect_err: Option<String>,

    // Agent Login
    agent_host: String,            // e.g. "agent.morwa.de:8443" or full URL
    agent_err: Option<String>,

    // Current tab
    tab: ui::UiTab,

    // Admin login
    admin_user: String,
    admin_pass: String,
    admin_authed: bool,

    // Updater UI state
    update_popup_open: bool,
    update_info: Option<updater::UpdateInfo>,
    update_error: Option<String>,
    updating_now: bool,
}

struct BestellDesk {
    rt: Arc<Runtime>,
    state: BestellDeskState,
    db: Option<db::Db>,
    rx: Option<mpsc::UnboundedReceiver<AppMsg>>,
    client_id: String,
}

enum AppMsg {
    SettingsChanged,
    SuppliersChanged,
    DishesChanged,
    OrdersChanged,
}

impl Default for BestellDesk {
    fn default() -> Self {
        // Load persisted config and ensure client_id
        let mut cfg = config::load().unwrap_or_default();
        if cfg.client_id.is_none() {
            cfg.client_id = Some(uuid::Uuid::new_v4().to_string());
            let _ = config::save(&cfg);
        }
        let client_id = cfg.client_id.clone().unwrap();
        let server_input = cfg.mongo_uri.clone().unwrap_or_default();
        let agent_host  = cfg.agent_host.clone().unwrap_or_default();

        let rt = Arc::new(Runtime::new().expect("tokio runtime"));

        // --- Update check at startup ---
        let current_ver = env!("CARGO_PKG_VERSION").to_string();
        let mut update_info: Option<updater::UpdateInfo> = None;
        match rt.block_on(updater::check_latest(GH_OWNER, GH_REPO, &current_ver)) {
            Ok(Some(info)) => update_info = Some(info),
            Ok(None) => {}
            Err(e) => eprintln!("Update check failed: {e:#}"),
        }

        Self {
            rt: rt.clone(),
            state: BestellDeskState {
                server_input,
                remember_server: cfg.remember_server,
                order_state: ui::order::OrderState::with_client_id(client_id.clone()),
                admin_state: Default::default(),

                // Prefill agent input from config
                agent_host,
                agent_err: None,

                update_popup_open: update_info.is_some(),
                update_info,
                update_error: None,
                updating_now: false,

                ..Default::default()
            },
            db: None,
            rx: None,
            client_id,
        }
    }
}

impl App for BestellDesk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // ----- UPDATE POPUP -----
        if self.state.update_popup_open {
            // local copy of the state for the .open() handle
            let mut open = self.state.update_popup_open;
            // flag toggled inside the closure to request closing the window
            let mut request_close = false;

            egui::Window::new("Update available")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(info) = &self.state.update_info {
                        ui.heading(format!("New version: {}", info.tag));
                        ui.separator();
                        ui.label("Release notes:");
                        egui::ScrollArea::vertical()
                            .max_height(240.0)
                            .show(ui, |ui| ui.label(&info.notes));
                        ui.add_space(8.0);

                        if let Some(err) = &self.state.update_error {
                            ui.colored_label(egui::Color32::RED, err);
                            ui.add_space(6.0);
                        }

                        ui.horizontal(|ui| {
                            if !self.state.updating_now {
                                if ui.button("Install now").clicked() {
                                    self.state.updating_now = true;
                                    match self.rt.block_on(updater::download_and_extract(info)) {
                                        Ok(new_exe) => {
                                            if let Err(e) = updater::spawn_replacer_and_exit(&new_exe) {
                                                self.state.update_error = Some(format!("{e:#}"));
                                                self.state.updating_now = false;
                                            }
                                        }
                                        Err(e) => {
                                            self.state.update_error = Some(format!("{e:#}"));
                                            self.state.updating_now = false;
                                        }
                                    }
                                }
                                if ui.button("Later").clicked() {
                                    // only set the flag; do NOT modify `open` directly inside the closure
                                    request_close = true;
                                }
                            } else {
                                ui.label("Installing…");
                            }
                        });
                    } else {
                        ui.label("No update information.");
                    }
                });

            // After show(): honor close request
            if request_close {
                open = false;
                // Also clear update_info so it does not pop up again later
                self.state.update_info = None;
            }

            // write back window state
            self.state.update_popup_open = open;
        }

        // ----- Connection setup -----
        if self.db.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Connect to MongoDB");

                // Direct Mongo URI (legacy/manual way)
                ui.label("Enter MongoDB connection string (e.g., mongodb+srv://user:pass@host/db)");
                ui.text_edit_singleline(&mut self.state.server_input);

                ui.add_space(8.0);

                // Agent login: resolves to a Mongo URI via your agent
                ui.label("Agent login (host:port or full URL)");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.state.agent_host);
                });
                if let Some(aerr) = &self.state.agent_err {
                    ui.colored_label(egui::Color32::YELLOW, format!("Agent hint: {aerr}"));
                }

                ui.add_space(6.0);
                ui.checkbox(&mut self.state.remember_server, "Remember this server");

                if ui.button("Connect").clicked() {
                    // Decide which URI to use
                    let mut used_agent = false;
                    let mut uri_to_use = self.state.server_input.trim().to_string();

                    if !self.state.agent_host.trim().is_empty() {
                        // Resolve via agent
                        match self.rt.block_on(services::agent_client::fetch_mongo_uri(&self.state.agent_host)) {
                            Ok(agent_uri) => {
                                uri_to_use = agent_uri;
                                self.state.agent_err = None;
                                self.state.server_input = uri_to_use.clone();
                                used_agent = true;
                            }
                            Err(e) => {
                                self.state.agent_err = Some(format!("{e:#}"));
                                self.state.connect_err = Some("Agent lookup failed".into());
                                return;
                            }
                        }
                    }

                    // Connect to MongoDB
                    match self.rt.block_on(db::connect(uri_to_use.trim())) {
                        Ok(dbh) => {
                            self.state.connect_err = None;

                            // Persist selection
                            let mut cfg = config::load().unwrap_or_default();
                            cfg.remember_server = self.state.remember_server;

                            if self.state.remember_server {
                                if used_agent {
                                    // Save the agent endpoint and clear direct URI
                                    cfg.agent_host = Some(self.state.agent_host.trim().to_string());
                                    cfg.mongo_uri  = None;
                                } else {
                                    // Save direct URI and clear agent endpoint
                                    cfg.mongo_uri  = Some(self.state.server_input.trim().to_string());
                                    cfg.agent_host = None;
                                }
                            } else {
                                // clear both
                                cfg.mongo_uri  = None;
                                cfg.agent_host = None;
                            }

                            if cfg.client_id.is_none() {
                                cfg.client_id = Some(self.client_id.clone());
                            }
                            let _ = config::save(&cfg);

                            // Spawn watchers...
                            let (tx, rx) = mpsc::unbounded_channel::<AppMsg>();
                            let db_clone = dbh.clone();
                            self.rt.spawn(db::watch_settings(db_clone.clone(), tx.clone()));
                            let db_clone = dbh.clone();
                            self.rt.spawn(db::watch_suppliers(db_clone.clone(), tx.clone()));
                            let db_clone = dbh.clone();
                            self.rt.spawn(db::watch_dishes(db_clone.clone(), tx.clone()));
                            let db_clone = dbh.clone();
                            self.rt.spawn(db::watch_orders(db_clone, tx.clone()));

                            self.db = Some(dbh);
                            self.rx = Some(rx);
                        }
                        Err(e) => self.state.connect_err = Some(format!("{e:#}")),
                    }
}

                if let Some(err) = &self.state.connect_err {
                    ui.colored_label(egui::Color32::RED, err);
                }
            });
            return;
        }

        // ----- Top navigation -----
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.state.tab == ui::UiTab::Order, "Order").clicked() {
                    self.state.tab = ui::UiTab::Order;
                }
                if ui.selectable_label(self.state.tab == ui::UiTab::Admin, "Admin").clicked() {
                    self.state.tab = ui::UiTab::Admin;
                }
            });
        });

        // ----- Watcher messages -----
        if let Some(rx) = &mut self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    AppMsg::SettingsChanged => {
                        self.state.order_state.loaded = false;
                        self.state.order_state.load_err = None;
                    }
                    AppMsg::SuppliersChanged => {
                        self.state.admin_state.sel_supplier_idx = 0;
                        self.state.admin_state.set_supplier_idx = 0;
                        self.state.order_state.loaded = false;
                        self.state.order_state.load_err = None;
                    }
                    AppMsg::DishesChanged => {
                        self.state.order_state.loaded = false;
                        self.state.order_state.load_err = None;
                    }
                    AppMsg::OrdersChanged => {
                        self.state.admin_state.orders_needs_reload = true;
                    }
                }
            }
        }

        // ----- Main content -----
        egui::CentralPanel::default().show(ctx, |ui| match self.state.tab {
            ui::UiTab::Order => ui::render_order(
                ui,
                &self.rt,
                self.db.as_ref().unwrap(),
                &mut self.state.order_state,
            ),
            ui::UiTab::Admin => ui::render_admin(
                ui,
                &self.rt,
                self.db.as_ref().unwrap(),
                &mut self.state.admin_user,
                &mut self.state.admin_pass,
                &mut self.state.admin_authed,
                &mut self.state.admin_state,
            ),
        });
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt().init();

    let opts = eframe::NativeOptions::default();

    // Read current version from Cargo.toml (injected at compile time)
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("BestellDesk v{}", version);

    // Run native application with version in window title
    eframe::run_native(
        &title,
        opts,
        Box::new(|_cc| Ok(Box::<BestellDesk>::default())),
    )
}
