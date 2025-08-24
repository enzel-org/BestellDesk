use eframe::egui;
use mongodb::bson::oid::ObjectId;

use crate::services::{dishes, orders, settings, suppliers};
use crate::model::Dish;

/// One selected dish row in the UI.
#[derive(Clone)]
struct ItemSel {
    dish_idx: usize,
    qty: i32,
}

/// Local UI state for the order screen.
#[derive(Default)]
pub struct OrderState {
    pub supplier_name: String,
    pub delivery_fee_cents: i64,
    pub supplier_id: Option<ObjectId>,
    pub dishes: Vec<Dish>,

    // Multiple dish selections
    pub selections: Vec<ItemSel>,

    // Who is this order for
    pub customer_name: String,

    // Injected from main.rs
    pub client_id: String,

    pub load_err: Option<String>,
    pub loaded: bool,
}

/// Format cents to "€X.YY".
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
    // Lazy-load supplier + dishes once
    if !state.loaded && state.load_err.is_none() {
        let res = rt.block_on(async {
            if let Some(sid) = settings::get_active_supplier_id(db).await? {
                if let Some(supp) = suppliers::get_supplier(db, sid).await? {
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
                    state.selections.push(ItemSel { dish_idx: 0, qty: 1 });
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
            state.selections.push(ItemSel { dish_idx: last_idx, qty: 1 });
        }
        if ui.button("− Remove last").clicked() {
            if state.selections.len() > 1 {
                state.selections.pop();
            }
        }
    });

    ui.separator();
    ui.label("Dishes");

    // Render each selection row
    for (i, sel) in state.selections.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // Dish selector
                let current = &state.dishes[sel.dish_idx];
                egui::ComboBox::from_label(format!("Dish #{}", i + 1))
                    .selected_text(format!("{} ({})", current.name, eur(current.price_cents)))
                    .show_ui(ui, |cb| {
                        for (idx, d) in state.dishes.iter().enumerate() {
                            cb.selectable_value(
                                &mut sel.dish_idx,
                                idx,
                                format!("{} ({})", d.name, eur(d.price_cents)),
                            );
                        }
                    });

                // Quantity control
                ui.add(egui::DragValue::new(&mut sel.qty).range(1..=20).prefix("Qty: "));
            });

            // Line total
            let d = &state.dishes[sel.dish_idx];
            let line_total = (d.price_cents as i64) * (sel.qty as i64);
            ui.monospace(format!("Line total: {}", eur(line_total)));
        });
    }

    // Summary
    let items_total: i64 = state
        .selections
        .iter()
        .map(|s| {
            let d = &state.dishes[s.dish_idx];
            (d.price_cents as i64) * (s.qty as i64)
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
            // Build items payload
            let items: Vec<(ObjectId, String, i32, i64)> = state
                .selections
                .iter()
                .map(|s| {
                    let d = &state.dishes[s.dish_idx];
                    (d.id.unwrap(), d.name.clone(), s.qty, d.price_cents as i64)
                })
                .collect();

            let res = rt.block_on(orders::create(
                db,
                &state.customer_name,
                supplier_id,
                items,
                state.delivery_fee_cents,
                &state.client_id, // include client_id in DB order
            ));

            match res {
                Ok(_) => {
                    // Reset selections (keep name)
                    state.selections.clear();
                    state.selections.push(ItemSel { dish_idx: 0, qty: 1 });
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::RED, format!("Failed to submit: {e}"));
                }
            }
        }
    }
}
