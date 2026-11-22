//! Handler `custom`: la sola logica scritta a mano del modulo.

use crate::Echo;
use noct_core::{ApiError, RouteCtx};

pub async fn echo(ctx: RouteCtx) -> Result<Echo, ApiError> {
    let word = ctx.path.get("word").cloned().unwrap_or_default();
    let greeting = ctx.config_str("greeting").unwrap_or("hello").to_string();
    Ok(Echo { word: word.to_uppercase(), greeting })
}

pub async fn admin_ping(_ctx: RouteCtx) -> Result<(), ApiError> {
    Ok(())
}
