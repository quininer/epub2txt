extern crate url;
extern crate zip;
extern crate sanngaa;
extern crate html2text;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;

mod error;

use std::path::{ Path, PathBuf };
use std::io::{ self, Read, Write, Seek };
use url::Url;
use zip::ZipArchive;
use sanngaa::traits::*;
use sanngaa::{ NodeDataRef, ElementData };
pub use error::{ Error, ErrorKind };


pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

lazy_static! {
    static ref BASE_URL: Url = Url::from_directory_path("/").unwrap();
}



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
                .next().map(|e| e.text_contents())
                .ok_or("No found <dc:title>.")?,
            dom.select(r"dc\:creator").unwrap()
                .next().map(|e| e.text_contents())
                .unwrap_or_else(|| "anonymous".into()),
            dom.select(r"dc\:description").unwrap()
                .next().map(|e| e.text_contents())
                .unwrap_or_else(|| "None".into())
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
            .map(|node| node_get_nav(&root, &node))
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
        nav.dedup_by(|&mut (.., ref p1), &mut (.., ref p2)| p1 == p2);

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
            description: {}\n\n\n",
            title.trim(),
            author.trim(),
            description.trim()
        )?;

        for &(_, ref label, ref path) in &self.nav {
            let context = html2text::from_read(
                &mut self.epub.by_name(&path.to_string_lossy())?,
                ::std::usize::MAX
            );

            if context.starts_with(label.trim()) {
                write!(output, "{}", context)?;
            } else {
                write!(output, "{}:\n{}", label.trim(), context)?;
            }

            write!(output, "\n-----\n\n")?;
        }

        Ok(())
    }
}

fn node_get_nav(root: &Path, node: &NodeDataRef<ElementData>) -> Result<(usize, String, PathBuf), Error> {
    let order = node.attributes.borrow()
        .get("playOrder").ok_or_else(|| format!("No `playOrder` in <navPoint>: {:?}", node))?
        .parse::<usize>()?;

    let label = node.as_node().select("navLabel > text").unwrap()
        .next()
        .map(|node| node.text_contents())
        .unwrap_or_default();

    let path = node.as_node().select("content").unwrap()
        .next()
        .and_then(|n| n.attributes.borrow()
            .get("src")
            .and_then(|src| Url::options().base_url(Some(&BASE_URL)).parse(src).ok())
            .map(|uri| root.join(uri.path().trim_left_matches('/')))
        )
        .ok_or_else(|| format!("No found <content> or `src`: {:?}", node))?;

    Ok((order, label, path))
}


#[inline]
pub fn epub2txt(input: &mut ReadSeek, output: &mut Write) -> Result<(), Error> {
    Book::from_container(ZipArchive::new(input)?)?.write_to(output)
}
