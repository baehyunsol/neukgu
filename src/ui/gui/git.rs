use super::worker::JobId;
use crate::Error;
use iced::{Element, Task};

#[derive(Clone, Debug)]
pub struct IcedContext {}

impl IcedContext {
    pub fn new(path: &str, init_job_id: JobId) -> IcedContext {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub enum IcedMessage {}

pub fn update(context: &mut IcedContext, message: IcedMessage) -> Result<Task<IcedMessage>, Error> {
    todo!()
}

pub fn view<'c>(context: &'c IcedContext) -> Element<'c, IcedMessage> {
    todo!()
}
