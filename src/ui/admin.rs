use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::model::Supplier;
use crate::services::{admin_users, dishes, settings, suppliers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdminPage {
    Menu,
    Suppliers,
    Dishes,
    Settings,
}

/// UI state for the Admin area (no globals).
pub struct AdminState {
    page: AdminPage,

    // Suppliers page state (create)
    supplier_name: String,
    supplier_fee: i64,

    // Suppliers page state (edit)
    edit_id: Option<ObjectId>,
    edit_name: String,
    edit_fee: i64,

    // Dishes page state
    dish_name: String,
    dish_price: i64,
    pub sel_supplier_idx: usize,

    // Settings page state
    pub set_supplier_idx: usize,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            page: AdminPage::Menu,
            supplier_name: String::new(),
            supplier_fee: 0,
            edit_id: None,
            edit_name: String::new(),
            edit_fee: 0,
            dish_name: String::new(),
            dish_price: 0,
            sel_supplier_idx: 0,
            set_supplier_idx: 0,
        }
    }
}

/// Render the Admin area: bootstrap admin user → login → section router.
pub fn render(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    user: &mut String,
    pass: &mut String,
    authed: &mut bool,
    state: &mut AdminState,
) {
    // 1) Bootstrap
    let need_bootstrap = rt.block_on(admin_users::count(db)).unwrap_or(0) == 0;
    if need_bootstrap {
        ui.heading("Create first admin user");
        ui.label("Username");
        ui.text_edit_singleline(user);
        ui.label("Password");
        ui.add(egui::TextEdit::singleline(pass).password(true));
        if ui.button("Create admin").clicked() {
            match rt.block_on(admin_users::create(db, user, pass)) {
                Ok(_) => {
                    *authed = true;
                    pass.clear();
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::RED, e.to_string());
                }
            };
        }
        return;
    }

    // 2) Login gate
    if !*authed {
        ui.heading("Admin login");
        ui.label("Username");
        ui.text_edit_singleline(user);
        ui.label("Password");
        ui.add(egui::TextEdit::singleline(pass).password(true));
        if ui.button("Login").clicked() {
            *authed = rt
                .block_on(admin_users::verify(db, user, pass))
                .unwrap_or(false);
            if *authed {
                pass.clear();
            } else {
                ui.colored_label(egui::Color32::RED, "Invalid credentials");
            }
        }
        return;
    }

    // 3) Section navigation
    ui.horizontal(|ui| {
        if ui.button("Menu").clicked() {
            state.page = AdminPage::Menu;
        }
        if ui.button("Suppliers").clicked() {
            state.page = AdminPage::Suppliers;
        }
        if ui.button("Dishes").clicked() {
            state.page = AdminPage::Dishes;
        }
        if ui.button("Settings").clicked() {
            state.page = AdminPage::Settings;
        }
    });
    ui.separator();

    // 4) Route
    match state.page {
        AdminPage::Menu => {
            ui.heading("Admin");
            ui.label("Choose a section using the buttons above.");
        }
        AdminPage::Suppliers => page_suppliers(ui, rt, db, state),
        AdminPage::Dishes => page_dishes(ui, rt, db, state),
        AdminPage::Settings => page_settings(ui, rt, db, state),
    }
}

/// Suppliers CRUD page.
fn page_suppliers(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    ui.heading("Suppliers");

    // Create form
    ui.separator();
    ui.label("Create supplier");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.supplier_name);
        ui.add(
            egui::DragValue::new(&mut state.supplier_fee)
                .range(0..=10_000)
                .prefix("Delivery fee (cents): "),
        );
        if ui.button("Create").clicked() && !state.supplier_name.trim().is_empty() {
            let _ = rt.block_on(suppliers::create(
                db,
                &state.supplier_name,
                state.supplier_fee,
            ));
            state.supplier_name.clear();
        }
    });

    // List with edit
    ui.separator();
    ui.label("Existing suppliers");
    let list = rt.block_on(suppliers::list(db)).unwrap_or_default();
    for s in list {
        ui.horizontal(|ui| {
            ui.label(format!("{} (fee: {} cents)", s.name, s.delivery_fee_cents));
            if let Some(id) = s.id {
                if ui.button("Edit").clicked() {
                    state.edit_id = Some(id);
                    state.edit_name = s.name.clone();
                    state.edit_fee = s.delivery_fee_cents;
                }
                if ui.button("Delete").clicked() {
                    let _ = rt.block_on(suppliers::delete(db, id));
                }
            }
        });
    }

    // Edit form
    if let Some(eid) = state.edit_id {
        ui.separator();
        ui.label("Edit supplier");
        ui.text_edit_singleline(&mut state.edit_name);
        ui.add(
            egui::DragValue::new(&mut state.edit_fee)
                .range(0..=10_000)
                .prefix("Delivery fee (cents): "),
        );
        if ui.button("Save changes").clicked() {
            let _ = rt.block_on(suppliers::update(
                db,
                eid,
                &state.edit_name,
                state.edit_fee,
            ));
            state.edit_id = None;
            state.edit_name.clear();
        }
        if ui.button("Cancel").clicked() {
            state.edit_id = None;
            state.edit_name.clear();
        }
    }
}

/// Dishes CRUD page.
fn page_dishes(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    ui.heading("Dishes");

    let sups = rt.block_on(suppliers::list(db)).unwrap_or_default();
    if sups.is_empty() {
        ui.label("No suppliers yet. Create a supplier first.");
        return;
    }

    // Select supplier
    if state.sel_supplier_idx >= sups.len() {
        state.sel_supplier_idx = 0;
    }
    egui::ComboBox::from_label("Supplier")
        .selected_text(sups[state.sel_supplier_idx].name.clone())
        .show_ui(ui, |cb| {
            for (i, s) in sups.iter().enumerate() {
                cb.selectable_value(&mut state.sel_supplier_idx, i, s.name.clone());
            }
        });

    let sid = sups[state.sel_supplier_idx].id.unwrap();

    // Create dish form
    ui.separator();
    ui.label("Create dish");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.dish_name);
        ui.add(
            egui::DragValue::new(&mut state.dish_price)
                .range(0..=100_000)
                .prefix("Price (cents): "),
        );
        if ui.button("Add dish").clicked() && !state.dish_name.trim().is_empty() {
            let _ = rt.block_on(dishes::create(
                db,
                sid,
                &state.dish_name,
                state.dish_price,
            ));
            state.dish_name.clear();
        }
    });

    // List dishes
    ui.separator();
    ui.label("Existing dishes");
    let dlist = rt
        .block_on(dishes::list_by_supplier(db, sid))
        .unwrap_or_default();
    for d in dlist {
        ui.horizontal(|ui| {
            ui.label(format!("{} ({} cents)", d.name, d.price_cents));
            if let Some(id) = d.id {
                if ui.button("Delete").clicked() {
                    let _ = rt.block_on(dishes::delete(db, id));
                }
            }
        });
    }
}

/// Settings page: set active supplier.
fn page_settings(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    ui.heading("Settings");

    let sups = rt.block_on(suppliers::list(db)).unwrap_or_default();
    if sups.is_empty() {
        ui.label("No suppliers yet. Create one first.");
        return;
    }
    if state.set_supplier_idx >= sups.len() {
        state.set_supplier_idx = 0;
    }

    // Show current active
    let active = rt
        .block_on(settings::get_active_supplier_id(db))
        .ok()
        .flatten();
    if let Some(a) = active {
        ui.label(format!("Active supplier: {}", id_to_name(&sups, a)));
    } else {
        ui.label("Active supplier: none");
    }

    // Choose and set active
    egui::ComboBox::from_label("Choose active supplier")
        .selected_text(sups[state.set_supplier_idx].name.clone())
        .show_ui(ui, |cb| {
            for (i, s) in sups.iter().enumerate() {
                cb.selectable_value(&mut state.set_supplier_idx, i, s.name.clone());
            }
        });
    if ui.button("Set active").clicked() {
        let sid = sups[state.set_supplier_idx].id.unwrap();
        let _ = rt.block_on(settings::set_active_supplier(db, sid));
    }
}

fn id_to_name(sups: &[Supplier], id: ObjectId) -> String {
    sups.iter()
        .find(|s| s.id == Some(id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| id.to_hex())
}
