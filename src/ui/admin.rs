use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::model::{Supplier, DishInput, PizzaSize};
use crate::services::{admin_users, dishes, settings, suppliers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdminPage {
    Menu,
    Suppliers,
    Dishes,
    Settings,
}

pub struct AdminState {
    page: AdminPage,

    // Suppliers page state
    supplier_name: String,
    supplier_fee: i64,

    // Dishes page state
    dish_name: String,
    dish_price: i64,
    pub sel_supplier_idx: usize,

    // Tags/Pizza-UI
    tag_is_pizza: bool,
    pizza_number: String,
    pizza_sizes: Vec<PizzaSize>, // dynamische rows
    new_size_label: String,
    new_size_price: i64,

    // Settings page state
    pub set_supplier_idx: usize,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            page: AdminPage::Menu,
            supplier_name: String::new(),
            supplier_fee: 0,
            dish_name: String::new(),
            dish_price: 0,
            sel_supplier_idx: 0,
            tag_is_pizza: false,
            pizza_number: String::new(),
            pizza_sizes: vec![],
            new_size_label: String::new(),
            new_size_price: 0,
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
    // 1) Bootstrap admin
    let need_bootstrap = rt.block_on(admin_users::count(db)).unwrap_or(0) == 0;
    if need_bootstrap {
        ui.heading("Create first admin user");
        ui.label("Username");
        ui.text_edit_singleline(user);
        ui.label("Password");
        ui.add(egui::TextEdit::singleline(pass).password(true));
        if ui.button("Create admin").clicked() {
            match rt.block_on(admin_users::create(db, user, pass)) {
                Ok(_) => { *authed = true; pass.clear(); }
                Err(e) => { ui.colored_label(egui::Color32::RED, e.to_string()); }
            };
        }
        return;
    }

    // 2) Login
    if !*authed {
        ui.heading("Admin login");
        ui.label("Username");
        ui.text_edit_singleline(user);
        ui.label("Password");
        ui.add(egui::TextEdit::singleline(pass).password(true));
        if ui.button("Login").clicked() {
            let ok = rt.block_on(admin_users::verify(db, user, pass)).unwrap_or(false);
            if ok {
                *authed = true;
                pass.clear();
            } else {
                ui.colored_label(egui::Color32::RED, "Invalid credentials");
            }
        }
        return;
    }

    // 3) Nav
    ui.horizontal(|ui| {
        if ui.button("Menu").clicked()      { state.page = AdminPage::Menu; }
        if ui.button("Suppliers").clicked() { state.page = AdminPage::Suppliers; }
        if ui.button("Dishes").clicked()    { state.page = AdminPage::Dishes; }
        if ui.button("Settings").clicked()  { state.page = AdminPage::Settings; }
    });
    ui.separator();

    // 4) Route
    match state.page {
        AdminPage::Menu => { ui.heading("Admin"); ui.label("Choose a section above."); }
        AdminPage::Suppliers => page_suppliers(ui, rt, db, state),
        AdminPage::Dishes => page_dishes(ui, rt, db, state),
        AdminPage::Settings => page_settings(ui, rt, db, state),
    }
}

fn page_suppliers(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    ui.heading("Suppliers");
    ui.separator();
    ui.label("Create supplier");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.supplier_name);
        ui.add(egui::DragValue::new(&mut state.supplier_fee).range(0..=10_000).prefix("Delivery fee (cents): "));
        if ui.button("Create").clicked() && !state.supplier_name.trim().is_empty() {
            let _ = rt.block_on(suppliers::create(db, &state.supplier_name, state.supplier_fee));
            state.supplier_name.clear();
        }
    });

    ui.separator();
    ui.label("Existing suppliers");
    for s in rt.block_on(suppliers::list(db)).unwrap_or_default() {
        ui.horizontal(|ui| {
            ui.label(format!("{} (fee: {} cents)", s.name, s.delivery_fee_cents));
            if let Some(id) = s.id {
                if ui.button("Delete").clicked() { let _ = rt.block_on(suppliers::delete(db, id)); }
            }
        });
    }
}

fn page_dishes(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    ui.heading("Dishes");
    let sups = rt.block_on(suppliers::list(db)).unwrap_or_default();
    if sups.is_empty() { ui.label("No suppliers yet."); return; }
    if state.sel_supplier_idx >= sups.len() { state.sel_supplier_idx = 0; }

    egui::ComboBox::from_label("Supplier")
        .selected_text(sups[state.sel_supplier_idx].name.clone())
        .show_ui(ui, |cb| {
            for (i, s) in sups.iter().enumerate() {
                cb.selectable_value(&mut state.sel_supplier_idx, i, s.name.clone());
            }
        });
    let sid = sups[state.sel_supplier_idx].id.unwrap();

    ui.separator();
    ui.label("Create dish");

    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.dish_name);
        ui.toggle_value(&mut state.tag_is_pizza, "Pizza");
        if state.tag_is_pizza {
            ui.add_enabled(false, egui::DragValue::new(&mut state.dish_price).prefix("€ price disabled"));
        } else {
            ui.add(egui::DragValue::new(&mut state.dish_price).range(0..=100_000).prefix("Price (cents): "));
        }
    });

    if state.tag_is_pizza {
        ui.horizontal(|ui| {
            ui.label("Nr.");
            ui.text_edit_singleline(&mut state.pizza_number);
        });

        ui.separator();
        ui.label("Pizza sizes");

        // Sicheres Iterieren + Editieren (kein doppeltes Borrow)
        let mut to_remove: Option<usize> = None;
        let len = state.pizza_sizes.len();
        for idx in 0..len {
            // lokale Kopie bearbeiten
            let mut item = state.pizza_sizes[idx].clone();
            ui.horizontal(|ui| {
                ui.label(format!("#{idx}"));
                ui.label("Label");
                ui.text_edit_singleline(&mut item.label);
                ui.label("Price (cents)");
                ui.add(egui::DragValue::new(&mut item.price_cents).range(0..=100_000));

                if ui.button("Update").clicked() {
                    // nach der UI-Zeile zurückschreiben
                    state.pizza_sizes[idx] = item.clone();
                }
                if ui.button("Remove").clicked() {
                    to_remove = Some(idx);
                }
            });
        }
        if let Some(idx) = to_remove {
            if idx < state.pizza_sizes.len() {
                state.pizza_sizes.remove(idx);
            }
        }

        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.new_size_label);
            ui.add(egui::DragValue::new(&mut state.new_size_price).range(0..=100_000).prefix("Price (cents): "));
            if ui.button("Add size").clicked() && !state.new_size_label.trim().is_empty() {
                state.pizza_sizes.push(PizzaSize {
                    label: state.new_size_label.clone(),
                    price_cents: state.new_size_price,
                });
                state.new_size_label.clear();
                state.new_size_price = 0;
            }
        });
    }

    if ui.button("Create").clicked() {
        if state.tag_is_pizza {
            let input = DishInput {
                supplier_id: sid,
                name: state.dish_name.trim().to_string(),
                price_cents: None,
                tags: vec!["Pizza".to_string()],
                number: if state.pizza_number.trim().is_empty() { None } else { Some(state.pizza_number.trim().to_string()) },
                pizza_sizes: if state.pizza_sizes.is_empty() { None } else { Some(state.pizza_sizes.clone()) },
            };
            if !input.name.is_empty() && input.pizza_sizes.is_some() {
                let _ = rt.block_on(dishes::create_with_tags(db, input));
                state.dish_name.clear();
                state.pizza_number.clear();
                state.pizza_sizes.clear();
                state.new_size_label.clear();
                state.new_size_price = 0;
                state.tag_is_pizza = false;
            }
        } else if !state.dish_name.trim().is_empty() {
            let _ = rt.block_on(dishes::create(db, sid, &state.dish_name, state.dish_price));
            state.dish_name.clear();
            state.dish_price = 0;
        }
    }

    ui.separator();
    ui.label("Existing dishes");
    for d in rt.block_on(dishes::list_by_supplier(db, sid)).unwrap_or_default() {
        ui.horizontal(|ui| {
            if d.tags.iter().any(|t| t == "Pizza") {
                let sizes = d.pizza_sizes.as_ref()
                    .map(|v| v.iter().map(|p| format!("{}:{}c", p.label, p.price_cents)).collect::<Vec<_>>().join(", "))
                    .unwrap_or_default();
                let nr = d.number.clone().unwrap_or_default();
                ui.label(format!("Pizza {} — {} [{}]", nr, d.name, sizes));
            } else {
                ui.label(format!("{} ({} cents)", d.name, d.price_cents));
            }
            if let Some(id) = d.id {
                if ui.button("Delete").clicked() {
                    let _ = rt.block_on(dishes::delete(db, id));
                }
            }
        });
    }
}

fn page_settings(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    ui.heading("Settings");

    let sups = rt.block_on(suppliers::list(db)).unwrap_or_default();
    if sups.is_empty() { ui.label("No suppliers yet. Create one first."); return; }
    if state.set_supplier_idx >= sups.len() { state.set_supplier_idx = 0; }

    let active = rt.block_on(settings::get_active_supplier_id(db)).ok().flatten();
    if let Some(a) = active {
        ui.label(format!("Active supplier: {}", id_to_name(&sups, a)));
    } else {
        ui.label("Active supplier: none");
    }

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
