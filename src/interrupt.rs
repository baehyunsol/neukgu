use crate::{Be2Fe, Context, Error, Fe2Be, load_json};
use ragit_fs::{
    WriteMode,
    exists,
    join,
    write_string,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Interrupt {
    Pause,
    Resume,
    Request {
        request_id: u64,
        request: String,
    },
}

impl Context {
    pub fn check_user_interrupt(&self) -> Result<Option<Interrupt>, Error> {
        if !self.is_fe_alive()? {
            return Ok(None);
        }

        let fe2be = load_json::<Fe2Be>(&join(".neukgu", "fe2be.json")?)?;

        if fe2be.pause {
            return Ok(Some(Interrupt::Pause));
        }

        if let Some((id, request)) = &fe2be.user_request && !self.completed_user_requests.contains(id) {
            return Ok(Some(Interrupt::Request { request_id: *id, request: request.to_string() }));
        }

        Ok(None)
    }

    pub fn add_user_request_turn(&mut self, id: u64, request: String) {
        // It has to be a fresh turn.
        assert!(self.curr_raw_response.is_none());
        self.curr_raw_response = Some((String::from("
<ask>
<to>user</to>
<question>Do you have any feedbacks so far?</question>
</ask>
        "), 0));
        self.user_request = Some((id, request.to_string()));
    }

    pub fn mark_user_request_complete(&mut self) -> Result<(), Error> {
        if let Some((id, _)) = self.user_request.take() {
            let be2fe_at = join(".neukgu", "be2fe.json")?;
            let mut be2fe = if exists(&be2fe_at) {
                load_json::<Be2Fe>(&be2fe_at)?
            } else {
                Be2Fe::default()
            };
            be2fe.completed_user_request = Some(id);
            self.completed_user_requests.insert(id);

            write_string(
                &be2fe_at,
                &serde_json::to_string_pretty(&be2fe)?,
                WriteMode::Atomic,
            )?;
            Ok(())
        } else {
            panic!("attempt to mark a user request that does not exist...")
        }
    }
}
