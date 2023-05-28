#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabView {
    #[default]
    Queue,
    History,
}

impl TabView {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::History => "History",
        }
    }
}
