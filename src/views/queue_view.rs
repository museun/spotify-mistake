use std::collections::VecDeque;

use crate::{
    db,
    views::list_view::{Action, ListView},
    Request,
};

pub struct QueueView<'a> {
    pub list_view: ListView<'a>,
    pub queue: &'a mut VecDeque<Request>,
    pub db: &'a db::Connection,
}

impl<'a> QueueView<'a> {
    pub fn display(self, ui: &mut egui::Ui) {
        if let Action::Remove { index } = self.list_view.display(
            ui,
            "Nothing is queued",
            self.queue.is_empty(),
            |ui, add, req| {},
            self.queue.iter(),
        ) {
            if let Some(item) = self.queue.remove(index) {
                self.db.remove_from_queue(&item);
            }
        }
    }
}
