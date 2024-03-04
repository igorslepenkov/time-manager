pub mod input;
pub mod submit_btn;

use ratatui::{layout::Rect, Frame};

pub use self::{input::Input, submit_btn::SubmitButton};

#[derive(Clone)]
pub enum Control {
    EndCommentInput(Input),
    TaskNameInput(Input),
    SubmitBtn(SubmitButton),
}

impl Control {
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self {
            Control::TaskNameInput(state) => {
                let title = "New task description";
                let placeholder = "Enter new task description";

                Input::render(title, state, placeholder, frame, area)
            }
            Control::EndCommentInput(state) => {
                let title = "Previous task comment";
                let placeholder = "Enter comment";

                Input::render(title, state, placeholder, frame, area)
            }
            Control::SubmitBtn(state) => SubmitButton::render(state, frame, area),
        }
    }

    pub fn clear_input(&mut self) -> () {
        match self {
            Control::TaskNameInput(state) => state.clear_input(),
            Control::EndCommentInput(state) => state.clear_input(),
            Control::SubmitBtn(_) => {}
        }
    }

    pub fn toggle_focus(&mut self) {
        match self {
            Control::TaskNameInput(state) => state.toggle_focus(),
            Control::EndCommentInput(state) => state.toggle_focus(),
            Control::SubmitBtn(state) => state.toggle_focus(),
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
