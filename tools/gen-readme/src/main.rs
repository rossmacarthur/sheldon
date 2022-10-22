//! A little tool to generate the README.md from the docs/.

use std::borrow::Borrow;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag};
use pulldown_cmark_to_cmark::{cmark_resume_with_options, Options as Options2};
use pulldown_cmark_toc as toc;
use regex_macro::regex;

/// The directory containing the mdBook.
const DOCS_DIR: &str = "docs/";
/// The directory containing the mdBook sources.
const DOCS_SRC_DIR: &str = "docs/src/";
/// A list of SUMMARY titles to add to the README.
const SOURCES: &[&str] = &[
    "Installation.md",
    "Getting-started.md",
    "Command-line-interface.md",
    "Configuration.md",
];

fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path).with_context(|| format!("failed to read from `{}`", path.display()))
}

/// Render Markdown events as Markdown.
fn to_cmark<'a, I, E>(events: I) -> Result<String>
where
    I: Iterator<Item = E>,
    E: Borrow<Event<'a>>,
{
    let mut buf = String::new();
    let opts = Options2 {
        code_block_token_count: 3,
        ..Default::default()
    };
    cmark_resume_with_options(events, &mut buf, None, opts)?.finalize(&mut buf)?;
    Ok(buf)
}

/// Returns a list of title and path links contained in the SUMMARY file.
fn summary() -> Result<Vec<PathBuf>> {
    let docs_src = Path::new(DOCS_SRC_DIR);
    let text = read_to_string(docs_src.join("SUMMARY.md"))?;
    let mut parser = Parser::new_ext(&text, Options::all());
    let mut vec = Vec::new();
    loop {
        let event = parser.next();
        match event {
            None => break,
            Some(Event::Start(Tag::Link(LinkType::Inline, path, _))) => {
                if let Event::Text(CowStr::Borrowed(_)) = parser.next().unwrap() {
                    let path = docs_src.join(path.into_string().trim_start_matches("./"));
                    vec.push(path);
                }
            }
            Some(_) => {}
        }
    }
    Ok(vec)
}

fn fix_broken_link(dest: CowStr<'_>) -> CowStr<'_> {
    let link = dest.as_ref();
    if !regex!(r"^(#|[a-z][a-z0-9+.-]*:)").is_match(link) {
        if let Some(captures) = regex!(r"^(?P<link>.*)\.md(?P<anchor>#.*)?$").captures(link) {
            let mut new_link = String::from("https://sheldon.cli.rs/");
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

/// Reformat a Markdown file and increase the heading level.
fn fmt_with_increased_heading_level(text: &str) -> Result<String> {
    to_cmark(
        Parser::new_ext(text, Options::all()).map(|event| match event {
            Event::Start(Tag::Heading(level, frag, classes)) => {
                let level = (level as usize + 1).try_into().unwrap();
                Event::Start(Tag::Heading(level, frag, classes))
            }
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
fn generate_readme_contents(summary: &[PathBuf]) -> Result<String> {
    let mut contents = String::new();
    for (i, path) in summary
        .iter()
        .filter(|path| SOURCES.contains(&path.file_name().unwrap().to_str().unwrap()))
        .enumerate()
    {
        if i != 0 {
            contents.push_str("\n\n");
        }
        let text = read_to_string(path)?;
        contents.push_str(&fmt_with_increased_heading_level(&text)?);
    }
    Ok(contents)
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    let borrowed: Vec<_> = args.iter().map(String::as_str).collect();
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
    let readme = read_to_string(Path::new(DOCS_DIR).join("README_TEMPLATE.md"))?;
    let result = upon::Engine::new().compile(&readme)?.render(upon::value! {
        toc: toc,
        contents: contents,
    })?;

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
