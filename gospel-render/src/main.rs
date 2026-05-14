use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

static CSS: &str = r#"
:root {
    --bg: #fdf6e3;
    --text: #3a2f28;
    --verse-color: #8b6914;
    --border: #d4c5a0;
    --heading: #5a3e28;
    --nav-bg: #2c1810;
    --nav-text: #e8d5b7;
    --link: #8b4513;
    --link-hover: #a0522d;
    --code-bg: #efe0c5;
    --code-text: #6b3a2a;
    --toc-hover: #f5ebd2;
}
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: Georgia, "Noto Serif SC", "Source Han Serif SC", "SimSun", serif;
    background: var(--bg);
    color: var(--text);
    line-height: 1.9;
    min-height: 100vh;
}
nav.top-nav {
    background: var(--nav-bg);
    padding: 12px 24px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    position: sticky;
    top: 0;
    z-index: 100;
    box-shadow: 0 2px 8px rgba(0,0,0,0.3);
}
nav.top-nav a {
    color: var(--nav-text);
    text-decoration: none;
    padding: 6px 14px;
    border-radius: 4px;
    font-size: 0.9em;
    transition: background 0.2s;
}
nav.top-nav a:hover { background: rgba(255,255,255,0.15); }
nav.top-nav a.active { background: rgba(255,255,255,0.25); font-weight: bold; }
nav.top-nav .sep { color: rgba(232,213,183,0.3); user-select: none; }
main {
    max-width: 720px;
    margin: 0 auto;
    padding: 32px 24px 64px;
}
.book-title {
    text-align: center;
    margin-bottom: 48px;
    padding-bottom: 32px;
    border-bottom: 2px double var(--border);
}
.book-title h1 {
    font-size: 2em;
    color: var(--heading);
    margin-bottom: 4px;
}
.book-title .sub {
    font-size: 1.1em;
    color: #7a6a5a;
    font-style: italic;
}
.chapter {
    margin-bottom: 40px;
}
.chapter h2 {
    color: var(--heading);
    font-size: 1.35em;
    margin-bottom: 20px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
    text-align: center;
}
.verse {
    margin-bottom: 6px;
    text-indent: -2.2em;
    padding-left: 2.8em;
    text-align: justify;
}
.verse-num {
    color: var(--verse-color);
    font-size: 0.75em;
    font-weight: bold;
    vertical-align: super;
    margin-right: 2px;
}
.verse-num::after { content: " "; }
code {
    font-family: "Cascadia Code", "Fira Code", "JetBrains Mono", Consolas, monospace;
    background: var(--code-bg);
    color: var(--code-text);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 0.92em;
}
.index-main {
    max-width: 600px;
    margin: 0 auto;
    padding: 48px 24px;
}
.index-main h1 {
    text-align: center;
    font-size: 2em;
    color: var(--heading);
    margin-bottom: 8px;
}
.index-main .subtitle {
    text-align: center;
    color: #7a6a5a;
    font-style: italic;
    margin-bottom: 40px;
}
.index-main .verse-quote {
    text-align: center;
    font-style: italic;
    color: #6b5a48;
    margin-bottom: 48px;
    font-size: 1.1em;
    line-height: 2;
}
.toc { list-style: none; }
.toc li { margin-bottom: 0; }
.toc li a {
    display: block;
    padding: 12px 16px;
    text-decoration: none;
    color: var(--text);
    border-bottom: 1px solid var(--border);
    transition: background 0.2s;
}
.toc li a:hover { background: var(--toc-hover); }
.toc li a .num {
    color: var(--verse-color);
    font-weight: bold;
    margin-right: 8px;
}
.toc li a .en {
    color: #9a8a7a;
    font-size: 0.85em;
    margin-left: 6px;
}
footer {
    text-align: center;
    color: #b0a090;
    font-size: 0.85em;
    padding: 32px 0;
    border-top: 1px solid var(--border);
    margin-top: 32px;
}
footer a { color: var(--link); }
"#;

#[derive(Debug)]
struct Verse {
    chapter_num: u32,
    verse_num: u32,
    text: String,
}

#[derive(Debug)]
struct Chapter {
    number: u32,
    title: String,
    verses: Vec<Verse>,
}

#[derive(Debug)]
struct Book {
    filename: String,
    number: u32,
    title_cn: String,
    title_en: String,
    chapters: Vec<Chapter>,
}

fn parse_book_title(heading: &str) -> Option<(u32, String, String)> {
    let h = heading.trim_start_matches('#').trim();

    let (colon_pos, colon_len) = if let Some(pos) = h.find('：') {
        (pos, 3)
    } else if let Some(pos) = h.find(':') {
        (pos, 1)
    } else {
        return None;
    };

    let num_part = &h[..colon_pos];
    let rest = &h[colon_pos + colon_len..];

    let num_str = num_part.trim_start_matches('卷');
    let num = chinese_num_to_u32(num_str)?;

    let rest = rest.trim();
    let (cn, en) = if let Some(pos) = rest.rfind('(') {
        let cn_part = rest[..pos].trim();
        let en_part = &rest[pos + 1..];
        let en_part = en_part.trim_end_matches(')');
        (cn_part.to_string(), en_part.to_string())
    } else {
        (rest.to_string(), String::new())
    };

    Some((num, cn, en))
}

fn parse_chapter_title(heading: &str) -> Option<(u32, String)> {
    let h = heading.trim_start_matches('#').trim();

    let rest = h.trim_start_matches('第');
    let zhang_pos = rest.find('章')?;

    let num_str = &rest[..zhang_pos];
    let num = chinese_num_to_u32(num_str)?;

    let title_part = &rest[zhang_pos + 3..];
    let colon_skip = if title_part.starts_with('：') {
        3
    } else if title_part.starts_with(':') {
        1
    } else {
        0
    };

    let title = title_part[colon_skip..].trim().to_string();
    Some((num, title))
}

fn chinese_num_to_u32(s: &str) -> Option<u32> {
    let map: HashMap<char, u32> = [
        ('一', 1),
        ('二', 2),
        ('三', 3),
        ('四', 4),
        ('五', 5),
        ('六', 6),
        ('七', 7),
        ('八', 8),
        ('九', 9),
        ('十', 10),
        ('百', 100),
    ]
    .into_iter()
    .collect();

    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return None;
    }

    if chars.len() == 1 {
        return map.get(&chars[0]).copied();
    }

    if chars.len() == 2 && chars[1] == '十' {
        return Some(map.get(&chars[0])? * 10);
    }
    if chars.len() == 2 && chars[0] == '十' {
        return Some(10 + map.get(&chars[1])?);
    }
    if chars.len() == 3 && chars[1] == '十' {
        return Some(map.get(&chars[0])? * 10 + map.get(&chars[2])?);
    }
    None
}

fn parse_verse_start(text: &str) -> Option<(u32, Vec<u32>, usize)> {
    let re = regex_lite::Regex::new(r"^(\d+):(\d+(?:,\d+)*)\s").ok()?;
    let caps = re.captures(text)?;
    let chapter: u32 = caps.get(1)?.as_str().parse().ok()?;
    let verse_str = caps.get(2)?.as_str();
    let verse_len = caps.get(0)?.end();
    let nums: Vec<u32> = verse_str
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.is_empty() {
        return None;
    }
    Some((chapter, nums, verse_len))
}

fn parse_book(filepath: &Path) -> Option<Book> {
    let filename = filepath.file_stem()?.to_str()?.to_string();
    let md = fs::read_to_string(filepath).ok()?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(&md, options);

    let mut book: Option<Book> = None;
    let mut current_chapter: Option<Chapter> = None;
    let mut in_heading = false;
    let mut heading_level = 0;
    let mut heading_text = String::new();
    let mut in_paragraph = false;
    let mut paragraph_text = String::new();
    let mut in_code = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_level = level as usize;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(..)) => {
                in_heading = false;
                let ht = heading_text.trim().to_string();

                if heading_level == 1 {
                    if let Some((num, cn, en)) = parse_book_title(&ht) {
                        book = Some(Book {
                            filename: filename.clone(),
                            number: num,
                            title_cn: cn,
                            title_en: en,
                            chapters: Vec::new(),
                        });
                    }
                } else if heading_level == 2 {
                    if let Some(ref mut b) = book {
                        if let Some(current) = current_chapter.take() {
                            b.chapters.push(current);
                        }
                        let (ch_num, ch_title) = parse_chapter_title(&ht).unwrap_or_else(|| {
                            let next = b.chapters.len() as u32 + 1;
                            (next, ht.clone())
                        });
                        current_chapter = Some(Chapter {
                            number: ch_num,
                            title: ch_title,
                            verses: Vec::new(),
                        });
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) | Event::Start(Tag::Paragraph) => {
                if !in_heading {
                    in_paragraph = true;
                    paragraph_text.clear();
                }
            }
            Event::End(TagEnd::CodeBlock) | Event::End(TagEnd::Paragraph) => {
                if !in_heading && in_paragraph {
                    in_paragraph = false;
                    let text = paragraph_text.trim().to_string();
                    if !text.is_empty() {
                        if let Some(ref mut ch) = current_chapter {
                            if let Some((ch_num, verse_nums, offset)) = parse_verse_start(&text) {
                                let verse_text = text[offset..].to_string();
                                for vn in verse_nums {
                                    ch.verses.push(Verse {
                                        chapter_num: ch_num,
                                        verse_num: vn,
                                        text: verse_text.clone(),
                                    });
                                }
                            } else {
                                if let Some(last) = ch.verses.last_mut() {
                                    last.text.push(' ');
                                    last.text.push_str(&text);
                                }
                            }
                        } else if let Some(_book) = &book {
                            if let Some((ch_num, verse_nums, offset)) = parse_verse_start(&text) {
                                let verse_text = text[offset..].to_string();
                                let mut ch = Chapter {
                                    number: ch_num,
                                    title: String::new(),
                                    verses: Vec::new(),
                                };
                                for vn in verse_nums {
                                    ch.verses.push(Verse {
                                        chapter_num: ch_num,
                                        verse_num: vn,
                                        text: verse_text.clone(),
                                    });
                                }
                                current_chapter = Some(ch);
                            }
                        }
                    }
                }
            }
            Event::Text(ref t) => {
                if in_heading {
                    heading_text.push_str(t);
                } else if in_paragraph {
                    if in_code {
                        paragraph_text.push('`');
                        in_code = false;
                    }
                    paragraph_text.push_str(t);
                }
            }
            Event::Code(ref t) => {
                if in_heading {
                    heading_text.push_str(t);
                } else if in_paragraph {
                    if !in_code {
                        paragraph_text.push('`');
                        in_code = true;
                    }
                    paragraph_text.push_str(t);
                }
            }
            Event::InlineHtml(t) | Event::Html(t) => {
                if in_paragraph {
                    paragraph_text.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_paragraph {
                    paragraph_text.push('\n');
                }
            }
            _ => {}
        }
    }

    if let Some(ref mut b) = book {
        if let Some(current) = current_chapter.take() {
            b.chapters.push(current);
        }
    }

    book
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_verse_html(text: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for ch in text.chars() {
        if ch == '`' {
            if in_code {
                out.push_str("</code>");
                in_code = false;
            } else {
                out.push_str("<code>");
                in_code = true;
            }
        } else if ch == '\n' {
            out.push_str("<br>");
        } else {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            out.push_str(&html_escape(s));
        }
    }
    if in_code {
        out.push_str("</code>");
    }
    out
}

fn generate_book_page(book: &Book, all_books: &[Book]) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!(
        "<title>{} —— 计算机嘉豪福音</title>\n",
        html_escape(&book.title_cn)
    ));
    html.push_str("<style>");
    html.push_str(CSS);
    html.push_str("</style>\n</head>\n<body>\n");

    html.push_str("<nav class=\"top-nav\">\n");
    html.push_str("<a href=\"index.html\">📖 目录</a>\n");
    html.push_str("<span class=\"sep\">|</span>\n");
    for (i, b) in all_books.iter().enumerate() {
        if i > 0 {
            html.push_str("<span class=\"sep\">|</span>\n");
        }
        let href = format!("{}.html", b.filename);
        let label = format!("{}. {}", b.number, b.title_cn);
        let active = if b.filename == book.filename {
            " class=\"active\""
        } else {
            ""
        };
        html.push_str(&format!(
            "<a href=\"{}\"{}>{}</a>\n",
            href,
            active,
            html_escape(&label)
        ));
    }
    html.push_str("</nav>\n");

    html.push_str("<main>\n");
    html.push_str("<div class=\"book-title\">\n");
    html.push_str(&format!(
        "<h1>卷{}：{} ({})</h1>\n",
        book.number,
        html_escape(&book.title_cn),
        html_escape(&book.title_en)
    ));
    html.push_str("</div>\n");

    for ch in &book.chapters {
        html.push_str("<div class=\"chapter\">\n");
        html.push_str(&format!(
            "<h2>第{}章：{}</h2>\n",
            ch.number,
            html_escape(&ch.title)
        ));
        let mut last_ch_num = 0;
        for v in &ch.verses {
            let show_ch = v.chapter_num != last_ch_num;
            if show_ch {
                last_ch_num = v.chapter_num;
            }
            let num_str = if show_ch {
                format!("{}:{}", v.chapter_num, v.verse_num)
            } else {
                v.verse_num.to_string()
            };
            html.push_str("<p class=\"verse\">");
            html.push_str(&format!("<sup class=\"verse-num\">{}</sup>", num_str));
            html.push_str(&render_verse_html(&v.text));
            html.push_str("</p>\n");
        }
        html.push_str("</div>\n");
    }

    html.push_str("<footer>\n");
    html.push_str("<p>《计算机嘉豪福音》—— 一部虚构的文学戏仿作品</p>\n");
    html.push_str("<p>本作品采用 <a href=\"https://creativecommons.org/licenses/by-nc-sa/4.0/\">CC BY-NC-SA 4.0</a> 协议 | 经文由 Rust 重写世间一切而生成</p>\n");
    html.push_str("</footer>\n");
    html.push_str("</main>\n</body>\n</html>\n");

    html
}

fn generate_index(all_books: &[Book]) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n");
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("<title>计算机嘉豪福音 — 正典全集</title>\n");
    html.push_str("<style>");
    html.push_str(CSS);
    html.push_str("</style>\n</head>\n<body>\n");

    html.push_str("<main class=\"index-main\">\n");
    html.push_str("<h1>📖 计算机嘉豪福音</h1>\n");
    html.push_str("<p class=\"subtitle\">The Gospel of Computer Jiahao</p>\n");
    html.push_str(
        "<p class=\"verse-quote\">起初，嘉豪面对漆黑的终端，屏幕渊面混沌。<br>嘉豪的灵运行在键盘之上。</p>\n",
    );
    html.push_str("<ul class=\"toc\">\n");
    for b in all_books {
        html.push_str("<li>");
        html.push_str(&format!(
            "<a href=\"{}.html\"><span class=\"num\">卷{}</span>{}</a>\n",
            b.filename,
            b.number,
            html_escape(&b.title_cn),
        ));
        html.push_str("</li>\n");
    }
    html.push_str("</ul>\n");

    html.push_str("<footer>\n");
    html.push_str("<p>《计算机嘉豪福音》—— 一部虚构的文学戏仿作品</p>\n");
    html.push_str("<p>本作品采用 <a href=\"https://creativecommons.org/licenses/by-nc-sa/4.0/\">CC BY-NC-SA 4.0</a> 协议 | 经文由 Rust 重写世间一切而生成</p>\n");
    html.push_str("</footer>\n");
    html.push_str("</main>\n</body>\n</html>\n");

    html
}

fn main() {
    let books_dir = Path::new("../books");
    let out_dir = Path::new("../site");

    fs::create_dir_all(out_dir).expect("无法创建 site 输出目录");

    let mut entries: Vec<_> = fs::read_dir(books_dir)
        .expect("无法读取 books 目录")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut books: Vec<Book> = Vec::new();
    for entry in &entries {
        let path = entry.path();
        if let Some(book) = parse_book(&path) {
            println!(
                "✅ 解析完成: 卷{} {} ({}章, {}节)",
                book.number,
                book.title_cn,
                book.chapters.len(),
                book.chapters.iter().map(|c| c.verses.len()).sum::<usize>()
            );
            books.push(book);
        } else {
            eprintln!("⚠️  跳过无法解析: {}", path.display());
        }
    }

    books.sort_by_key(|b| b.number);

    for book in &books {
        let html = generate_book_page(book, &books);
        let out_path = out_dir.join(format!("{}.html", book.filename));
        let mut f = fs::File::create(&out_path).expect("无法创建 HTML 文件");
        f.write_all(html.as_bytes()).expect("无法写入 HTML");
        println!("📄 生成: {}", out_path.display());
    }

    let index_html = generate_index(&books);
    let index_path = out_dir.join("index.html");
    let mut f = fs::File::create(&index_path).expect("无法创建 index.html");
    f.write_all(index_html.as_bytes())
        .expect("无法写入 index.html");
    println!("📄 生成: {}", index_path.display());

    println!(
        "\n✨ 全部完成！{} 卷经文已由 Rust 重写世间一切。",
        books.len()
    );
    println!("   打开 {} 查看效果", out_dir.join("index.html").display());
}
