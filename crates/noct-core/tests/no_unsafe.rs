//! Guardia contro il codice `unsafe`: il compilatore la impone già (`unsafe_code = "forbid"`
//! a livello di workspace), questo test la rende verificabile e blocca chi provasse ad
//! allentarla (rimuovere il lint da un crate, o scrivere `#[allow(unsafe_code)]`).

use std::{
    fs,
    path::{Path, PathBuf},
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).expect("radice del workspace").to_path_buf()
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

fn sources(ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["crates", "modules", "bins"] {
        files(&workspace_root().join(sub), ext, &mut out);
    }
    out
}

/// Toglie il contenuto dei letterali stringa su una riga (es. l'elenco delle parole
/// riservate del generatore di codice, che cita `"unsafe"` senza usarlo).
fn strip_strings(line: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut escaped = false;
    for c in line.chars() {
        if inside {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                inside = false;
            }
        } else if c == '"' {
            inside = true;
        } else {
            out.push(c);
        }
    }
    out
}

#[test]
fn workspace_forbids_unsafe_code() {
    let manifest = fs::read_to_string(workspace_root().join("Cargo.toml")).unwrap();
    assert!(manifest.contains("[workspace.lints.rust]") && manifest.contains("unsafe_code = \"forbid\""));
}

#[test]
fn every_crate_inherits_the_workspace_lints() {
    let manifests: Vec<_> =
        sources("toml").into_iter().filter(|p| p.file_name().is_some_and(|n| n == "Cargo.toml")).collect();
    assert!(manifests.len() >= 15, "trovati solo {} crate", manifests.len());
    for m in manifests {
        let text = fs::read_to_string(&m).unwrap();
        assert!(
            text.contains("[lints]") && text.contains("workspace = true"),
            "{} non eredita i lint del workspace",
            m.display()
        );
    }
}

#[test]
fn no_source_file_uses_unsafe_code() {
    let all = sources("rs");
    assert!(all.len() > 50);
    for f in all {
        if f.file_name().is_some_and(|n| n == "no_unsafe.rs") {
            continue;
        }
        let text = fs::read_to_string(&f).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = strip_strings(line.split("//").next().unwrap_or(""));
            let has_word = code.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "unsafe");
            assert!(!has_word, "{}:{}: `unsafe` non è ammesso", f.display(), n + 1);
            assert!(!code.contains("unsafe_code"), "{}:{}: il lint unsafe_code non va toccato", f.display(), n + 1);
        }
    }
}

#[test]
fn the_scanner_sees_unsafe_but_not_strings_or_comments() {
    let hit = |l: &str| {
        strip_strings(l.split("//").next().unwrap())
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|w| w == "unsafe")
    };
    assert!(hit("    unsafe { *p }"));
    assert!(hit("unsafe impl Send for X {}"));
    assert!(hit("pub unsafe fn f() {}"));
    assert!(!hit(r#"const K: &[&str] = &["unsafe", "use"];"#));
    assert!(!hit("// unsafe in un commento"));
    assert!(!hit("let unsafe_count = 1;"));
}
