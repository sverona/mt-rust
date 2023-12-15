use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

use clap::Parser;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use orgize::export::{DefaultHtmlHandler, HtmlHandler};
use orgize::{Element, Org};
use regex::Regex;
use slugify::slugify;
use url::Url;

// Conditional compilation --- goes with cargo stuff
#[cfg(feature = "watch")]
mod watch;

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Parser)]
enum Commands {
    #[clap(name = "build")]
    Build {
        #[clap(long)]
        websocket_port: Option<u16>,
    },
    #[cfg(feature = "watch")]
    #[clap(name = "watch")]
    Watch,
}

pub struct Head {
    pub title: String,
    pub description: String,
    pub url: Option<Url>,
}

impl Default for Head {
    fn default() -> Head {
        Head {
            title: "A title".into(),
            description: "A description".into(),
            url: Some(Url::parse("https://a.url").unwrap()),
        }
    }
}

#[derive(Default)]
struct CustomHtmlHandler(DefaultHtmlHandler);

impl HtmlHandler<io::Error> for CustomHtmlHandler {
    fn start<W: io::Write>(&mut self, mut w: W, element: &Element) -> Result<(), io::Error> {
        match element {
            Element::Document { .. } => {
                write!(w, "<main><article>")
            },
            Element::FnRef(note) => {
                let text = note.definition.clone().unwrap().into_owned();

                let label = if !note.label.is_empty() {
                    note.label.clone().into_owned()
                } else {
                    slugify!(&text)
                };

                write!(
                    w,
                    "<label for=\"sidenote-{0}\" class=\"margin-toggle sidenote-number\"></label>
                    <input type=\"checkbox\" id=\"sidenote-{0}\" class=\"margin-toggle\" />
                    <span class=\"sidenote\">",
                    label,
                )?;

                let org = Org::parse_string(text);
                let mut handler = InlineHtmlHandler::default();

                // TODO Nest these fuckers!
                org.write_html_custom(w, &mut handler)
            },
            Element::Text { value } => {
                let text = value.clone().into_owned();
                let abbr_regex = Regex::new(r"\b[A-Z]{2,}\b").unwrap();
                let abbrs = abbr_regex.replace_all(&text, "<abbr>$0</abbr>");
                write!(w, "{}", abbrs)
            },
            _ => {
                self.0.start(w, element)
            }
        }
    }

    fn end<W: io::Write>(&mut self, mut w: W, element: &Element) -> Result<(), io::Error> {
        match element {
            Element::Document { .. } => {
                write!(w, "</article></main>")
            },
            Element::FnRef(_) => {
                write!(w, "</span>")
            },
            _ => self.0.end(w, element)
        }
    }
}

#[derive(Default)]
struct InlineHtmlHandler(DefaultHtmlHandler);

impl HtmlHandler<io::Error> for InlineHtmlHandler {
    fn start<W: io::Write>(&mut self, w: W, element: &Element) -> Result<(), io::Error> {
        match element {
            Element::Document { .. } => Ok(()),
            Element::Section {} => Ok(()),
            Element::Paragraph { .. } => Ok(()),
            _ => self.0.start(w, element),
        }
    }

    fn end<W: io::Write>(&mut self, w: W, element: &Element) -> Result<(), io::Error> {
        match element {
            Element::Document { .. } => Ok(()),
            Element::Section {} => Ok(()),
            Element::Paragraph { .. } => Ok(()),
            _ => self.0.end(w, element),
        }
    }
}

pub fn render(meta: Head, content: Org) -> Result<Markup, Box<dyn Error>> {
    let mut writer = Vec::new();
    let mut handler = CustomHtmlHandler::default();
    content.write_html_custom(&mut writer, &mut handler)?;
    let html_content = String::from_utf8(writer)?;

    Ok(html! {
        (DOCTYPE)
        html lang="en" {
            head {
                title { (meta.title) }
                meta charset="utf-8";
                link rel="stylesheet" href="/style.css";
            }
            body {
                header {
                    h1 { (meta.title) }
                }
                (PreEscaped(html_content))
            }
        }
    })
}

fn build(_websocket_port: Option<u16>) -> Result<(), Box<dyn Error>> {
    fs::remove_dir_all("dist").ok();
    fs::create_dir_all("dist")?;

    // if exists
    copy_dir("static", "dist")?;

    // let layout =

    fs::create_dir_all("dist/blog/")?;
    let articles = fs::read_dir("content/")?;
    for entry in articles {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_stem().unwrap().to_str().unwrap();

        if (filename != "index") {
            fs::create_dir_all(format!("dist/blog/{}", filename))?;
            build_page(path.clone(), format!("dist/blog/{}/index.html", filename))?;
        }
    }
    build_page("content/index.org", "dist/blog/index.html")?;

    Ok(())
}

fn build_page<F, T>(from: F, to: T) -> Result<(), Box<dyn Error>>
where
    F: AsRef<Path> + Send + Sync,
    T: AsRef<Path> + Send,
{
    let org_content = fs::read_to_string(from)?;
    let markup = Org::parse(&org_content);

    let keywords: HashMap<String, String> = markup
        .keywords()
        .map(|kw| {
            (
                kw.key.clone().into_owned().to_lowercase(),
                kw.value.clone().into_owned(),
            )
        })
        .collect();

    let head = Head {
        title: keywords
            .get("title")
            .unwrap_or(&"A title".to_string())
            .to_string(),
        description: keywords
            .get("description")
            .or(keywords.get("subtitle"))
            .unwrap_or(&"".to_string())
            .to_string(),
        url: None,
    };

    fs::write(to, render(head, markup)?.into_string())?;

    Ok(())
}

fn copy_dir<F, T>(from: F, to: T) -> io::Result<()>
where
    F: AsRef<Path> + Send + Sync,
    T: AsRef<Path> + Send,
{
    fs::read_dir(&from)?
        .map_while(|item| item.ok())
        .try_for_each(|entry| {
            let filename = entry.file_name();
            let old_path = entry.path();

            let new_path = to.as_ref().join(filename);
            if new_path.exists() {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "file exists"));
            }

            if old_path.is_dir() {
                fs::create_dir(&new_path)?;
                copy_dir(old_path, &new_path)
            } else {
                fs::copy(old_path, new_path).map(|_| ())
            }
        })
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { websocket_port } => build(websocket_port),
        #[cfg(feature = "watch")]
        Commands::Watch => watch::watch(),
    }
}
