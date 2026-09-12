//! sitemap.xml / RSS feed.xml / robots.txt 生成

use crate::models::post::{Post, SiteConfig};
use std::fs;
use std::path::Path;

/// 生成 sitemap.xml
///
/// 包含：首页 + 所有文章页（per-post URL）
pub fn write_sitemap(
    output: &Path,
    config: &SiteConfig,
    posts: &[Post],
) -> std::io::Result<()> {
    let base = config.url.trim_end_matches('/');
    let mut urls = Vec::new();
    urls.push(format!(
        r#"  <url>
    <loc>{}/</loc>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>"#,
        base
    ));
    for p in posts {
        let slug = if p.slug.is_empty() {
            &p.id
        } else {
            &p.slug
        };
        let url = format!("{}/posts/{}.html", base, slug);
        urls.push(format!(
            r#"  <url>
    <loc>{}</loc>
    <lastmod>{}</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>"#,
            url,
            normalize_date(&p.updated_at)
        ));
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{urls}
</urlset>"#,
        urls = urls.join("\n")
    );
    fs::write(output.join("sitemap.xml"), xml)
}

/// 生成 RSS feed.xml (RSS 2.0)
pub fn write_rss(
    output: &Path,
    config: &SiteConfig,
    posts: &[Post],
) -> std::io::Result<()> {
    let base = config.url.trim_end_matches('/');
    let build_date = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000");
    let items: Vec<String> = posts
        .iter()
        .map(|p| {
            let slug = if p.slug.is_empty() {
                &p.id
            } else {
                &p.slug
            };
            let link = format!("{}/posts/{}.html", base, slug);
            let pub_date =
                format_rfc822_date(&p.created_at);
            format!(
                r#"    <item>
      <title>{}</title>
      <link>{}</link>
      <guid isPermaLink="true">{}</guid>
      <pubDate>{}</pubDate>
      <category>{}</category>
    </item>"#,
                xml_escape(&p.title),
                link,
                link,
                pub_date,
                xml_escape(&p.category)
            )
        })
        .collect();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>{}</title>
    <link>{}</link>
    <description>{}</description>
    <language>zh-CN</language>
    <lastBuildDate>{}</lastBuildDate>
    <atom:link href="{}/feed.xml" rel="self" type="application/rss+xml"/>
{}
  </channel>
</rss>"#,
        xml_escape(&config.title),
        base,
        xml_escape(&config.description),
        build_date,
        base,
        items.join("\n")
    );
    fs::write(output.join("feed.xml"), xml)
}

/// 生成 robots.txt
pub fn write_robots(
    output: &Path,
    config: &SiteConfig,
) -> std::io::Result<()> {
    let base = config.url.trim_end_matches('/');
    let body = format!(
        "User-agent: *\nAllow: /\n\nSitemap: {}/sitemap.xml\n",
        base
    );
    fs::write(output.join("robots.txt"), body)
}

// ============================================================
// 工具
// ============================================================

/// 把 ISO 时间 (2026-09-12T08:00:00Z) 截取为 YYYY-MM-DD
fn normalize_date(s: &str) -> String {
    s.chars().take(10).collect::<String>()
}

/// 把 ISO 时间转为 RFC 822 (RSS pubDate 格式)
/// 简化处理：直接截前 25 个字符加时区修正
fn format_rfc822_date(s: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return dt.format("%a, %d %b %Y %H:%M:%S +0000").to_string();
    }
    s.to_string()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
