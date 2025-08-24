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

#[derive(Default)]
struct BestellAppState {
    // UI State
    order_state: ui::order::OrderState,
    admin_state: ui::admin::AdminState,

    // Persisted config
    server_input: String,
    remember_server: bool,
    connect_err: Option<String>,

    // Simple tab state
    tab: ui::UiTab,

    // Admin login state
    admin_user: String,
    admin_pass: String,
    admin_authed: bool,
}

struct BestellApp {
    rt: Arc<Runtime>,
    state: BestellAppState,
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

impl Default for BestellApp {
    fn default() -> Self {
        // Load persisted local config and ensure client_id
        let mut cfg = config::load().unwrap_or_default();
        if cfg.client_id.is_none() {
            cfg.client_id = Some(uuid::Uuid::new_v4().to_string());
            let _ = config::save(&cfg);
        }
        let client_id = cfg.client_id.clone().unwrap();
        let server_input = cfg.mongo_uri.clone().unwrap_or_default();

        Self {
            rt: Arc::new(Runtime::new().expect("tokio runtime")),
            state: BestellAppState {
                server_input,
                remember_server: cfg.remember_server,
                order_state: ui::order::OrderState {
                    // Inject client_id so orders can include it
                    client_id: client_id.clone(),
                    ..Default::default()
                },
                admin_state: Default::default(),
                ..Default::default()
            },
            db: None,
            rx: None,
            client_id,
        }
    }
}

impl App for BestellApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        if self.db.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("First-time setup");
                ui.label("Enter MongoDB connection string (e.g., mongodb+srv://user:pass@host/db)");
                ui.text_edit_singleline(&mut self.state.server_input);
                ui.checkbox(&mut self.state.remember_server, "Remember this server");
                if ui.button("Connect").clicked() {
                    match self.rt.block_on(db::connect(&self.state.server_input)) {
                        Ok(dbh) => {
                            self.state.connect_err = None;

                            // Persist config without losing client_id
                            let mut cfg = config::load().unwrap_or_default();
                            cfg.remember_server = self.state.remember_server;
                            cfg.mongo_uri = if self.state.remember_server {
                                Some(self.state.server_input.clone())
                            } else {
                                None
                            };
                            if cfg.client_id.is_none() {
                                cfg.client_id = Some(self.client_id.clone());
                            }
                            let _ = config::save(&cfg);

                            // Spawn watchers
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

        // Top tab bar
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.state.tab == ui::UiTab::Order, "Order").clicked() {
                    self.state.tab = ui::UiTab::Order;
                }
                if ui.selectable_label(self.state.tab == ui::UiTab::Admin, "Admin").clicked() {
                    self.state.tab = ui::UiTab::Admin;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Reconnect").clicked() {
                        self.db = None;
                        self.rx = None;
                    }
                });
            });
        });

        // Handle realtime messages (change streams)
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
                        // Placeholder for future admin "current orders" view refresh
                    }
                }
            }
        }

        // Routed content
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
    eframe::run_native(
        "BestellDesk",
        opts,
        Box::new(|_cc| Ok(Box::<BestellApp>::default())),
    )
}
