use std::fmt::Display;

use ratatui::text::Line;

#[derive(Eq, PartialEq)]
pub enum Tab {
    Home,
    Start,
    Out,
    End,
    ClearState,
}

impl From<Tab> for Line<'_> {
    fn from(val: Tab) -> Self {
        match val {
            Tab::Home => Line::raw(Tab::Home.to_string()),
            Tab::Start => Line::raw(Tab::Start.to_string()),
            Tab::Out => Line::raw(Tab::Out.to_string()),
            Tab::End => Line::raw(Tab::Out.to_string()),
            Tab::ClearState => Line::raw(Tab::ClearState.to_string()),
        }
    }
}

impl Tab {
    pub fn as_string_vec() -> Vec<String> {
        vec![
            Tab::Home.to_string(),
            Tab::Start.to_string(),
            Tab::Out.to_string(),
            Tab::End.to_string(),
            Tab::ClearState.to_string(),
        ]
    }
}

impl Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tab::Home => write!(f, "Home"),
            Tab::Start => write!(f, "Start"),
            Tab::Out => write!(f, "Out"),
            Tab::End => write!(f, "End"),
            Tab::ClearState => write!(f, "Clear State"),
        }
    }
}
