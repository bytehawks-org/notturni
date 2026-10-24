//! Moderazione automatica delle immagini.
//!
//! Il modulo è l'interruttore: con il modulo attivo, `media` classifica ogni immagine caricata con il
//! servizio `moderation/` (`NOCT_MODERATION_SERVICE_URL`, soglia in `platform_config.moderation_threshold`)
//! e la marca come sensibile; spento (o senza URL del servizio), nessuna classificazione. Il client HTTP è
//! in `noct_http::Moderation` e fallisce sempre «aperto»: un servizio irraggiungibile non blocca un upload.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));
