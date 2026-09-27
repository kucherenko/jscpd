//! Page links for list endpoints.

use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PageLink {
    Page(u32),
    Gap,
}

/// Page numbers to link to: the first and last page, `radius` pages on each
/// side of the current one, and a gap marker wherever pages are skipped.
pub fn page_links(current: u32, total_pages: u32, radius: u32) -> Vec<PageLink> {
    if total_pages == 0 {
        return Vec::new();
    }
    let current = current.clamp(1, total_pages);
    let from = current.saturating_sub(radius).max(1);
    let to = (current + radius).min(total_pages);
    let mut links = Vec::new();
    if from > 1 {
        links.push(PageLink::Page(1));
        if from > 2 {
            links.push(PageLink::Gap);
        }
    }
    links.extend((from..=to).map(PageLink::Page));
    if to < total_pages {
        if to < total_pages - 1 {
            links.push(PageLink::Gap);
        }
        links.push(PageLink::Page(total_pages));
    }
    links
}
