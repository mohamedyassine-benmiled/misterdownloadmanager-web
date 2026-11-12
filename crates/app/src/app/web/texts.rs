//! Fork-owned Web source collection. Keep remote.rs's translation hook one line long.
pub(crate) fn page_sources() -> std::collections::BTreeSet<String> {
    let html = include_str!("../../../../remote/web/index.html");
    let js = include_str!("../../../../remote/web/app.js");
    let mut sources = std::collections::BTreeSet::new();
    for attr in ["data-t=\"", "data-ta=\"", "data-tp=\""] {
        for part in html.split(attr).skip(1) {
            sources.insert(part.split('"').next().unwrap_or_default().to_owned());
        }
    }
    for call in ["T('", "F('"] {
        for part in js.split(call).skip(1) {
            sources.insert(part.split('\'').next().unwrap_or_default().to_owned());
        }
    }
    for field in super::specs() {
        for key in ["label", "section"] {
            if let Some(text) = field[key].as_str() {
                sources.insert(text.to_owned());
            }
        }
    }
    sources
}
