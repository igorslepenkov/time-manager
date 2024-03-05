pub mod control;
pub mod tabs;

use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use itertools::Itertools;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    Frame,
};

use crate::{theme::THEME, utils::centered_rect};
use tabs::Tab;

pub use control::{Control, Input};

use self::control::SubmitButton;

pub enum AppStage {
    Waiting,
    Working,
    Paused,
}

impl Display for AppStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppStage::Waiting => write!(f, "Waiting for start"),
            AppStage::Working => write!(f, "Working on task"),
            AppStage::Paused => write!(f, "Track paused"),
        }
    }
}

pub struct AppUiState {
    submit_btn: Arc<Mutex<Control>>,
    active_tab: usize,
    pub tabs: Vec<String>,
    pub control_focused: Option<Arc<Mutex<Control>>>,
    pub task_name_input: Arc<Mutex<Control>>,
    pub task_end_comment_input: Arc<Mutex<Control>>,
    pub stage: AppStage,
}

impl AppUiState {
    pub fn init() -> AppUiState {
        AppUiState {
            task_end_comment_input: Arc::new(Mutex::new(Control::EndCommentInput(Input::init()))),
            task_name_input: Arc::new(Mutex::new(Control::TaskNameInput(Input::init()))),
            submit_btn: Arc::new(Mutex::new(Control::SubmitBtn(SubmitButton::init()))),
            active_tab: 0_usize,
            tabs: Tab::as_string_vec(),
            control_focused: None,
            stage: AppStage::Waiting,
        }
    }

    pub fn switch_tabs_forward(&mut self) {
        let tabs = &self.tabs;
        let active_tab = &self.active_tab;

        let new_tab_idx = active_tab + 1;
        let new_tab_option = tabs.get(new_tab_idx);

        if let Some(tab) = new_tab_option {
            let tab = tab.to_owned();

            self.clear_inputs_state();

            self.active_tab = new_tab_idx;

            if tab == Tab::Start.to_string() {
                self.control_focused = Some(Arc::clone(&self.task_name_input));
                self.task_name_input.lock().unwrap().set_focus();
            } else if tab == Tab::End.to_string() || tab == Tab::Out.to_string() {
                self.control_focused = Some(Arc::clone(&self.task_end_comment_input));
                self.task_end_comment_input.lock().unwrap().set_focus();
            } else if tab == Tab::ClearState.to_string() {
                self.control_focused = Some(Arc::clone(&self.submit_btn));
                self.submit_btn.lock().unwrap().set_focus();
            } else {
                self.control_focused = None
            }
        }
    }

    pub fn switch_tabs_backward(&mut self) {
        let tabs = &self.tabs;
        let active_tab = &self.active_tab;

        if *active_tab == 0_usize {
            return;
        }

        let new_tab_idx = active_tab - 1;
        let new_tab_option = tabs.get(new_tab_idx);

        if let Some(tab) = new_tab_option {
            let tab = tab.to_owned();

            self.clear_inputs_state();

            self.active_tab = new_tab_idx;

            if tab == Tab::Start.to_string() {
                self.control_focused = Some(Arc::clone(&self.task_name_input));
                self.task_name_input.lock().unwrap().toggle_focus();
            } else if tab == Tab::End.to_string() {
                self.control_focused = Some(Arc::clone(&self.task_end_comment_input));
                self.task_end_comment_input.lock().unwrap().toggle_focus();
            } else if tab == Tab::Out.to_string() {
                self.control_focused = Some(Arc::clone(&self.task_end_comment_input));
                self.task_end_comment_input.lock().unwrap().set_focus();
            } else if tab == Tab::ClearState.to_string() {
                self.control_focused = Some(Arc::clone(&self.submit_btn));
                self.submit_btn.lock().unwrap().set_focus();
            } else {
                self.control_focused = None
            }
        }
    }

    pub fn switch_control_focus_forwards(&mut self) {
        let active_tab = self.get_active_tab();

        match active_tab {
            Tab::Start => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::TaskNameInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_name_input));

                            state.unset_focus();
                            self.task_name_input.lock().unwrap().set_focus();
                        }
                    }
                }
            }
            Tab::End | Tab::Out => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub fn switch_control_focus_backwards(&mut self) {
        let active_tab = self.get_active_tab();

        match active_tab {
            Tab::Start => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::TaskNameInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_name_input));

                            state.unset_focus();
                            self.task_name_input.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                    }
                }
            }
            Tab::End | Tab::Out => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub fn clear_inputs_state(&mut self) {
        self.task_name_input.lock().unwrap().clear_input();
        self.task_end_comment_input.lock().unwrap().clear_input();
        self.submit_btn.lock().unwrap().unset_focus();
    }

    pub fn get_active_tab(&self) -> Tab {
        let current_active_idx = self.active_tab;

        if current_active_idx == Tab::Home as usize {
            Tab::Home
        } else if current_active_idx == Tab::Start as usize {
            Tab::Start
        } else if current_active_idx == Tab::Out as usize {
            Tab::Out
        } else if current_active_idx == Tab::End as usize {
            Tab::End
        } else if current_active_idx == Tab::ClearState as usize {
            Tab::ClearState
        } else {
            Tab::Home
        }
    }

    pub fn get_active_tab_idx(&self) -> usize {
        self.active_tab
    }

    pub fn render_home_tab(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = vec![];

        lines.push(Line::from(vec![Span::styled(
            "Welcome to the time manager tool!",
            Style::new().yellow().bold(),
        )]));

        lines.push(
          Line::from(
            vec![
              Span::styled(
                "With this tool you can manage your working hours and concentrate on your work while manager takes care of creating report of your activity with a little bit of your help of course",
                Style::default().light_blue()
              )
            ]
          )
        );

        lines.push(Line::from(vec![Span::styled(
            "Here are some usefull keys:",
            Style::default().blue().bold(),
        )]));

        let keys = [
            ("←", "Left"),
            ("→", "Right"),
            ("↑", "Up"),
            ("↓", "Down"),
            ("Esc", "Quit"),
        ];
        let spans = keys
            .iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(format!(" {} ", key), THEME.key_binding.key);
                let desc = Span::styled(format!(" {} ", desc), THEME.key_binding.description);
                [key, desc]
            })
            .collect_vec();

        lines.push(
            Line::from(spans)
                .centered()
                .style((Color::Indexed(236), Color::Indexed(232))),
        );

        let text = Paragraph::new(Text::from(lines))
            .alignment(Alignment::Center)
            .wrap(Wrap::default())
            .block(
                Block::new()
                    .borders(Borders::empty())
                    .padding(Padding::new(2, 2, 2, 2)),
            );

        frame.render_widget(text, area)
    }

    pub fn render_start_tab(&self, frame: &mut Frame, area: Rect) {
        let inner_area = centered_rect(area, 90, 70);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(37),
                Constraint::Percentage(38),
                Constraint::Percentage(25),
            ])
            .split(inner_area);

        self.task_name_input
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[0]);

        self.task_end_comment_input
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[1]);
        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[2])
    }

    pub fn render_out_tab(&self, frame: &mut Frame, area: Rect) {
        let inner_area = centered_rect(area, 90, 70);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(20),
                Constraint::Percentage(50),
                Constraint::Percentage(30),
            ])
            .split(inner_area);

        let text = Text::from("Do you want to stop tracking?");
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, area_vertical_layouts[0]);

        let task_end_comment_rect = centered_rect(area_vertical_layouts[1], 100, 80);

        self.task_end_comment_input
            .lock()
            .unwrap()
            .render(frame, task_end_comment_rect);

        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[2])
    }

    pub fn render_end_tab(&self, frame: &mut Frame, area: Rect) {
        let inner_area = centered_rect(area, 90, 70);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(inner_area);

        let inner_bottom_area = centered_rect(area_vertical_layouts[1], 50, 50);

        self.task_end_comment_input
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[0]);
        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, inner_bottom_area)
    }

    pub fn render_clear_tab(&self, frame: &mut Frame, area: Rect) {
        let inner_area = centered_rect(area, 90, 70);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(40),
            ])
            .split(inner_area);

        let text = Text::from("Do you want to clear app state?");
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, area_vertical_layouts[0]);

        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[1])
    }
}
