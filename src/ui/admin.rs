use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::model::{Dish, DishInput, PizzaSize, Supplier};
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

    // Dishes page state (Create)
    dish_name: String,
    dish_price: i64,
    pub sel_supplier_idx: usize,
    tag_is_pizza: bool,
    // Nummer (auch für ungetaggte Gerichte)
    dish_number: String,

    // Pizza Create
    pizza_number: String, // historisch – wir nutzen dish_number
    pizza_sizes: Vec<PizzaSize>,
    new_size_label: String,
    new_size_price: i64,

    // Dishes page state (Edit)
    edit_id: Option<ObjectId>,
    edit_is_pizza: bool,
    edit_name: String,
    edit_number: String,
    edit_price: i64,            // nur plain
    edit_sizes: Vec<PizzaSize>, // nur pizza
    edit_new_size_label: String,
    edit_new_size_price: i64,

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
            dish_number: String::new(),

            pizza_number: String::new(),
            pizza_sizes: vec![],
            new_size_label: String::new(),
            new_size_price: 0,

            edit_id: None,
            edit_is_pizza: false,
            edit_name: String::new(),
            edit_number: String::new(),
            edit_price: 0,
            edit_sizes: vec![],
            edit_new_size_label: String::new(),
            edit_new_size_price: 0,

            set_supplier_idx: 0,
        }
    }
}

fn eur(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}€{}.{}", abs / 100, format!("{:02}", abs % 100))
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

fn parse_nr_key(nr_opt: &Option<String>) -> i64 {
    // extrahiert führende Zahl aus "P12", "12", "Nr. 12", etc., sonst groß
    if let Some(nr) = nr_opt {
        let digits: String = nr.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if let Ok(x) = digits.parse::<i64>() {
                return x;
            }
        }
    }
    i64::MAX // ohne Nummer ans Ende
}

fn row_label(d: &Dish) -> String {
    if d.tags.iter().any(|t| t == "Pizza") {
        // "Pizza Nr. XX: Name [size:price, ...]"
        let nr = d.number.clone().unwrap_or_default();
        let sizes = d
            .pizza_sizes
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|p| format!("{} {}", p.label, eur(p.price_cents)))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if nr.is_empty() {
            format!("Pizza: {} [{}]", d.name, sizes)
        } else {
            format!("Pizza Nr. {}: {} [{}]", nr, d.name, sizes)
        }
    } else {
        // "{nr}: Name (Preis)"
        let nr = d.number.clone().unwrap_or_default();
        if nr.is_empty() {
            format!("{} ({})", d.name, eur(d.price_cents))
        } else {
            format!("{}: {} ({})", nr, d.name, eur(d.price_cents))
        }
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

    // Create form (Nummer auch bei non-Pizza)
    ui.horizontal(|ui| {
        ui.label("Nr.");
        ui.text_edit_singleline(&mut state.dish_number);
        ui.text_edit_singleline(&mut state.dish_name);
        ui.toggle_value(&mut state.tag_is_pizza, "Pizza");

        if state.tag_is_pizza {
            ui.add_enabled(false, egui::DragValue::new(&mut state.dish_price).prefix("€ disabled"));
        } else {
            ui.add(egui::DragValue::new(&mut state.dish_price).range(0..=100_000).prefix("Price (cents): "));
        }
    });

    if state.tag_is_pizza {
        ui.separator();
        ui.label("Pizza sizes");

        // Direkt editierbar (kein "Update"-Knopf)
        let mut remove_idx: Option<usize> = None;
        for idx in 0..state.pizza_sizes.len() {
            ui.horizontal(|ui| {
                ui.label(format!("#{idx}"));
                ui.label("Label");
                let l_ref: *mut String = &mut state.pizza_sizes[idx].label;
                let p_ref: *mut i64 = &mut state.pizza_sizes[idx].price_cents;
                // SAFETY: Wir bleiben im UI-Frame, keine Aliasierung außerhalb.
                unsafe {
                    ui.text_edit_singleline(&mut *l_ref);
                    ui.label("Price (cents)");
                    ui.add(egui::DragValue::new(&mut *p_ref).range(0..=100_000));
                }
                if ui.button("Remove").clicked() {
                    remove_idx = Some(idx);
                }
            });
        }
        if let Some(i) = remove_idx {
            if i < state.pizza_sizes.len() { state.pizza_sizes.remove(i); }
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
                number: if state.dish_number.trim().is_empty() { None } else { Some(state.dish_number.trim().to_string()) },
                pizza_sizes: if state.pizza_sizes.is_empty() { None } else { Some(state.pizza_sizes.clone()) },
            };
            if !input.name.is_empty() && input.pizza_sizes.is_some() {
                let _ = rt.block_on(dishes::create_with_tags(db, input));
                state.dish_name.clear();
                state.dish_number.clear();
                state.pizza_sizes.clear();
                state.new_size_label.clear();
                state.new_size_price = 0;
                state.tag_is_pizza = false;
            }
        } else if !state.dish_name.trim().is_empty() {
            let id = rt.block_on(dishes::create(db, sid, &state.dish_name, state.dish_price)).ok();
            // Nummer nachziehen (update_plain), wenn gesetzt
            if let (Some(_), true) = (id, !state.dish_number.trim().is_empty()) {
                if let Ok(dl) = rt.block_on(dishes::list_by_supplier(db, sid)) {
                    if let Some(d) = dl.into_iter().filter(|x| x.name == state.dish_name && x.number.is_none()).last() {
                        let _ = rt.block_on(dishes::update_plain(
                            db,
                            d.id.unwrap(),
                            &state.dish_name,
                            Some(state.dish_number.trim().to_string()),
                            state.dish_price,
                        ));
                    }
                }
            }
            state.dish_name.clear();
            state.dish_number.clear();
            state.dish_price = 0;
        }
    }

    ui.separator();
    ui.label("Existing dishes");

    // Liste laden und nach Nummer (asc) sortieren
    let mut dlist = rt.block_on(dishes::list_by_supplier(db, sid)).unwrap_or_default();
    dlist.sort_by_key(|d| parse_nr_key(&d.number));

    // Zeilen mit Edit/Delete
    for d in dlist {
        ui.horizontal(|ui| {
            ui.label(row_label(&d));
            if let Some(id) = d.id {
                if ui.button("Edit").clicked() {
                    state.edit_id = Some(id);
                    let is_pizza = d.tags.iter().any(|t| t == "Pizza");
                    state.edit_is_pizza = is_pizza;
                    state.edit_name = d.name.clone();
                    state.edit_number = d.number.clone().unwrap_or_default();
                    if is_pizza {
                        state.edit_sizes = d.pizza_sizes.clone().unwrap_or_default();
                        state.edit_price = 0;
                    } else {
                        state.edit_price = d.price_cents;
                        state.edit_sizes.clear();
                    }
                }
                if ui.button("Delete").clicked() {
                    let _ = rt.block_on(dishes::delete(db, id));
                }
            }
        });
    }

    // Edit-Formular
    if let Some(eid) = state.edit_id {
        ui.separator();
        ui.heading("Edit dish");

        ui.horizontal(|ui| {
            ui.label("Nr.");
            ui.text_edit_singleline(&mut state.edit_number);
            ui.text_edit_singleline(&mut state.edit_name);
            if state.edit_is_pizza {
                ui.add_enabled(false, egui::DragValue::new(&mut state.edit_price).prefix("€ disabled"));
            } else {
                ui.add(egui::DragValue::new(&mut state.edit_price).range(0..=100_000).prefix("Price (cents): "));
            }
        });

        if state.edit_is_pizza {
            ui.label("Pizza sizes");

            // Direkt editierbar: wir binden die Felder **by-ref** an state.edit_sizes
            let mut remove_idx: Option<usize> = None;
            for idx in 0..state.edit_sizes.len() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{idx}"));

                    // Sicher Referenzen ausleihen und im UI bearbeiten:
                    let l_ref: *mut String = &mut state.edit_sizes[idx].label;
                    let p_ref: *mut i64 = &mut state.edit_sizes[idx].price_cents;

                    // SAFETY: begrenzter Scope des UI-Aufrufs, keine aliasierten
                    // konkurrierenden &mut außerhalb der Closure.
                    unsafe {
                        ui.label("Label");
                        ui.text_edit_singleline(&mut *l_ref);

                        ui.label("Price (cents)");
                        ui.add(egui::DragValue::new(&mut *p_ref).range(0..=100_000));
                    }

                    if ui.button("Remove").clicked() {
                        remove_idx = Some(idx);
                    }
                });
            }
            if let Some(i) = remove_idx {
                if i < state.edit_sizes.len() { state.edit_sizes.remove(i); }
            }

            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.edit_new_size_label);
                ui.add(egui::DragValue::new(&mut state.edit_new_size_price).range(0..=100_000).prefix("Price (cents): "));
                if ui.button("Add size").clicked() && !state.edit_new_size_label.trim().is_empty() {
                    state.edit_sizes.push(PizzaSize {
                        label: state.edit_new_size_label.clone(),
                        price_cents: state.edit_new_size_price,
                    });
                    state.edit_new_size_label.clear();
                    state.edit_new_size_price = 0;
                }
            });
        }

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                if state.edit_is_pizza {
                    let _ = rt.block_on(dishes::update_pizza(
                        db,
                        eid,
                        &state.edit_name,
                        if state.edit_number.trim().is_empty() { None } else { Some(state.edit_number.trim().to_string()) },
                        state.edit_sizes.clone(),
                    ));
                } else {
                    let _ = rt.block_on(dishes::update_plain(
                        db,
                        eid,
                        &state.edit_name,
                        if state.edit_number.trim().is_empty() { None } else { Some(state.edit_number.trim().to_string()) },
                        state.edit_price,
                    ));
                }
                // Close editor
                state.edit_id = None;
                state.edit_name.clear();
                state.edit_number.clear();
                state.edit_sizes.clear();
                state.edit_price = 0;
            }

            if ui.button("Cancel").clicked() {
                state.edit_id = None;
                state.edit_name.clear();
                state.edit_number.clear();
                state.edit_sizes.clear();
                state.edit_price = 0;
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
