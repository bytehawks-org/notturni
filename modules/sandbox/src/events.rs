//! Handler degli eventi a cui il modulo è iscritto (`[events] subscribes`).
//! Il nome della funzione deriva dal subject: `sandbox.note.created` → `on_sandbox_note_created`.

use noct_core::{
    AppState,
    bus::{HandlerError, Message},
};

pub async fn on_sandbox_note_created(st: AppState, msg: Message) -> Result<(), HandlerError> {
    st.services
        .cache
        .set("sandbox:last_created", &msg.payload.to_string(), None)
        .await
        .map_err(|e| HandlerError::Retry(e.to_string()))
}
