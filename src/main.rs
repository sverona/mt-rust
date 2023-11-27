use std::error::Error;
use std::path::Path;

use maud::{html, Markup, DOCTYPE, PreEscaped};
use orgize::Org;
use url::Url;
use std::fs;
use std::io;

mod watch;

pub struct Head<'a> {
	pub title: &'a str,
	pub description: &'a str,
	pub url: Url,
}

impl Default for Head<'_> {
	fn default() -> Head<'static> {
		Head {
			title: "A title",
			description: "A description",
			url: Url::parse("https://a.url").unwrap(),
		}
	}
}

pub fn render(meta: Head, content: Org) -> Result<Markup, Box<dyn Error>> {
	let mut writer = Vec::new();
	content.write_html(&mut writer)?;
	let html_content = String::from_utf8(writer)?;
	Ok(html! {
		(DOCTYPE)
		html lang="en" {
			head {
				title { (meta.title) }
				meta charset="utf-8";
				link rel="stylesheet" href="style.css";
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

fn build() -> Result<(), Box<dyn Error>> {
	fs::remove_dir_all("dist").ok();
	fs::create_dir_all("dist")?;

	// if exists
	copy_dir("static", "dist")?;

	// let layout =

	build_page("content/index.org", "dist/index.html")?;
	build_page("content/cube-mnemonics.org", "dist/cube-mnemonics.html")?;

	Ok(())
}

fn build_page<F, T>(from: F, to: T) -> Result<(), Box<dyn Error>>
where
	F: AsRef<Path> + Send + Sync,
	T: AsRef<Path> + Send,
{
	let org_content = fs::read_to_string(from)?;
	let markup = Org::parse(&org_content);

	fs::write(
		to,
		render(Head::default(), markup)?.into_string(),
	)?;

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
				return Err(io::Error::other("asdf"))
			}

			if old_path.is_dir() {
				fs::create_dir(&new_path)?;
				copy_dir(old_path, &new_path)
			} else {
				fs::copy(old_path, new_path).map(|_| ())
			}
		})
}

fn main() {
	let _ = watch::watch();
}
