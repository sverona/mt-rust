use maud::{html, Markup};
use orgize::{Element, Org}
use url::Url;

pub struct Head<'a> {
	pub title: &'a str,
	pub description: &'a str,
	pub url: Url,
	pub css_hash: &'a str,
}

pub fn render(meta: Head, content: Org) -> Result<Markup, Box<dyn Error>> {
	let mut writer = Vec::new();
	Org::parse(content).write_html(&mut writer)?;
	let html_content = String::from_utf8(writer)?;
	html! {
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
				main #main { (html_content) }
			}
		}
	}
}

fn build() -> Result<(), Box<dyn Error>> {
	fs::remove_dir_all("dist").ok();
	fs::create_dir_all("dist")?;

}

fn main() {
    println!("Hello, world!");
}
