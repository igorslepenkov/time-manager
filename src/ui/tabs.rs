use ratatui::text::Line;

#[derive(Eq, PartialEq)]
pub enum Tab {
    Home,
    Start,
    Out,
    End,
    ClearState,
}

impl Into<Line<'_>> for Tab {
    fn into(self) -> Line<'static> {
        match self {
            Tab::Home => Line::raw(Tab::Home.to_string()),
            Tab::Start => Line::raw(Tab::Start.to_string()),
            Tab::Out => Line::raw(Tab::Out.to_string()),
            Tab::End => Line::raw(Tab::Out.to_string()),
            Tab::ClearState => Line::raw(Tab::ClearState.to_string()),
        }
    }
}

impl Tab {
    pub fn to_string(&self) -> String {
        match self {
            Tab::Home => String::from("Home"),
            Tab::Start => String::from("Start"),
            Tab::Out => String::from("Out"),
            Tab::End => String::from("End"),
            Tab::ClearState => String::from("Clear State"),
        }
    }

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
