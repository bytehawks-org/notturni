//! Consumer dei job del modulo (`[jobs.*]` di module.toml).

use noct_core::{
    AppState,
    bus::{HandlerError, Message},
};

pub async fn maintenance(st: AppState, _msg: Message) -> Result<(), HandlerError> {
    let archive = st.module_flag("audit", "archive_enabled").unwrap_or(true);
    let report = crate::maintenance::run_once(&st, archive).await.map_err(|e| HandlerError::Retry(e.to_string()))?;
    if !report.archived_weeks.is_empty() || report.deleted_rows > 0 {
        tracing::info!(weeks = ?report.archived_weeks, deleted = report.deleted_rows, "manutenzione dell'audit");
    }
    Ok(())
}
