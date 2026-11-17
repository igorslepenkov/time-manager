pub mod input;
pub mod submit_btn;

use std::fmt::Display;

use ratatui::{layout::Rect, Frame};

use crate::{i18n::t, ui::tabs::Tab};

pub use self::{input::Input, submit_btn::SubmitButton};

#[derive(Clone, PartialEq)]
pub enum Control {
    EndCommentInput(Input),
    TaskNameInput(Input),
    SubmitBtn(SubmitButton),
}

impl Display for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Control::TaskNameInput(_) => write!(f, "Task name"),
            Control::EndCommentInput(_) => write!(f, "Task end comment"),
            Control::SubmitBtn(_) => write!(f, "Submit btn"),
        }
    }
}

impl Control {
    pub fn render(&mut self, frame: &mut Frame, area: Rect, tab: Tab) {
        match self {
            Control::TaskNameInput(state) => Input::render(
                t("New task description", "Новая задача"),
                state,
                t("Enter new task description", "Введите название задачи"),
                frame,
                area,
            ),
            Control::EndCommentInput(state) => {
                let title = if tab == Tab::Start {
                    t(
                        "Comment on the previous task",
                        "Комментарий к прошлой задаче",
                    )
                } else {
                    t("Task comment", "Комментарий к задаче")
                };

                Input::render(
                    title,
                    state,
                    t("What was done (optional)", "Что сделано (необязательно)"),
                    frame,
                    area,
                )
            }
            Control::SubmitBtn(state) => {
                let label = match tab {
                    Tab::Start => t("Start task", "Начать задачу"),
                    Tab::Out => t("Pause", "Приостановить"),
                    Tab::End => t("Close day", "Закрыть день"),
                    Tab::ClearState => t("Clear day", "Сбросить день"),
                    _ => t("Submit", "Отправить"),
                };

                SubmitButton::render(state, frame, area, label)
            }
        }
    }

    pub fn clear_input(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.clear_input(),
            Control::EndCommentInput(state) => state.clear_input(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn insert_char(&mut self, char: char) {
        match self {
            Control::TaskNameInput(state) => state.insert_char(char),
            Control::EndCommentInput(state) => state.insert_char(char),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn insert_text(&mut self, text: &str) {
        match self {
            Control::TaskNameInput(state) => state.insert_text(text),
            Control::EndCommentInput(state) => state.insert_text(text),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn backspace(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.backspace(),
            Control::EndCommentInput(state) => state.backspace(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn delete_forward(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.delete_forward(),
            Control::EndCommentInput(state) => state.delete_forward(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn delete_word_backward(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.delete_word_backward(),
            Control::EndCommentInput(state) => state.delete_word_backward(),
            Control::SubmitBtn(_) => {}
        }
    }

    /// true, если у контрола есть текст, по которому можно двигать курсор.
    pub fn is_text_input(&self) -> bool {
        matches!(
            self,
            Control::TaskNameInput(_) | Control::EndCommentInput(_)
        )
    }

    pub fn move_cursor_left(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.move_cursor_left(),
            Control::EndCommentInput(state) => state.move_cursor_left(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn move_cursor_right(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.move_cursor_right(),
            Control::EndCommentInput(state) => state.move_cursor_right(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn move_cursor_home(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.move_cursor_home(),
            Control::EndCommentInput(state) => state.move_cursor_home(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn move_cursor_end(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.move_cursor_end(),
            Control::EndCommentInput(state) => state.move_cursor_end(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn set_focus(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.set_focus(),
            Control::EndCommentInput(state) => state.set_focus(),
            Control::SubmitBtn(state) => state.set_focus(),
        }
    }

    pub fn unset_focus(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.unset_focus(),
            Control::EndCommentInput(state) => state.unset_focus(),
            Control::SubmitBtn(state) => state.unset_focus(),
        }
    }
}
