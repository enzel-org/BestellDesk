use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::model::{Dish, DishInput, PizzaSize, Supplier, Category};
use crate::services::{admin_users, dishes, settings, suppliers, categories};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdminPage { Menu, Suppliers, Dishes, Categories, Settings }

pub struct AdminState {
    page: AdminPage,

    supplier_name: String,
    supplier_fee: i64,
    edit_supplier_id: Option<ObjectId>,
    edit_supplier_name: String,
    edit_supplier_fee: i64,

    dish_name: String,
    dish_price: i64,
    pub sel_supplier_idx: usize,
    tag_is_pizza: bool,
    dish_number: String,
    pizza_sizes: Vec<PizzaSize>,
    new_size_label: String,
    new_size_price: i64,

    edit_id: Option<ObjectId>,
    edit_is_pizza: bool,
    edit_name: String,
    edit_number: String,
    edit_price: i64,
    edit_sizes: Vec<PizzaSize>,
    edit_new_size_label: String,
    edit_new_size_price: i64,

    available_categories: Vec<Category>,
    chosen_categories_create: Vec<ObjectId>,
    chosen_categories_edit: Vec<ObjectId>,

    pub cat_new_name: String,
    pub cat_edit_id: Option<ObjectId>,
    pub cat_edit_name: String,
    pub cat_edit_pos: i64,

    pub set_supplier_idx: usize,

    backup_pass: String,
    backup_export_path: String,
    backup_import_path: String,
    backup_msg: Option<(bool, String)>,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            page: AdminPage::Menu,

            supplier_name: String::new(),
            supplier_fee: 0,
            edit_supplier_id: None,
            edit_supplier_name: String::new(),
            edit_supplier_fee: 0,

            dish_name: String::new(),
            dish_price: 0,
            sel_supplier_idx: 0,
            tag_is_pizza: false,
            dish_number: String::new(),
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

            available_categories: vec![],
            chosen_categories_create: vec![],
            chosen_categories_edit: vec![],

            cat_new_name: String::new(),
            cat_edit_id: None,
            cat_edit_name: String::new(),
            cat_edit_pos: 0,

            set_supplier_idx: 0,

            backup_pass: String::new(),
            backup_export_path: "backup.json.enc".to_string(),
            backup_import_path: String::new(),
            backup_msg: None,
        }
    }
}

fn eur(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}€{}.{}", abs / 100, format!("{:02}", abs % 100))
}

pub fn render(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    user: &mut String,
    pass: &mut String,
    authed: &mut bool,
    state: &mut AdminState,
) {
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

    if !*authed {
        ui.heading("Admin login");
        ui.label("Username");
        ui.text_edit_singleline(user);
        ui.label("Password");
        ui.add(egui::TextEdit::singleline(pass).password(true));
        if ui.button("Login").clicked() {
            let ok = rt.block_on(admin_users::verify(db, user, pass)).unwrap_or(false);
            if ok { *authed = true; pass.clear(); }
            else { ui.colored_label(egui::Color32::RED, "Invalid credentials"); }
        }
        return;
    }

    ui.horizontal(|ui| {
        if ui.button("Menu").clicked()       { state.page = AdminPage::Menu; }
        if ui.button("Suppliers").clicked()  { state.page = AdminPage::Suppliers; }
        if ui.button("Dishes").clicked()     { state.page = AdminPage::Dishes; }
        if ui.button("Categories").clicked() { state.page = AdminPage::Categories; }
        if ui.button("Settings").clicked()   { state.page = AdminPage::Settings; }
    });
    ui.separator();

    match state.page {
        AdminPage::Menu => { ui.heading("Admin"); ui.label("Choose a section above."); }
        AdminPage::Suppliers => page_suppliers(ui, rt, db, state),
        AdminPage::Dishes => page_dishes(ui, rt, db, state),
        AdminPage::Categories => page_categories(ui, rt, db, state),
        AdminPage::Settings => page_settings(ui, rt, db, state),
    }
}

/* ---------------- Suppliers ---------------- */

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

    ui.separator();
    ui.label("Existing suppliers");
    let list = rt.block_on(suppliers::list(db)).unwrap_or_default();
    for s in list {
        ui.horizontal(|ui| {
            ui.label(format!("{} (fee: {} cents)", s.name, s.delivery_fee_cents));
            if let Some(id) = s.id {
                if ui.button("Edit").clicked() {
                    state.edit_supplier_id = Some(id);
                    state.edit_supplier_name = s.name.clone();
                    state.edit_supplier_fee = s.delivery_fee_cents;
                }
                if ui.button("Delete").clicked() {
                    let _ = rt.block_on(suppliers::delete(db, id));
                }
            }
        });
    }

    if let Some(eid) = state.edit_supplier_id {
        ui.separator();
        ui.heading("Edit supplier");
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut state.edit_supplier_name);
            ui.label("Delivery fee (cents)");
            ui.add(
                egui::DragValue::new(&mut state.edit_supplier_fee)
                    .range(0..=10_000),
            );
        });

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                let _ = rt.block_on(suppliers::update(
                    db,
                    eid,
                    &state.edit_supplier_name,
                    state.edit_supplier_fee,
                ));
                state.edit_supplier_id = None;
                state.edit_supplier_name.clear();
                state.edit_supplier_fee = 0;
            }
            if ui.button("Cancel").clicked() {
                state.edit_supplier_id = None;
                state.edit_supplier_name.clear();
                state.edit_supplier_fee = 0;
            }
        });
    }
}

/* ---------------- Dishes (Create + Edit) ---------------- */

fn parse_nr_key(nr_opt: &Option<String>) -> i64 {
    if let Some(nr) = nr_opt {
        let digits: String = nr.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if let Ok(x) = digits.parse::<i64>() { return x; }
        }
    }
    i64::MAX
}

fn row_label(d: &Dish) -> String {
    if d.tags.iter().any(|t| t == "Pizza") {
        let nr = d.number.clone().unwrap_or_default();
        let sizes = d.pizza_sizes.as_ref().map(|v| {
            v.iter().map(|p| format!("{} {}", p.label, eur(p.price_cents))).collect::<Vec<_>>().join(", ")
        }).unwrap_or_default();
        if nr.is_empty() { format!("Pizza: {} [{}]", d.name, sizes) }
        else { format!("Pizza Nr. {}: {} [{}]", nr, d.name, sizes) }
    } else {
        let nr = d.number.clone().unwrap_or_default();
        if nr.is_empty() { format!("{} ({})", d.name, eur(d.price_cents)) }
        else { format!("{}: {} ({})", nr, d.name, eur(d.price_cents)) }
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
    let sid = sups[state.sel_supplier_idx].id.unwrap();

    state.available_categories = rt.block_on(categories::list_by_supplier(db, sid)).unwrap_or_default();

    egui::ComboBox::from_label("Supplier")
        .selected_text(sups[state.sel_supplier_idx].name.clone())
        .show_ui(ui, |cb| {
            for (i, s) in sups.iter().enumerate() {
                cb.selectable_value(&mut state.sel_supplier_idx, i, s.name.clone());
            }
        });

    ui.separator();
    ui.label("Create dish");

    ui.horizontal_wrapped(|ui| {
        ui.label("Categories:");
        for c in &state.available_categories {
            let mut checked = state.chosen_categories_create.contains(&c.id.unwrap());
            if ui.checkbox(&mut checked, c.name.clone()).clicked() {
                if checked {
                    state.chosen_categories_create.push(c.id.unwrap());
                } else {
                    state.chosen_categories_create.retain(|x| *x != c.id.unwrap());
                }
            }
        }
    });

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
        let mut remove_idx: Option<usize> = None;
        for idx in 0..state.pizza_sizes.len() {
            ui.horizontal(|ui| {
                ui.label(format!("#{idx}"));
                let l_ref: *mut String = &mut state.pizza_sizes[idx].label;
                let p_ref: *mut i64 = &mut state.pizza_sizes[idx].price_cents;
                unsafe {
                    ui.text_edit_singleline(&mut *l_ref);
                    ui.label("Price (cents)");
                    ui.add(egui::DragValue::new(&mut *p_ref).range(0..=100_000));
                }
                if ui.button("Remove").clicked() { remove_idx = Some(idx); }
            });
        }
        if let Some(i) = remove_idx { if i < state.pizza_sizes.len() { state.pizza_sizes.remove(i); } }

        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.new_size_label);
            ui.add(egui::DragValue::new(&mut state.new_size_price).range(0..=100_000).prefix("Price (cents): "));
            if ui.button("Add size").clicked() && !state.new_size_label.trim().is_empty() {
                state.pizza_sizes.push(PizzaSize { label: state.new_size_label.clone(), price_cents: state.new_size_price });
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
                categories: Some(state.chosen_categories_create.clone()),
            };
            if !input.name.is_empty() && input.pizza_sizes.is_some() {
                let _ = rt.block_on(dishes::create_with_tags(db, input));
                state.dish_name.clear();
                state.dish_number.clear();
                state.pizza_sizes.clear();
                state.new_size_label.clear();
                state.new_size_price = 0;
                state.tag_is_pizza = false;
                state.chosen_categories_create.clear();
            }
        } else if !state.dish_name.trim().is_empty() {
            let _ = rt.block_on(dishes::create_plain(
                db,
                sid,
                &state.dish_name,
                if state.dish_number.trim().is_empty() { None } else { Some(state.dish_number.trim().to_string()) },
                state.dish_price,
                state.chosen_categories_create.clone(),
            ));
            state.dish_name.clear();
            state.dish_number.clear();
            state.dish_price = 0;
            state.chosen_categories_create.clear();
        }
    }

    ui.separator();
    ui.label("Existing dishes");

    let mut dlist = rt.block_on(dishes::list_by_supplier(db, sid)).unwrap_or_default();
    dlist.sort_by_key(|d| parse_nr_key(&d.number));

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
                    state.chosen_categories_edit = d.categories.clone();
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

    if let Some(eid) = state.edit_id {
        ui.separator();
        ui.heading("Edit dish");

        ui.horizontal_wrapped(|ui| {
            ui.label("Categories:");
            for c in &state.available_categories {
                let mut checked = state.chosen_categories_edit.contains(&c.id.unwrap());
                if ui.checkbox(&mut checked, c.name.clone()).clicked() {
                    if checked {
                        state.chosen_categories_edit.push(c.id.unwrap());
                    } else {
                        state.chosen_categories_edit.retain(|x| *x != c.id.unwrap());
                    }
                }
            }
        });

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
            let mut remove_idx: Option<usize> = None;
            for idx in 0..state.edit_sizes.len() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{idx}"));
                    let l_ref: *mut String = &mut state.edit_sizes[idx].label;
                    let p_ref: *mut i64 = &mut state.edit_sizes[idx].price_cents;
                    unsafe {
                        ui.label("Label");
                        ui.text_edit_singleline(&mut *l_ref);
                        ui.label("Price (cents)");
                        ui.add(egui::DragValue::new(&mut *p_ref).range(0..=100_000));
                    }
                    if ui.button("Remove").clicked() { remove_idx = Some(idx); }
                });
            }
            if let Some(i) = remove_idx { if i < state.edit_sizes.len() { state.edit_sizes.remove(i); } }

            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.edit_new_size_label);
                ui.add(egui::DragValue::new(&mut state.edit_new_size_price).range(0..=100_000).prefix("Price (cents): "));
                if ui.button("Add size").clicked() && !state.edit_new_size_label.trim().is_empty() {
                    state.edit_sizes.push(PizzaSize { label: state.edit_new_size_label.clone(), price_cents: state.edit_new_size_price });
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
                        state.chosen_categories_edit.clone(),
                    ));
                } else {
                    let _ = rt.block_on(dishes::update_plain(
                        db,
                        eid,
                        &state.edit_name,
                        if state.edit_number.trim().is_empty() { None } else { Some(state.edit_number.trim().to_string()) },
                        state.edit_price,
                        state.chosen_categories_edit.clone(),
                    ));
                }
                state.edit_id = None;
                state.edit_name.clear();
                state.edit_number.clear();
                state.edit_sizes.clear();
                state.edit_price = 0;
                state.chosen_categories_edit.clear();
            }
            if ui.button("Cancel").clicked() {
                state.edit_id = None;
                state.edit_name.clear();
                state.edit_number.clear();
                state.edit_sizes.clear();
                state.edit_price = 0;
                state.chosen_categories_edit.clear();
            }
        });
    }
}

/* ---------------- Categories page ---------------- */

fn page_categories(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut AdminState,
) {
    use crate::services::{categories, suppliers};

    ui.heading("Categories");

    // 1) Supplier wählen
    let sups = rt.block_on(suppliers::list(db)).unwrap_or_default();
    if sups.is_empty() {
        ui.label("No suppliers yet.");
        return;
    }
    if state.set_supplier_idx >= sups.len() {
        state.set_supplier_idx = 0;
    }

    egui::ComboBox::from_label("Supplier")
        .selected_text(sups[state.set_supplier_idx].name.clone())
        .show_ui(ui, |cb| {
            for (i, s) in sups.iter().enumerate() {
                cb.selectable_value(&mut state.set_supplier_idx, i, s.name.clone());
            }
        });

    let sid = sups[state.set_supplier_idx].id.unwrap();

    ui.separator();

    // 2) Neue Category anlegen
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.cat_new_name);
        if ui.button("Add category").clicked() {
            let name = state.cat_new_name.trim();
            if !name.is_empty() {
                let _ = rt.block_on(categories::create(db, sid, name));
                state.cat_new_name.clear();
            }
        }
    });

    ui.separator();
    ui.label("Existing categories");

    // 3) Liste anzeigen
    let cats = rt.block_on(categories::list_by_supplier(db, sid)).unwrap_or_default();

    for c in &cats {
        ui.horizontal(|ui| {
            ui.monospace(format!("#{} {}", c.position, c.name));

            if ui.button("Edit").clicked() {
                state.cat_edit_id = c.id;
                state.cat_edit_name = c.name.clone();
                state.cat_edit_pos = c.position;
            }

            if ui.button("Delete").clicked() {
                if let Some(id) = c.id {
                    let _ = rt.block_on(categories::delete(db, id));
                }
            }
        });
    }

    // 4) Edit-Form (Position + Name)
    if let Some(edit_id) = state.cat_edit_id {
        ui.separator();
        ui.heading("Edit category");

        ui.horizontal(|ui| {
            ui.label("Position");
            ui.add(egui::DragValue::new(&mut state.cat_edit_pos).range(0..=10_000));
            ui.label("Name");
            ui.text_edit_singleline(&mut state.cat_edit_name);
        });

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                let name = state.cat_edit_name.trim().to_string();
                if !name.is_empty() {
                    let _ = rt.block_on(categories::update(db, edit_id, &name, state.cat_edit_pos));
                    // UI-State zurücksetzen
                    state.cat_edit_id = None;
                    state.cat_edit_name.clear();
                    state.cat_edit_pos = 0;
                }
            }
            if ui.button("Cancel").clicked() {
                state.cat_edit_id = None;
                state.cat_edit_name.clear();
                state.cat_edit_pos = 0;
            }
        });

        ui.label("Hinweis: Positionen werden aufsteigend sortiert. Bei gleichen Positionen entscheidet der Name.");
    }
}

/* ---------------- Settings ---------------- */

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

    ui.separator();
    ui.heading("Backup (verschlüsselt)");

    ui.horizontal(|ui| {
        ui.label("Passwort");
        ui.add(egui::TextEdit::singleline(&mut state.backup_pass).password(true));
    });

    ui.horizontal(|ui| {
        ui.label("Export-Datei");
        ui.text_edit_singleline(&mut state.backup_export_path);
        if ui.button("Export (encrypted)").clicked() {
            if state.backup_pass.is_empty() || state.backup_export_path.trim().is_empty() {
                state.backup_msg = Some((false, "Bitte Passwort und Dateipfad ausfüllen.".into()));
            } else {
                match rt.block_on(crate::services::backup::export_to_file(
                    db,
                    state.backup_export_path.trim(),
                    state.backup_pass.trim(),
                )) {
                    Ok(_) => state.backup_msg = Some((true, format!("Export erfolgreich: {}", state.backup_export_path.trim()))),
                    Err(e) => state.backup_msg = Some((false, format!("Export fehlgeschlagen: {e}"))),
                }
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Import-Datei");
        ui.text_edit_singleline(&mut state.backup_import_path);
        if ui.button("Import (encrypted)").clicked() {
            if state.backup_pass.is_empty() || state.backup_import_path.trim().is_empty() {
                state.backup_msg = Some((false, "Bitte Passwort und Dateipfad ausfüllen.".into()));
            } else {
                match rt.block_on(crate::services::backup::import_from_file(
                    db,
                    state.backup_import_path.trim(),
                    state.backup_pass.trim(),
                )) {
                    Ok(_) => state.backup_msg = Some((true, "Import erfolgreich (DB ersetzt).".into())),
                    Err(e) => state.backup_msg = Some((false, format!("Import fehlgeschlagen: {e}"))),
                }
            }
        }
    });

    if let Some((ok, msg)) = &state.backup_msg {
        let color = if *ok { egui::Color32::from_rgb(20,160,20) } else { egui::Color32::RED };
        ui.colored_label(color, msg);
    }

}

fn id_to_name(sups: &[Supplier], id: ObjectId) -> String {
    sups.iter().find(|s| s.id == Some(id)).map(|s| s.name.clone()).unwrap_or_else(|| id.to_hex())
}
