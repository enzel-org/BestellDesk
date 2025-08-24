use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::model::Dish;
use crate::services::{dishes, orders, settings, suppliers};

#[derive(Clone)]
pub(crate) struct ItemSel {
    pub(crate) dish_idx: usize,
    pub(crate) qty: i32,
    pub(crate) size_idx: Option<usize>, // only for Pizza
    pub(crate) note: String,            // optional
}

#[derive(Default)]
pub struct OrderState {
    pub supplier_name: String,
    pub delivery_fee_cents: i64,
    pub supplier_id: Option<ObjectId>,
    pub dishes: Vec<Dish>,

    pub(crate) selections: Vec<ItemSel>,
    pub customer_name: String,
    pub client_id: String,

    pub load_err: Option<String>,
    pub loaded: bool,
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
    state: &mut OrderState,
) {
    // Lazy-load once
    if !state.loaded && state.load_err.is_none() {
        let res = rt.block_on(async {
            if let Some(sid) = settings::get_active_supplier_id(db).await? {
                if let Some(supp) = suppliers::get(db, sid).await? {
                    // ^^^^^^^^^^^^^ fix: suppliers::get(...) statt get_supplier(...)
                    let ds = dishes::list_by_supplier(db, sid).await?;
                    Ok::<_, anyhow::Error>((Some(sid), supp.name, supp.delivery_fee_cents, ds))
                } else {
                    anyhow::bail!("Active supplier not found");
                }
            } else {
                anyhow::bail!("No active supplier in settings");
            }
        });
        match res {
            Ok((sid, name, fee, ds)) => {
                state.supplier_id = sid;
                state.supplier_name = name;
                state.delivery_fee_cents = fee;
                state.dishes = ds;
                if state.selections.is_empty() {
                    state
                        .selections
                        .push(ItemSel { dish_idx: 0, qty: 1, size_idx: None, note: String::new() });
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
        if ui.button("+ Add dish").clicked() {
            let last_idx = state.selections.last().map(|s| s.dish_idx).unwrap_or(0);
            state
                .selections
                .push(ItemSel { dish_idx: last_idx, qty: 1, size_idx: None, note: String::new() });
        }
        if ui.button("− Remove last").clicked() && state.selections.len() > 1 {
            state.selections.pop();
        }
    });

    ui.separator();
    ui.label("Dishes");

    for (i, sel) in state.selections.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                let current = &state.dishes[sel.dish_idx];
                let show_name = if current.tags.iter().any(|t| t == "Pizza") {
                    let nr = current.number.clone().unwrap_or_default();
                    format!("{} {}", nr, current.name)
                } else {
                    current.name.clone()
                };
                egui::ComboBox::from_label(format!("Dish #{}", i + 1))
                    .selected_text(show_name)
                    .show_ui(ui, |cb| {
                        for (idx, d) in state.dishes.iter().enumerate() {
                            let label = if d.tags.iter().any(|t| t == "Pizza") {
                                let nr = d.number.clone().unwrap_or_default();
                                format!("{} {}", nr, d.name)
                            } else {
                                d.name.clone()
                            };
                            cb.selectable_value(&mut sel.dish_idx, idx, label);
                        }
                    });

                let d = &state.dishes[sel.dish_idx];
                if let Some(sizes) = &d.pizza_sizes {
                    if sel.size_idx.is_none() && !sizes.is_empty() {
                        sel.size_idx = Some(0);
                    }
                    let curr_idx = sel.size_idx.unwrap_or(0).min(sizes.len().saturating_sub(1));
                    let curr = &sizes[curr_idx];
                    egui::ComboBox::from_label("Size")
                        .selected_text(format!("{} ({})", curr.label, eur(curr.price_cents)))
                        .show_ui(ui, |cb| {
                            for (sidx, s) in sizes.iter().enumerate() {
                                cb.selectable_value(
                                    &mut sel.size_idx,
                                    Some(sidx),
                                    format!("{} ({})", s.label, eur(s.price_cents)),
                                );
                            }
                        });
                } else {
                    ui.monospace(format!("Unit: {}", eur(d.price_cents)));
                }

                ui.add(egui::DragValue::new(&mut sel.qty).range(1..=20).prefix("Qty: "));
            });

            ui.horizontal(|ui| {
                ui.label("Note (optional)");
                ui.text_edit_singleline(&mut sel.note);
            });

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
    }

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
            let items: Vec<(ObjectId, String, i32, i64, Option<String>, Option<String>)> =
                state
                    .selections
                    .iter()
                    .map(|s| {
                        let d = &state.dishes[s.dish_idx];
                        if let Some(sizes) = &d.pizza_sizes {
                            let idx =
                                s.size_idx.unwrap_or(0).min(sizes.len().saturating_sub(1));
                            let sz = &sizes[idx];
                            let nr = d.number.clone().unwrap_or_default();
                            let name = format!("{} {} ({})", nr, d.name, sz.label);
                            (
                                d.id.unwrap(),
                                name,
                                s.qty,
                                sz.price_cents as i64,
                                if s.note.trim().is_empty() {
                                    None
                                } else {
                                    Some(s.note.clone())
                                },
                                Some(sz.label.clone()),
                            )
                        } else {
                            (
                                d.id.unwrap(),
                                d.name.clone(),
                                s.qty,
                                d.price_cents as i64,
                                if s.note.trim().is_empty() {
                                    None
                                } else {
                                    Some(s.note.clone())
                                },
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
