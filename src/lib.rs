#![feature(field_init_shorthand)]

extern crate url;
extern crate zip;
extern crate kuchiki;
extern crate html2text;
#[macro_use] extern crate error_chain;

mod error;

use std::path::{ Path, PathBuf };
use std::io::{ self, Read, Write, Seek };
use url::percent_encoding::percent_decode;
use zip::ZipArchive;
use kuchiki::traits::*;
pub use error::Error;


pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}


#[derive(Debug)]
pub struct Book<R: ReadSeek> {
    epub: ZipArchive<R>,
    /// `(title, author, description)`
    pub metadata: (String, String, String),
    /// `(label, path)`
    pub nav: Vec<(usize, String, PathBuf)>
}

impl<R: ReadSeek> Book<R> {
    pub fn new<'a>(mut epub: ZipArchive<R>, opf: &str) -> Result<Book<R>, Error> {
        let mut root = Path::new(opf).to_path_buf();
        root.pop();

        let dom = kuchiki::parse_html()
            .from_utf8()
            .read_from(&mut epub.by_name(opf)?)?;

        let metadata = (
            dom.select(r"dc\:title").unwrap()
                .next().map(|e| e.text_contents())
                .ok_or("No found <dc:title>.")?,
            dom.select(r"dc\:creator").unwrap()
                .next().map(|e| e.text_contents())
                .unwrap_or(String::from("anonymous")),
            dom.select(r"dc\:description").unwrap()
                .next().map(|e| e.text_contents())
                .unwrap_or(String::from("None"))
        );

        let ncx_node = dom.select("item#ncx, item#toc, item#ncxtoc").unwrap()
            .next().ok_or("No found <item id=..>.")?;
        let ncx_attr = ncx_node
            .as_node().as_element().ok_or("No element.")?
            .attributes.borrow();
        let ncx_path = ncx_attr
            .get("href")
            .map(|p| root.join(p))
            .ok_or("No `href` in opf.")?;

        let mut nav = kuchiki::parse_html()
            .from_utf8()
            .read_from(&mut epub.by_name(ncx_path.to_str().unwrap())?)?
            .select("navMap > navPoint").unwrap()
            .map(|node| -> Result<(usize, String, PathBuf), Error> {
                let node = node.as_node();

                let order = node.as_element().ok_or("No element.")?
                    .attributes.borrow()
                    .get("playorder").ok_or("No `playOrder` in <navPoint>")?
                    .parse::<usize>()?;

                let label = node.select("navLabel > text").unwrap()
                    .next().ok_or("No found <text>.")?
                    .text_contents();

                let path = node.select("content").unwrap()
                    .next()
                    .and_then(|n| n.as_node().as_element()
                        .and_then(|n| n.attributes.borrow()
                            .get("src")
                            .map(|src| root.join(src))
                        )
                    )
                    .ok_or("No found <content> or `src`.")?;

                Ok((order, label, path))
            })
            .filter_map(|r| match r {
                Ok(output) => Some(output),
                Err(err) => {
                    write!(io::stderr(), "warn: {}", err).unwrap();
                    None
                }
            })
            .collect::<Vec<(usize, String, PathBuf)>>();

        if nav.is_empty() { Err("nav list is empty!")? };
        nav.sort_by_key(|&(order, ..)| order);

        Ok(Book { epub, metadata, nav })
    }

    pub fn from_container(mut epub: ZipArchive<R>) -> Result<Book<R>, Error> {
        let node = kuchiki::parse_html()
            .from_utf8()
            .read_from(&mut epub.by_name("META-INF/container.xml")?)?
            .select("rootfile").unwrap()
            .next().ok_or("No found <rootfile>.")?;

        let attr = node
            .as_node().as_element().ok_or("No element.")?
            .attributes.borrow();

        Book::new(epub, attr.get("full-path").ok_or("No `full-path` in container.")?)
    }

    pub fn write(&mut self, output: &mut Write) -> Result<(), Error> {
        let (ref title, ref author, ref description) = self.metadata;

        write!(
            output,
"title: {}
author: {}\n
description: {}\n\n\n",
            title.trim(),
            author.trim(),
            description.trim()
        )?;

        for &(i, ref label, _) in &self.nav {
            writeln!(output, "{} - {}", i, label.trim())?;
        }

        write!(output, "\n\n")?;

        for &(_, ref label, ref path) in &self.nav {
            let path = percent_decode(path.to_str().unwrap().as_bytes()).decode_utf8()?;
            write!(
                output,
                "{}\n{}",
                label.trim(),
                html2text::from_read(&mut self.epub.by_name(&path)?, 120),
            )?;
        }

        Ok(())
    }
}


#[inline]
pub fn epub2txt(input: &mut ReadSeek, output: &mut Write) -> Result<(), Error> {
    Book::from_container(ZipArchive::new(input)?)?.write(output)
}
