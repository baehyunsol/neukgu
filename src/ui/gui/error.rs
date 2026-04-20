use iced::Size;

#[derive(Clone, Debug)]
pub struct IcedContext {
    pub window_size: Size,
}

#[derive(Clone, Debug)]
pub enum IcedMessage {
    WindowResized(Size),
}
