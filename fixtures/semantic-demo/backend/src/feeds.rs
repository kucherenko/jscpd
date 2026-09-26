//! RSS feed for the blog. Written by another team than `text.rs`, which is
//! how a second slug function came to exist.

use crate::db::Article;

const FEED_TITLE: &str = "Shop news";
const ITEM_LIMIT: usize = 20;

/// The path segment an article gets in feed links.
fn feed_item_slug(title: &str) -> String {
    let lowered = title.to_lowercase();
    let words: Vec<&str> = lowered
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let mut slug = words.join("-");
    if slug.len() > 80 {
        slug.truncate(80);
        slug = slug.trim_end_matches('-').to_string();
    }
    slug
}

pub fn render_feed(site_url: &str, articles: &[Article]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\"><channel>");
    xml.push_str(&format!("<title>{FEED_TITLE}</title><link>{site_url}</link>"));
    for article in articles.iter().take(ITEM_LIMIT) {
        let link = format!("{site_url}/articles/{}", feed_item_slug(&article.title));
        xml.push_str(&format!(
            "<item><title>{}</title><link>{link}</link><pubDate>{}</pubDate></item>",
            escape(&article.title),
            article.published_at.to_rfc2822()
        ));
    }
    xml.push_str("</channel></rss>");
    xml
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
