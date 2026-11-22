//! Categorie per-blog dei post. Il campo `category_id` di create/update/traduzione dei post e
//! `category` della risposta arrivano dall'estensione di `posts` (`extensions.rs`).

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

pub mod extensions;

pub mod admin_sections;
