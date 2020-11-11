//! A little tool to generate the README.md from the docs/.

use std::borrow::Borrow;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use maplit::hashmap;
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag};
use pulldown_cmark_to_cmark::cmark_with_options;
use pulldown_cmark_toc as toc;
use regex_macro::regex;

/// The directory containing the mdBook.
const DOCS_DIR: &str = "docs/";
/// The directory containing the mdBook sources.
const DOCS_SRC_DIR: &str = "docs/src/";
/// A list of SUMMARY titles to add to the README.
const SOURCES: &[&str] = &[
    "Installation",
    "Getting started",
    "Command line interface",
    "Configuration",
];

fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    Ok(fs::read_to_string(path)
        .with_context(|| format!("failed to read from `{}`", path.display()))?)
}

/// Render Markdown events as Markdown.
fn to_cmark<'a, I, E>(events: I) -> String
where
    I: Iterator<Item = E>,
    E: Borrow<Event<'a>>,
{
    let mut buf = String::new();
    cmark_with_options(
        events,
        &mut buf,
        None,
        pulldown_cmark_to_cmark::Options {
            code_block_backticks: 3,
            ..Default::default()
        },
    )
    .unwrap();
    buf
}

/// Returns a list of title and path links contained in the SUMMARY file.
fn summary() -> Result<Vec<(String, PathBuf)>> {
    let docs_src = Path::new(DOCS_SRC_DIR);
    let text = read_to_string(docs_src.join("SUMMARY.md"))?;
    let mut parser = Parser::new_ext(&text, Options::all());
    let mut vec = Vec::new();
    loop {
        let event = parser.next();
        match event {
            None => break,
            Some(Event::Start(Tag::Link(LinkType::Inline, path, _))) => {
                if let Event::Text(CowStr::Borrowed(name)) = parser.next().unwrap() {
                    let path = docs_src.join(path.into_string().trim_start_matches("./"));
                    vec.push((name.into(), path));
                }
            }
            _ => {}
        }
    }
    Ok(vec)
}

fn fix_broken_link(dest: CowStr) -> CowStr {
    let link = dest.as_ref();
    if !regex!(r"^(#|[a-z][a-z0-9+.-]*:)").is_match(link) {
        if let Some(captures) = regex!(r"^(?P<link>.*)\.md(?P<anchor>#.*)?$").captures(link) {
            let mut new_link = String::from("https://rossmacarthur.github.io/sheldon/");
            new_link.push_str(&captures["link"]);
            new_link.push_str(".html");
            if let Some(capture) = captures.name("anchor") {
                new_link.push_str(capture.as_str());
            }
            return CowStr::Boxed(new_link.into());
        }
    }
    dest
}

/// Reformat a Markdown file and prefix headings with the given value.
fn fmt_with_renamed_headings(text: &str, prefix: &str) -> String {
    let mut parser = Parser::new_ext(&text, Options::all());
    let mut events = Vec::new();

    loop {
        match parser.next() {
            Some(Event::Start(Tag::Heading(1))) => {
                while !matches!(parser.next(), None | Some(Event::End(Tag::Heading(_)))) {}
            }
            Some(event @ Event::Start(Tag::Heading(2))) => {
                events.push(event);
                match parser.next().unwrap() {
                    Event::Text(CowStr::Borrowed(name)) => {
                        let new_name = format!("{} {}", prefix, name);
                        events.push(Event::Text(new_name.into()));
                    }
                    event => panic!("expected heading to contain text, got {:?}", event),
                }
            }
            Some(Event::Start(Tag::Link(link_type, dest, title))) => events.push(Event::Start(
                Tag::Link(link_type, fix_broken_link(dest), title),
            )),
            Some(Event::End(Tag::Link(link_type, dest, title))) => events.push(Event::End(
                Tag::Link(link_type, fix_broken_link(dest), title),
            )),
            Some(event) => {
                events.push(event);
            }
            None => break,
        }
    }
    to_cmark(events.into_iter())
}

/// Reformat a Markdown file and increase the heading level.
fn fmt_with_increased_heading_level(text: &str) -> String {
    to_cmark(
        Parser::new_ext(&text, Options::all()).map(|event| match event {
            Event::Start(Tag::Heading(level)) => Event::Start(Tag::Heading(level + 1)),
            Event::Start(Tag::Link(link_type, dest, title)) => {
                Event::Start(Tag::Link(link_type, fix_broken_link(dest), title))
            }
            Event::End(Tag::Link(link_type, dest, title)) => {
                Event::End(Tag::Link(link_type, fix_broken_link(dest), title))
            }
            event => event,
        }),
    )
}

// Construct the contents of our README from docs/
fn generate_readme_contents(summary: &[(String, PathBuf)]) -> Result<String> {
    let mut contents = String::new();
    for (i, (name, path)) in summary
        .iter()
        .filter(|(name, _)| SOURCES.contains(&name.as_str()))
        .enumerate()
    {
        if i != 0 {
            contents.push_str("\n\n");
        }
        let text = read_to_string(&path)?;
        if name == "Configuration" {
            contents.push_str(&fmt_with_renamed_headings(&text, "Configuration:"));
        } else {
            contents.push_str(&fmt_with_increased_heading_level(&text));
        }
    }
    Ok(contents)
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    let borrowed: Vec<_> = args.iter().map(|s| s.as_str()).collect();
    let check = match borrowed.as_slice() {
        &["--check"] => true,
        &[] => false,
        what => bail!("unrecognized command line argument(s): {:?}", what),
    };

    let summary = summary()?;
    let contents = generate_readme_contents(&summary)?;

    // Generate a table of contents for our README
    let toc = toc::TableOfContents::new(&contents)
        .to_cmark_with_options(toc::Options::default().levels(2..=6));

    // Render the updated README
    let mut templates = handlebars::Handlebars::new();
    templates.set_strict_mode(true);
    let readme = read_to_string(Path::new(DOCS_DIR).join("README.hbs"))?;
    let data = hashmap! {
        "toc" => toc,
        "contents" => contents,
    };
    let result = "<!-- automatically generated by ./tools/generate-readme -->\n\n".to_string()
        + &templates.render_template(&readme, &data)?;

    // Finally compare the current README contents and take appropriate action.
    let current = read_to_string("README.md")?;

    if current == result {
        println!("README is up to date!");
    } else if check {
        bail!("README is not up to date!");
    } else {
        fs::write("README.md", result)?;
        println!("README was updated!");
    }

    Ok(())
}
