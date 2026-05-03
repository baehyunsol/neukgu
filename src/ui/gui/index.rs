use iced::{Element, Task};
use iced::keyboard::{Key, Modifiers};
use iced::widget::text;

// TODO
// 1. recent projects
// 2. create new
// 3. current tabs

pub struct IcedContext {}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    KeyPressed { key: Key, modifiers: Modifiers },
}

pub fn boot() -> IcedContext {
    IcedContext {}
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    match message {
        IcedMessage::KeyPressed { key, modifiers } => {
            Task::none()
        },
    }
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    text!("TODO").into()
}
