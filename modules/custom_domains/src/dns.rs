//! Ricerca del record TXT di verifica.

use hickory_resolver::{
    Resolver, TokioResolver,
    config::{ConnectionConfig, NameServerConfig, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
    proto::rr::{RData, RecordType},
};
use std::{net::SocketAddr, time::Duration};

/// `server` = `ip` o `ip:porta`; vuoto = resolver del sistema.
fn resolver(server: &str, timeout: Duration) -> Result<TokioResolver, String> {
    let mut builder = if server.trim().is_empty() {
        Resolver::builder_tokio().map_err(|e| e.to_string())?
    } else {
        let addr: SocketAddr = server
            .trim()
            .parse()
            .or_else(|_| format!("{}:53", server.trim()).parse())
            .map_err(|_| format!("NOCT_DNS_SERVER non valido: `{server}` (ip o ip:porta)"))?;
        let mut udp = ConnectionConfig::udp();
        udp.port = addr.port();
        let mut tcp = ConnectionConfig::tcp();
        tcp.port = addr.port();
        let ns = NameServerConfig::new(addr.ip(), true, vec![udp, tcp]);
        let config = ResolverConfig::from_parts(None, vec![], vec![ns]);
        Resolver::builder_with_config(config, TokioRuntimeProvider::default())
    };
    builder.options_mut().timeout = timeout;
    builder.options_mut().attempts = 1;
    builder.build().map_err(|e| e.to_string())
}

/// `true` se `name` ha un record TXT con esattamente `expected`. Qualunque errore (DNS
/// irraggiungibile, nome inesistente, timeout) vale come «non verificato»: non fa fallire la
/// richiesta, solo la singola verifica.
pub async fn txt_matches(server: &str, timeout: Duration, name: &str, expected: &str) -> bool {
    let resolver = match resolver(server, timeout) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "resolver DNS non utilizzabile");
            return false;
        }
    };
    let fqdn = format!("{name}.");
    let Ok(answer) = tokio::time::timeout(timeout * 2, resolver.lookup(fqdn.as_str(), RecordType::TXT)).await else {
        return false;
    };
    let Ok(answer) = answer else { return false };
    answer.answers().iter().any(|record| match &record.data {
        RData::TXT(txt) => {
            let value: Vec<u8> = txt.txt_data.iter().flat_map(|part| part.iter().copied()).collect();
            String::from_utf8_lossy(&value) == expected
        }
        _ => false,
    })
}
