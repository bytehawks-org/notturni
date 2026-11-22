//! Consumer dei job del modulo (`[jobs.*]` di module.toml).

use noct_core::{
    AppState,
    bus::{HandlerError, Message},
};

pub async fn run(st: AppState, _msg: Message) -> Result<(), HandlerError> {
    match crate::infra::run_once(&st).await {
        Ok(Some(report)) => {
            tracing::info!(dump = %report.dump_key, objects = report.mirrored_objects, "backup completato");
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => Err(HandlerError::Retry(e)),
    }
}
