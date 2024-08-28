use axum::{
    body::Body,
    http::Response,
    response::{Html, IntoResponse},
};
use eyre::Result;
use frontmatter::FrontMatter;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

mod frontmatter;
mod markdown;
mod themes;

const DEFAULT_HTML: &str = r#"<!DOCTYPE html><html><head><meta charset="utf-8">{{head}}</head><body>{{content}}</body></html>"#;

pub async fn render(base_path: PathBuf, file: tokio::fs::File) -> Result<Response<Body>> {
    let md = read_file(file).await?;
    let mut front_matter = FrontMatter::from_md(&md);
    let mut content = markdown::md_to_html(&md);

    if let Some(layout) = front_matter.layout.clone() {
        let layout = layout.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "");
        let layout_path = base_path.join(format!("./_layouts/{}.md", layout));
        if layout_path.exists() {
            let layout_file = tokio::fs::File::open(layout_path).await?;
            let layout_md = read_file(layout_file).await?;
            let layout_front_matter = FrontMatter::from_md(&layout_md);
            let layout_content = markdown::md_to_html(&layout_md);
            if layout_content.contains("{{content}}") {
                content = layout_content.replace("{{content}}", &content);
            } else {
                content = layout_content + &content;
            }
            front_matter.merge(&layout_front_matter);
        }
    }

    let html = DEFAULT_HTML
        .replace("{{head}}", &front_matter.html_head())
        .replace("{{content}}", &content);

    Ok(Html::from(html).into_response())
}

async fn read_file(mut file: tokio::fs::File) -> Result<String> {
    let mut buf = String::new();
    file.read_to_string(&mut buf).await?;
    Ok(buf)
}
