//! Motore dei modelli dei documenti legali, volutamente minimo e senza I/O.
//!
//! * `{{nome}}` — valore (dati del titolare o dedotto dall'installazione);
//! * `{{nome|testo di riserva}}` — il testo di riserva se il valore è vuoto;
//! * `{{#if funzione}} … {{/if}}` e `{{#unless funzione}} … {{/unless}}` — blocchi che compaiono solo se la
//!   funzione è attiva (o, per un valore, se non è vuoto); si annidano.
//!
//! Un valore vuoto senza riserva è «mancante» (il documento non si può pubblicare); un nome sconosciuto o un
//! blocco non chiuso è un «problema» (stesso effetto): un refuso non deve finire in una pagina pubblica.
//! Un tag di blocco da solo sulla propria riga non lascia righe vuote.

use std::collections::BTreeMap;

const MAX_DEPTH: usize = 8;
const MAX_TAG: usize = 160;

pub struct Context<'a> {
    pub flags: &'a BTreeMap<String, bool>,
    pub values: &'a BTreeMap<String, String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    /// variabili usate dal testo attivo ma vuote e senza riserva, nell'ordine d'uso
    pub missing: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug)]
enum Node {
    Text(String),
    Var { name: String, fallback: Option<String> },
    Block { name: String, negate: bool, children: Vec<Node> },
}

#[derive(Debug)]
enum Token {
    Text(String),
    Var { name: String, fallback: Option<String> },
    Open { name: String, negate: bool },
    Close { negate: bool },
}

/// Spezza il testo in testo e tag; i tag di blocco da soli su una riga si prendono anche l'a capo.
fn tokenize(src: &str, issues: &mut Vec<String>) -> Vec<Token> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = src;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}").filter(|e| *e <= MAX_TAG) else {
            text.push_str(&rest[..start + 2]);
            rest = after;
            issues.push("`{{` senza `}}` di chiusura".into());
            continue;
        };
        let inner = after[..end].trim();
        let tag = if let Some(n) = inner.strip_prefix("#if ") {
            Some(Token::Open { name: n.trim().to_string(), negate: false })
        } else if let Some(n) = inner.strip_prefix("#unless ") {
            Some(Token::Open { name: n.trim().to_string(), negate: true })
        } else if inner == "/if" {
            Some(Token::Close { negate: false })
        } else if inner == "/unless" {
            Some(Token::Close { negate: true })
        } else {
            None
        };
        let mut before = &rest[..start];
        let mut tail = &after[end + 2..];
        match tag {
            Some(tag) => {
                // tag di blocco: se è l'unico contenuto della riga, sparisce l'intera riga
                let line_start = before.rfind('\n').map_or(0, |i| i + 1);
                let alone_before = before[line_start..].chars().all(|c| c == ' ' || c == '\t');
                let line_end = tail.find('\n').unwrap_or(tail.len());
                let alone_after = tail[..line_end].chars().all(|c| c == ' ' || c == '\t' || c == '\r');
                if alone_before && alone_after {
                    before = &before[..line_start];
                    tail = tail.get(line_end + 1..).unwrap_or("");
                }
                text.push_str(before);
                if !text.is_empty() {
                    out.push(Token::Text(std::mem::take(&mut text)));
                }
                out.push(tag);
            }
            None => {
                text.push_str(before);
                if !text.is_empty() {
                    out.push(Token::Text(std::mem::take(&mut text)));
                }
                let (name, fallback) = match inner.split_once('|') {
                    Some((n, f)) => (n.trim(), Some(f.trim().to_string())),
                    None => (inner, None),
                };
                out.push(Token::Var { name: name.to_string(), fallback });
            }
        }
        rest = tail;
    }
    text.push_str(rest);
    if !text.is_empty() {
        out.push(Token::Text(text));
    }
    out
}

fn parse(tokens: Vec<Token>, issues: &mut Vec<String>) -> Vec<Node> {
    // pila di (nome, negate, figli); in fondo il livello radice
    let mut stack: Vec<(String, bool, Vec<Node>)> = vec![(String::new(), false, Vec::new())];
    for tok in tokens {
        match tok {
            Token::Text(t) => stack.last_mut().expect("radice").2.push(Node::Text(t)),
            Token::Var { name, fallback } => stack.last_mut().expect("radice").2.push(Node::Var { name, fallback }),
            Token::Open { name, negate } => {
                if stack.len() > MAX_DEPTH {
                    issues.push(format!("blocchi annidati oltre {MAX_DEPTH} livelli"));
                }
                stack.push((name, negate, Vec::new()));
            }
            Token::Close { negate } => {
                if stack.len() == 1 {
                    issues.push(format!("`{{{{/{}}}}}` senza apertura", if negate { "unless" } else { "if" }));
                    continue;
                }
                let (name, open_negate, children) = stack.pop().expect("livello");
                if open_negate != negate {
                    issues.push(format!("il blocco `{name}` è chiuso con il tag sbagliato"));
                }
                stack.last_mut().expect("radice").2.push(Node::Block { name, negate: open_negate, children });
            }
        }
    }
    while stack.len() > 1 {
        let (name, negate, children) = stack.pop().expect("livello");
        issues.push(format!("il blocco `{name}` non è chiuso"));
        stack.last_mut().expect("radice").2.push(Node::Block { name, negate, children });
    }
    stack.pop().expect("radice").2
}

fn eval(nodes: &[Node], ctx: &Context, out: &mut Rendered) {
    for node in nodes {
        match node {
            Node::Text(t) => out.text.push_str(t),
            Node::Var { name, fallback } => match ctx.values.get(name) {
                None => {
                    out.issues.push(format!("variabile sconosciuta `{name}`"));
                    out.text.push_str(&format!("{{{{{name}}}}}"));
                }
                Some(v) if !v.trim().is_empty() => out.text.push_str(v.trim()),
                Some(_) => match fallback.as_deref().filter(|f| !f.is_empty()) {
                    Some(f) => out.text.push_str(f),
                    None => {
                        if !out.missing.contains(name) {
                            out.missing.push(name.clone());
                        }
                        out.text.push_str(&format!("{{{{{name}}}}}"));
                    }
                },
            },
            Node::Block { name, negate, children } => {
                let truth = match (ctx.flags.get(name), ctx.values.get(name)) {
                    (Some(f), _) => *f,
                    (None, Some(v)) => !v.trim().is_empty(),
                    (None, None) => {
                        out.issues.push(format!("funzione o variabile sconosciuta `{name}`"));
                        false
                    }
                };
                if truth != *negate {
                    eval(children, ctx, out);
                }
            }
        }
    }
}

/// Risolve `template`. Mai un errore: `missing` e `issues` dicono cosa impedisce la pubblicazione.
pub fn render(template: &str, ctx: &Context) -> Rendered {
    let mut out = Rendered::default();
    let mut issues = Vec::new();
    let nodes = parse(tokenize(&template.replace("\r\n", "\n"), &mut issues), &mut issues);
    eval(&nodes, ctx, &mut out);
    for i in issues.into_iter().chain(std::mem::take(&mut out.issues)) {
        if !out.issues.contains(&i) {
            out.issues.push(i);
        }
    }
    out.text = out.text.trim_end().to_string() + "\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(t: &str, flags: &[(&str, bool)], values: &[(&str, &str)]) -> Rendered {
        let flags: BTreeMap<String, bool> = flags.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let values: BTreeMap<String, String> = values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        render(t, &Context { flags: &flags, values: &values })
    }

    #[test]
    fn variables_fall_back_or_are_reported_missing() {
        let r = run(
            "Titolare: {{name}}, età {{age|14}}, mail {{mail}}.",
            &[],
            &[("name", "  Anna Rossi "), ("age", ""), ("mail", "")],
        );
        assert_eq!(r.text, "Titolare: Anna Rossi, età 14, mail {{mail}}.\n");
        assert_eq!((r.missing, r.issues), (vec!["mail".to_string()], vec![]));
        // un valore presente batte la riserva; la riserva vuota non basta
        assert_eq!(run("{{a|x}}", &[], &[("a", "v")]).text, "v\n");
        assert_eq!(run("{{a|}}", &[], &[("a", "")]).missing, ["a"]);
    }

    #[test]
    fn blocks_follow_flags_and_non_empty_values_and_nest() {
        let t = "A\n{{#if sso}}\nB {{#if google}}G{{/if}}\n{{/if}}\n{{#unless sso}}\nNO\n{{/unless}}\nZ";
        assert_eq!(run(t, &[("sso", true), ("google", true)], &[]).text, "A\nB G\nZ\n");
        assert_eq!(run(t, &[("sso", true), ("google", false)], &[]).text, "A\nB \nZ\n");
        assert_eq!(run(t, &[("sso", false), ("google", true)], &[]).text, "A\nNO\nZ\n");
        // un valore fa da condizione: pieno = vero
        let dpo = "{{#if dpo}}DPO: {{dpo}}{{/if}}";
        assert_eq!(run(dpo, &[], &[("dpo", "dpo@example.com")]).text, "DPO: dpo@example.com\n");
        assert_eq!(run(dpo, &[], &[("dpo", " ")]).text, "\n");
    }

    #[test]
    fn missing_variables_inside_inactive_blocks_do_not_count() {
        let t = "{{#if backup}}Backup: {{backup_retention}}{{/if}}\nFine";
        assert!(run(t, &[("backup", false)], &[("backup_retention", "")]).missing.is_empty());
        assert_eq!(run(t, &[("backup", true)], &[("backup_retention", "")]).missing, ["backup_retention"]);
    }

    #[test]
    fn typos_and_broken_blocks_are_issues() {
        assert_eq!(run("{{nope}}", &[], &[("a", "1")]).issues, ["variabile sconosciuta `nope`"]);
        assert_eq!(run("{{#if nope}}x{{/if}}", &[], &[]).issues, ["funzione o variabile sconosciuta `nope`"]);
        assert_eq!(run("{{#if a}}x", &[("a", true)], &[]).issues, ["il blocco `a` non è chiuso"]);
        assert_eq!(run("x{{/if}}", &[], &[]).issues, ["`{{/if}}` senza apertura"]);
        assert_eq!(
            run("{{#if a}}x{{/unless}}", &[("a", true)], &[]).issues,
            ["il blocco `a` è chiuso con il tag sbagliato"]
        );
        assert_eq!(run("a {{ b", &[], &[]).issues, ["`{{` senza `}}` di chiusura"]);
    }

    #[test]
    fn markdown_without_tags_is_untouched_and_crlf_is_normalized() {
        let md = "# Titolo\n\n- uno\n- due {x}\n\n| a | b |\n|---|---|\n| 1 | 2 |";
        assert_eq!(run(md, &[], &[]).text, format!("{md}\n"));
        assert_eq!(run("a\r\n{{#if f}}\r\nb\r\n{{/if}}\r\nc", &[("f", true)], &[]).text, "a\nb\nc\n");
    }
}
