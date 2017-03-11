extern crate url;
extern crate zip;
extern crate sanngaa;
extern crate html2text;
#[macro_use] extern crate error_chain;

mod error;

use std::path::{ Path, PathBuf };
use std::io::{ self, Read, Write, Seek };
use url::percent_encoding::percent_decode;
use zip::ZipArchive;
use sanngaa::traits::*;
use sanngaa::{ NodeDataRef, ElementData };
pub use error::{ Error, ErrorKind };


pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}


#[derive(Debug)]
pub struct Book<R: ReadSeek> {
    /// epub zip archive
    pub epub: ZipArchive<R>,
    /// `(title, author, description)`
    pub metadata: (String, String, String),
    /// `(order, label, path)`
    pub nav: Vec<(usize, String, PathBuf)>
}

impl<R: ReadSeek> Book<R> {
    pub fn new(mut epub: ZipArchive<R>, opf: &str) -> Result<Book<R>, Error> {
        let mut root = Path::new(opf).to_path_buf();
        root.pop();

        let dom = sanngaa::parse_xml()
            .from_utf8()
            .read_from(&mut epub.by_name(opf)?)?;

        let metadata = (
            dom.select(r"dc\:title").unwrap()
                .next().and_then(|e| e.as_node().first_child())
                .and_then(|e| e.as_text().map(|t| t.borrow().to_string()))
                .ok_or("No found <dc:title>.")?,
            dom.select(r"dc\:creator").unwrap()
                .next().and_then(|e| e.as_node().first_child())
                .and_then(|e| e.as_text().map(|t| t.borrow().to_string()))
                .unwrap_or_else(|| String::from("anonymous")),
            dom.select(r"dc\:description").unwrap()
                .next().and_then(|e| e.as_node().first_child())
                .and_then(|e| e.as_text().map(|t| t.borrow().to_string()))
                .unwrap_or_else(|| String::from("None"))
        );

        let ncx_node = dom.select("item#ncx, item#toc, item#ncxtoc").unwrap()
            .next().ok_or("No found <item id=..>.")?;
        let ncx_path = ncx_node.attributes.borrow()
            .get("href")
            .map(|p| root.join(p))
            .ok_or("No `href` in opf.")?;

        let mut nav = sanngaa::parse_xml()
            .from_utf8()
            .read_from(&mut epub.by_name(ncx_path.to_str().unwrap())?)?
            .select("navMap > navPoint").unwrap()
            .map(|node| node_get_nav(&root, node))
            .filter_map(|r| match r {
                Ok(output) => Some(output),
                Err(err) => {
                    writeln!(io::stderr(), "warn: {}", err).unwrap();
                    None
                }
            })
            .collect::<Vec<_>>();

        if nav.is_empty() { Err("nav list is empty!")? };
        nav.sort_by_key(|&(order, ..)| order);

        Ok(Book { epub, metadata, nav })
    }

    pub fn from_container(mut epub: ZipArchive<R>) -> Result<Book<R>, Error> {
        let node = sanngaa::parse_xml()
            .from_utf8()
            .read_from(&mut epub.by_name("META-INF/container.xml")?)?
            .select("rootfile").unwrap()
            .next().ok_or("No found <rootfile>.")?;

        let attrs = node.attributes.borrow();
        Book::new(epub, attrs.get("full-path").ok_or("No `full-path` in container.")?)
    }

    pub fn write_to(&mut self, output: &mut Write) -> Result<(), Error> {
        let (ref title, ref author, ref description) = self.metadata;

        write!(
            output,
            "title: {}\n\
            author: {}\n\
            description: {}\n\n",
            title.trim(),
            author.trim(),
            description.trim()
        )?;

        for &(_, ref label, ref path) in &self.nav {
            let path = path.to_string_lossy();
            let path = percent_decode(path.as_bytes()).decode_utf8()?;
            write!(
                output,
                "{}:\n{}\n\n",
                label.trim(),
                html2text::from_read(&mut self.epub.by_name(&path)?, 180),
            )?;
        }

        Ok(())
    }
}

fn node_get_nav(root: &Path, node: NodeDataRef<ElementData>) -> Result<(usize, String, PathBuf), Error> {
    let order = node.attributes.borrow()
        .get("playOrder").ok_or("No `playOrder` in <navPoint>")?
        .parse::<usize>()?;

    let label = node.as_node().select("navLabel > text").unwrap()
        .next().ok_or("No found <text>.")?
        .text_contents();

    let path = node.as_node().select("content").unwrap()
        .next()
        .and_then(|n| n.attributes.borrow()
            .get("src")
            .map(|src| root.join(src))
        )
        .ok_or("No found <content> or `src`.")?;

    Ok((order, label, path))
}


#[inline]
pub fn epub2txt(input: &mut ReadSeek, output: &mut Write) -> Result<(), Error> {
    Book::from_container(ZipArchive::new(input)?)?.write_to(output)
}
