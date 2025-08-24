// src/ui/order.rs
use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::model::{Category, Dish};
use crate::services::{categories, dishes, orders, settings, suppliers};

#[derive(Clone)]
pub(crate) struct ItemSel {
    pub(crate) dish_idx: usize,         // Index in state.dishes (global)
    pub(crate) qty: i32,
    pub(crate) size_idx: Option<usize>, // nur für Pizza
    pub(crate) note: String,            // optional
}

#[derive(Default)]
pub struct OrderState {
    pub supplier_name: String,
    pub delivery_fee_cents: i64,
    pub supplier_id: Option<ObjectId>,

    pub dishes: Vec<Dish>,
    pub categories: Vec<Category>,
    pub active_category: Option<ObjectId>, // None = All

    pub(crate) selections: Vec<ItemSel>,
    pub customer_name: String,
    pub client_id: String,

    pub load_err: Option<String>,
    pub loaded: bool,
}

/* ---------- helpers ---------- */

fn eur(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}€{}.{}", abs / 100, format!("{:02}", abs % 100))
}

fn dish_sort_key(d: &Dish) -> (i32, i64, String) {
    if let Some(nr) = &d.number {
        let digits: String = nr.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(v) = digits.parse::<i64>() {
            return (0, v, d.name.clone());
        }
    }
    (1, i64::MAX, d.name.clone())
}

fn dish_label(d: &Dish) -> String {
    let nr = d.number.clone().unwrap_or_default();
    let base = if nr.is_empty() {
        d.name.clone()
    } else {
        format!("Nr. {}: {}", nr, d.name)
    };
    if d.tags.iter().any(|t| t == "Pizza") {
        base
    } else {
        format!("{} ({})", base, eur(d.price_cents))
    }
}

// Indizes der Gerichte, die zur aktiven Kategorie passen (oder alle)
fn filtered_indices(state: &OrderState) -> Vec<usize> {
    match state.active_category {
        None => (0..state.dishes.len()).collect(),
        Some(cid) => state
            .dishes
            .iter()
            .enumerate()
            .filter(|(_, d)| d.categories.iter().any(|x| *x == cid))
            .map(|(i, _)| i)
            .collect(),
    }
}

/* ---------- UI ---------- */

pub fn render(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut OrderState,
) {
    // Initial laden
    if !state.loaded && state.load_err.is_none() {
        let res = rt.block_on(async {
            if let Some(sid) = settings::get_active_supplier_id(db).await? {
                if let Some(supp) = suppliers::get(db, sid).await? {
                    let mut ds = dishes::list_by_supplier(db, sid).await?;
                    // sortieren
                    ds.sort_by_key(dish_sort_key);

                    let cats = categories::list_by_supplier(db, sid).await?;

                    Ok::<_, anyhow::Error>((
                        Some(sid),
                        supp.name,
                        supp.delivery_fee_cents,
                        ds,
                        cats,
                    ))
                } else {
                    anyhow::bail!("Active supplier not found");
                }
            } else {
                anyhow::bail!("No active supplier in settings");
            }
        });

        match res {
            Ok((sid, name, fee, ds, cats)) => {
                state.supplier_id = sid;
                state.supplier_name = name;
                state.delivery_fee_cents = fee;
                state.dishes = ds;
                state.categories = cats;
                // Default: All
                state.active_category = None;

                if state.selections.is_empty() {
                    state.selections.push(ItemSel {
                        dish_idx: 0,
                        qty: 1,
                        size_idx: None,
                        note: String::new(),
                    });
                }
                state.loaded = true;
            }
            Err(e) => state.load_err = Some(e.to_string()),
        }
    }

    ui.heading("Place your order");

    if let Some(err) = &state.load_err {
        ui.colored_label(egui::Color32::RED, err);
        ui.label("Admin must set an active supplier and menu.");
        return;
    }
    if !state.loaded {
        ui.label("Loading…");
        return;
    }
    if state.dishes.is_empty() {
        ui.label("No dishes available.");
        return;
    }

    ui.label(format!("Supplier: {}", state.supplier_name));
    ui.label(format!("Delivery fee: {}", eur(state.delivery_fee_cents)));

    ui.separator();
    ui.label("Your name");
    ui.text_edit_singleline(&mut state.customer_name);

    ui.separator();
    ui.horizontal(|ui| {
        // + / − Buttons
        if ui.button("+ Add dish").clicked() {
            // Voreinstellung: erste passende Option der aktiven Kategorie (falls vorhanden)
            let f = filtered_indices(state);
            let fallback = *f.get(0).unwrap_or(&0);
            let last_idx = state.selections.last().map(|s| s.dish_idx).unwrap_or(fallback);
            state.selections.push(ItemSel {
                dish_idx: last_idx,
                qty: 1,
                size_idx: None,
                note: String::new(),
            });
        }
        if ui.button("− Remove last").clicked() && state.selections.len() > 1 {
            state.selections.pop();
        }
    });

    ui.separator();
    ui.label("Dishes");

    // Kategorie-Tabs
    ui.horizontal_wrapped(|ui| {
        // "All" Tab
        ui.selectable_value(&mut state.active_category, None, "All");
        // supplier-spezifische Kategorien
        for c in &state.categories {
            if let Some(cid) = c.id {
                ui.selectable_value(&mut state.active_category, Some(cid), c.name.clone());
            }
        }
    });

    let filtered = filtered_indices(state);
    if filtered.is_empty() {
        ui.label("No dishes in this category.");
    }

    // Selektionen rendern
    for (i, sel) in state.selections.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Gericht-Combo: nur gefilterte Optionen anzeigen
                    ui.label(format!("Dish #{}", i + 1));

                    // Aktueller Text für ausgewähltes Gericht
                    let current_label = dish_label(&state.dishes[sel.dish_idx]);

                    egui::ComboBox::from_id_salt(("dish_select", i))
                        .selected_text(current_label)
                        .show_ui(ui, |cb| {
                            for idx in &filtered {
                                let d = &state.dishes[*idx];
                                cb.selectable_value(&mut sel.dish_idx, *idx, dish_label(d));
                            }
                        });

                    // Größe (nur Pizza)
                    let d = &state.dishes[sel.dish_idx];
                    if let Some(sizes) = &d.pizza_sizes {
                        if sel.size_idx.is_none() && !sizes.is_empty() {
                            sel.size_idx = Some(0);
                        }
                        let sidx = sel.size_idx.unwrap_or(0).min(sizes.len().saturating_sub(1));
                        let scur = &sizes[sidx];

                        ui.label("Size");
                        egui::ComboBox::from_id_salt(("size_select", i))
                            .selected_text(format!("{} ({})", scur.label, eur(scur.price_cents)))
                            .show_ui(ui, |cb| {
                                for (j, s) in sizes.iter().enumerate() {
                                    cb.selectable_value(
                                        &mut sel.size_idx,
                                        Some(j),
                                        format!("{} ({})", s.label, eur(s.price_cents)),
                                    );
                                }
                            });
                    } else {
                        ui.monospace(format!("Unit: {}", eur(d.price_cents)));
                    }

                    // Menge
                    ui.add(
                        egui::DragValue::new(&mut sel.qty)
                            .range(1..=20)
                            .prefix("Qty: "),
                    );
                });

                // Notiz
                ui.horizontal(|ui| {
                    ui.label("Note (optional)");
                    ui.text_edit_singleline(&mut sel.note);
                });

                // Zeilensumme
                let d = &state.dishes[sel.dish_idx];
                let unit = if let Some(sizes) = &d.pizza_sizes {
                    let idx = sel.size_idx.unwrap_or(0).min(sizes.len().saturating_sub(1));
                    sizes[idx].price_cents
                } else {
                    d.price_cents
                } as i64;
                let line_total = unit * (sel.qty as i64);
                ui.monospace(format!("Line total: {}", eur(line_total)));
            });
        });
    }

    // Summary
    let items_total: i64 = state
        .selections
        .iter()
        .map(|s| {
            let d = &state.dishes[s.dish_idx];
            let unit = if let Some(sizes) = &d.pizza_sizes {
                let idx = s.size_idx.unwrap_or(0).min(sizes.len().saturating_sub(1));
                sizes[idx].price_cents
            } else {
                d.price_cents
            } as i64;
            unit * (s.qty as i64)
        })
        .sum();

    let grand_total = items_total + state.delivery_fee_cents;

    ui.separator();
    ui.label("Summary");
    ui.monospace(format!("Items total: {}", eur(items_total)));
    ui.monospace(format!("Delivery fee: {}", eur(state.delivery_fee_cents)));
    ui.monospace(format!("Grand total: {}", eur(grand_total)));

    let can_submit = state.supplier_id.is_some()
        && !state.customer_name.trim().is_empty()
        && !state.selections.is_empty();

    if ui.add_enabled(can_submit, egui::Button::new("Submit order")).clicked() {
        if let Some(supplier_id) = state.supplier_id {
            // Items mit Namen/Nummer/Größe/Notiz aufbereiten
            let items: Vec<(ObjectId, String, i32, i64, Option<String>, Option<String>)> = state
                .selections
                .iter()
                .map(|s| {
                    let d = &state.dishes[s.dish_idx];
                    if let Some(sizes) = &d.pizza_sizes {
                        let idx = s.size_idx.unwrap_or(0).min(sizes.len().saturating_sub(1));
                        let sz = &sizes[idx];
                        let nr = d.number.clone().unwrap_or_default();
                        let base = if nr.is_empty() {
                            d.name.clone()
                        } else {
                            format!("Nr. {}: {}", nr, d.name)
                        };
                        let name = format!("{} ({})", base, sz.label);
                        (
                            d.id.unwrap(),
                            name,
                            s.qty,
                            sz.price_cents as i64,
                            if s.note.trim().is_empty() { None } else { Some(s.note.clone()) },
                            Some(sz.label.clone()),
                        )
                    } else {
                        let nr = d.number.clone().unwrap_or_default();
                        let base = if nr.is_empty() {
                            d.name.clone()
                        } else {
                            format!("Nr. {}: {}", nr, d.name)
                        };
                        (
                            d.id.unwrap(),
                            base,
                            s.qty,
                            d.price_cents as i64,
                            if s.note.trim().is_empty() { None } else { Some(s.note.clone()) },
                            None,
                        )
                    }
                })
                .collect();

            let res = rt.block_on(orders::create_with_notes(
                db,
                &state.customer_name,
                supplier_id,
                items,
                state.delivery_fee_cents,
                &state.client_id,
            ));

            match res {
                Ok(_) => {
                    state.selections.clear();
                    state.selections.push(ItemSel {
                        dish_idx: 0,
                        qty: 1,
                        size_idx: None,
                        note: String::new(),
                    });
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::RED, format!("Failed to submit: {e}"));
                }
            }
        }
    }
}
