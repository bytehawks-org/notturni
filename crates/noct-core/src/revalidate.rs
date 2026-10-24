//! Tag di invalidazione della cache del frontend. Le stringhe devono restare identiche a
//! quelle di `frontend/src/lib/revalidate.ts`: un disallineamento rompe l'invalidazione
//! mirata senza errori visibili.

pub fn feed() -> String {
    "feed".into()
}

pub fn blog(slug: &str) -> String {
    format!("blog:{slug}")
}

pub fn post(blog_slug: &str, post_slug: &str) -> String {
    format!("post:{blog_slug}:{post_slug}")
}

pub fn platform_pages() -> String {
    "platform-pages".into()
}

pub fn platform_page(slug: &str) -> String {
    format!("platform-page:{slug}")
}

pub fn blog_page(blog_slug: &str, page_slug: &str) -> String {
    format!("blog-page:{blog_slug}:{page_slug}")
}

pub fn platform_footer() -> String {
    "platform-footer".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_match_the_frontend() {
        assert_eq!(feed(), "feed");
        assert_eq!(blog("demo"), "blog:demo");
        assert_eq!(post("demo", "ciao"), "post:demo:ciao");
        assert_eq!(blog_page("demo", "chi-siamo"), "blog-page:demo:chi-siamo");
        assert_eq!(platform_page("privacy"), "platform-page:privacy");
        assert_eq!(platform_pages(), "platform-pages");
        assert_eq!(platform_footer(), "platform-footer");
    }
}
