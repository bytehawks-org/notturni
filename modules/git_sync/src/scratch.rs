//! Directory temporanea con permessi 0700, rimossa quando esce di scope. Contiene il clone di lavoro, la
//! HOME isolata di git e, per SSH, la chiave privata decifrata: non deve sopravvivere al giro.

use std::{
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct Scratch(PathBuf);

impl Scratch {
    pub fn create() -> std::io::Result<Scratch> {
        let root = std::env::temp_dir().join(format!("noct-git-sync-{}", Uuid::new_v4()));
        std::fs::DirBuilder::new().mode(0o700).create(&root)?;
        let s = Scratch(root);
        std::fs::create_dir(s.repo())?;
        std::fs::create_dir(s.home())?;
        Ok(s)
    }

    pub fn root(&self) -> &Path {
        &self.0
    }

    pub fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    pub fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    /// Scrive un file riservato al proprietario (0600) e ne restituisce il percorso.
    pub fn write_private(&self, name: &str, content: &str) -> std::io::Result<PathBuf> {
        let path = self.0.join(name);
        std::fs::write(&path, content)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
