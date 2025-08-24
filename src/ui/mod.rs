use eframe::egui;

pub mod order;
pub mod admin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTab { Order, Admin }
impl Default for UiTab { fn default() -> Self { UiTab::Order } }

pub fn render_order(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    state: &mut order::OrderState,
) {
    order::render(ui, rt, db, state);
}

pub fn render_admin(
    ui: &mut egui::Ui,
    rt: &tokio::runtime::Runtime,
    db: &crate::db::Db,
    user: &mut String,
    pass: &mut String,
    authed: &mut bool,
    state: &mut admin::AdminState,
) {
    admin::render(ui, rt, db, user, pass, authed, state);
}
