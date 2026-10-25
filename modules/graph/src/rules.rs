//! Costruzione pura del grafo a partire dalle righe lette dal database, con i test.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use url::Url;
use uuid::Uuid;

pub struct BlogIn {
    pub slug: String,
    pub title: String,
    pub kind: String,
}

pub struct PostIn {
    pub id: Uuid,
    pub blog_slug: String,
    pub slug: String,
    pub title: String,
    pub locale: String,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
pub struct Inputs {
    pub blogs: Vec<BlogIn>,
    pub posts: Vec<PostIn>,
    /// `(post, tag normalizzato)`
    pub tags: Vec<(Uuid, String)>,
    /// `(post, id categoria, nome)`
    pub categories: Vec<(Uuid, Uuid, String)>,
    /// URL citati nel testo dei post: `(post, url)`
    pub urls: Vec<(Uuid, String)>,
    /// link interni `[[slug]]`: `(post, slug di destinazione)`, nello stesso Spazio
    pub wikilinks: Vec<(Uuid, String)>,
}

pub struct Graph {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
}

/// `(slug dello Spazio, slug del post)` a cui punta un URL del sito: `https://{fqdn}/{blog}/{post}`,
/// `https://{blog}.{fqdn}/{post}` oppure il percorso relativo `/{blog}/{post}`. `None` per tutto il resto
/// (siti esterni, pagine che non sono post, domini personali).
pub fn link_target(raw: &str, fqdn: &str) -> Option<(String, String)> {
    let fqdn = fqdn.trim().to_ascii_lowercase();
    let base = Url::parse(&format!("https://{fqdn}/")).ok()?;
    let url = if raw.starts_with('/') && !raw.starts_with("//") {
        base.join(raw).ok()?
    } else {
        Url::parse(raw.trim()).ok()?
    };
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    if host == fqdn || host == format!("www.{fqdn}") {
        return (segments.len() >= 2).then(|| (segments[0].to_string(), segments[1].to_string()));
    }
    let blog = host.strip_suffix(&format!(".{fqdn}"))?;
    (!segments.is_empty() && !blog.contains('.')).then(|| (blog.to_string(), segments[0].to_string()))
}

/// Il grafo: nodi `blog`, `post`, `tag`, `category`; archi `in` (post → Spazio), `tag`, `category` e `link`
/// (post → post, con `via` = `url` o `wiki`). Un tag o una categoria compaiono solo se collegano post inclusi;
/// i link verso post fuori dall'insieme, verso se stessi o duplicati sono scartati.
pub fn build(inputs: &Inputs, fqdn: &str) -> Graph {
    let post_ids: HashSet<Uuid> = inputs.posts.iter().map(|p| p.id).collect();
    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();

    let used_blogs: HashSet<&str> = inputs.posts.iter().map(|p| p.blog_slug.as_str()).collect();
    for b in inputs.blogs.iter().filter(|b| used_blogs.contains(b.slug.as_str())) {
        nodes.push(
            json!({"id": format!("blog:{}", b.slug), "type": "blog", "label": b.title, "slug": b.slug, "kind": b.kind}),
        );
    }
    for p in &inputs.posts {
        nodes.push(json!({
            "id": format!("post:{}", p.id), "type": "post", "label": p.title, "blog": p.blog_slug, "slug": p.slug,
            "locale": p.locale, "status": p.status, "published_at": p.published_at,
            "path": format!("/{}/{}", p.blog_slug, p.slug),
        }));
        edges
            .push(json!({"source": format!("post:{}", p.id), "target": format!("blog:{}", p.blog_slug), "type": "in"}));
    }

    let mut tag_nodes: BTreeMap<&str, ()> = BTreeMap::new();
    for (post, tag) in inputs.tags.iter().filter(|(p, _)| post_ids.contains(p)) {
        tag_nodes.insert(tag, ());
        edges.push(json!({"source": format!("post:{post}"), "target": format!("tag:{tag}"), "type": "tag"}));
    }
    for tag in tag_nodes.keys() {
        nodes.push(json!({"id": format!("tag:{tag}"), "type": "tag", "label": format!("#{tag}"), "tag": tag}));
    }
    let mut cat_nodes: BTreeMap<Uuid, &str> = BTreeMap::new();
    for (post, cat, name) in inputs.categories.iter().filter(|(p, _, _)| post_ids.contains(p)) {
        cat_nodes.insert(*cat, name);
        edges.push(json!({"source": format!("post:{post}"), "target": format!("cat:{cat}"), "type": "category"}));
    }
    for (id, name) in &cat_nodes {
        nodes.push(json!({"id": format!("cat:{id}"), "type": "category", "label": name}));
    }

    let by_slug: HashMap<(&str, &str), Uuid> =
        inputs.posts.iter().map(|p| ((p.blog_slug.as_str(), p.slug.as_str()), p.id)).collect();
    let blog_of: HashMap<Uuid, &str> = inputs.posts.iter().map(|p| (p.id, p.blog_slug.as_str())).collect();
    let mut seen: HashSet<(Uuid, Uuid)> = HashSet::new();
    let mut link = |from: Uuid, to: Uuid, via: &str, edges: &mut Vec<Value>| {
        if from != to && seen.insert((from, to)) {
            edges.push(
                json!({"source": format!("post:{from}"), "target": format!("post:{to}"), "type": "link", "via": via}),
            );
        }
    };
    for (post, url) in inputs.urls.iter().filter(|(p, _)| post_ids.contains(p)) {
        if let Some((blog, slug)) = link_target(url, fqdn)
            && let Some(&to) = by_slug.get(&(blog.as_str(), slug.as_str()))
        {
            link(*post, to, "url", &mut edges);
        }
    }
    for (post, target) in inputs.wikilinks.iter().filter(|(p, _)| post_ids.contains(p)) {
        if let Some(&to) = blog_of.get(post).and_then(|b| by_slug.get(&(*b, target.as_str()))) {
            link(*post, to, "wiki", &mut edges);
        }
    }
    Graph { nodes, edges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_urls_resolve_to_blog_and_post() {
        let t = |u: &str| link_target(u, "Notturni.EU");
        let want = Some(("diario".to_string(), "primo".to_string()));
        assert_eq!(t("https://notturni.eu/diario/primo"), want);
        assert_eq!(t("http://www.notturni.eu/diario/primo/#nota-1"), want);
        assert_eq!(t("https://diario.notturni.eu/primo"), want);
        assert_eq!(t("/diario/primo"), want);
        assert_eq!(t("/diario/primo?x=1"), want);
        for none in [
            "https://example.com/diario/primo",
            "https://notturni.eu/diario",
            "https://notturni.eu/",
            "https://a.b.notturni.eu/primo",
            "https://diario.notturni.eu/",
            "//evil.com/diario/primo",
            "mailto:a@notturni.eu",
            "javascript:alert(1)",
            "#nota-1",
            "notturni.eu/diario/primo",
            "",
        ] {
            assert_eq!(t(none), None, "{none}");
        }
    }

    fn post(blog: &str, slug: &str) -> PostIn {
        PostIn {
            id: Uuid::new_v4(),
            blog_slug: blog.into(),
            slug: slug.into(),
            title: format!("T {slug}"),
            locale: "it".into(),
            status: "published".into(),
            published_at: None,
        }
    }

    fn count(g: &Graph, key: &str, ty: &str) -> usize {
        g.nodes.iter().chain(g.edges.iter()).filter(|v| v[key] == ty).count()
    }

    #[test]
    fn graph_connects_posts_through_tags_categories_and_links() {
        let (a, b, c) = (post("diario", "a"), post("diario", "b"), post("guida", "c"));
        let outside = Uuid::new_v4();
        let cat = Uuid::new_v4();
        let inputs = Inputs {
            blogs: vec![
                BlogIn { slug: "diario".into(), title: "Diario".into(), kind: "blog".into() },
                BlogIn { slug: "guida".into(), title: "Guida".into(), kind: "wiki".into() },
                BlogIn { slug: "vuoto".into(), title: "Vuoto".into(), kind: "blog".into() },
            ],
            tags: vec![
                (a.id, "rust".into()),
                (b.id, "rust".into()),
                (c.id, "wiki".into()),
                (outside, "fantasma".into()),
            ],
            categories: vec![(a.id, cat, "Appunti".into()), (outside, Uuid::new_v4(), "Fuori".into())],
            urls: vec![
                (a.id, "https://notturni.eu/diario/b".into()),
                (a.id, "/diario/b".into()),                    // duplicato
                (b.id, "https://notturni.eu/diario/b".into()), // su se stesso
                (b.id, "https://diario.notturni.eu/a".into()),
                (c.id, "https://example.com/diario/a".into()), // esterno
                (c.id, "https://notturni.eu/diario/inesistente".into()),
            ],
            wikilinks: vec![(c.id, "c".into()), (c.id, "assente".into())],
            posts: vec![a, b, c],
        };
        let g = build(&inputs, "notturni.eu");
        assert_eq!((count(&g, "type", "blog"), count(&g, "type", "post")), (2, 3), "lo Spazio senza post non compare");
        assert_eq!(count(&g, "type", "tag"), 2 + 3, "2 nodi tag + 3 archi tag (il tag fuori insieme non c'è)");
        assert_eq!(g.nodes.iter().filter(|n| n["type"] == "category").count(), 1);
        assert_eq!(g.edges.iter().filter(|e| e["type"] == "category").count(), 1);
        let links: Vec<_> = g.edges.iter().filter(|e| e["type"] == "link").collect();
        assert_eq!(
            links.len(),
            2,
            "a→b una sola volta, b→a dal sottodominio; niente self-link, esterni o inesistenti: {links:?}"
        );
        assert!(links.iter().all(|l| l["via"] == "url"));
        assert!(g.edges.iter().filter(|e| e["type"] == "in").count() == 3);
        assert!(!g.nodes.iter().any(|n| n["label"] == "#fantasma" || n["label"] == "Fuori"));
    }

    #[test]
    fn wiki_links_resolve_within_the_same_space() {
        let (home, install, other) = (post("guida", "home"), post("guida", "install"), post("altro", "install"));
        let inputs = Inputs {
            blogs: vec![],
            wikilinks: vec![(home.id, "install".into())],
            posts: vec![home, install, other],
            ..Default::default()
        };
        let g = build(&inputs, "notturni.eu");
        let links: Vec<_> = g.edges.iter().filter(|e| e["type"] == "link").collect();
        assert_eq!(links.len(), 1);
        assert_eq!(
            (links[0]["via"].as_str(), links[0]["target"] == format!("post:{}", inputs.posts[1].id)),
            (Some("wiki"), true)
        );
    }
}
