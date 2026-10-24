//! Motore di migrazione dichiarativo: confronta lo schema desiderato (db.toml)
//! con l'ultima istantanea (schema.lock.json), produce SQL versionato e lo
//! applica in modo ordinato, verificando i checksum.
//!
//! Politica: solo cambiamenti additivi sono generati in automatico. Tutto ciò
//! che può perdere dati o fallire su dati esistenti viene *bloccato* con una
//! spiegazione e richiede una migrazione SQL scritta a mano.

mod diff;
mod files;
mod render;

#[cfg(feature = "apply")]
mod apply;

pub use diff::{Plan, Step, diff};
pub use files::{checksum, parse_file_name, render_file};
pub use render::{create_table_sql, quote};

#[cfg(feature = "apply")]
pub use apply::{
    AppliedMigration, Migration, MigrationError, ModuleMigrations, Status, StatusRow, apply, status, verify_live,
};
