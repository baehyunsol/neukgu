use iced::{Element, Task};
use iced::widget::text;

pub struct IcedContext {}

#[derive(Clone, Debug)]
pub enum IcedMessage {}

pub fn boot() -> IcedContext {
    IcedContext {}
}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Task<IcedMessage> {
    todo!()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    text!("TODO").into()
}
