use std::collections::VecDeque;

use crate::{
    db, history,
    views::list_view::{Action, ListView},
    Request,
};

pub struct HistoryView<'a> {
    pub list_view: ListView<'a>,
    pub history: &'a mut history::History,
    pub queue: &'a mut VecDeque<Request>,
    pub db: &'a db::Connection,
}

impl<'a> HistoryView<'a> {
    pub fn display(self, ui: &mut egui::Ui) {
        match self.list_view.display(
            ui,
            "History is empty",
            self.history.requests.is_empty(),
            |ui, add, req| {
                if ui.small_button("➕").clicked() {
                    add.replace(req.clone());
                }
            },
            self.history.requests.iter(),
        ) {
            Action::Add { request } => {
                self.db.queue(&request);
                self.queue.push_back(request);
            }
            Action::Remove { index } => {
                let req = self.history.requests.remove(index);
                self.db.remove_from_history(&req);
            }
            Action::Nothing => {}
        }
    }
}
