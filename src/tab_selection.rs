#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabSelection {
    #[default]
    Queue,
    History,
}

impl TabSelection {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::History => "History",
        }
    }
}
