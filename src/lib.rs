extern crate url;
extern crate zip;
extern crate failure;
extern crate kuchiki;
#[macro_use] extern crate lazy_static;

use std::collections::HashMap;
use std::path::{ Path, PathBuf };
use std::io::{ Read, Write, Seek };
use url::Url;
use zip::ZipArchive;
use failure::{ Error, err_msg };
use kuchiki::traits::*;


pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

macro_rules! try_continue {
    ( $val:expr ) => {
        match $val {
            Some(val) => val,
            None => {
                eprintln!("key missing!");
                return None
            }
        }
    }
}

lazy_static! {
    static ref BASE_URL: Url = Url::from_directory_path("/").unwrap();
}



#[derive(Debug)]
pub struct Book<R: ReadSeek> {
    /// epub zip archive
    pub epub: ZipArchive<R>,
    /// `(title, author, description)`
    pub metadata: (String, String, String),
    /// `path`
    pub spine: Vec<PathBuf>,
}

impl<R: ReadSeek> Book<R> {
    pub fn new(mut epub: ZipArchive<R>, opf: &str) -> Result<Book<R>, Error> {
        let mut root = Path::new(opf).to_path_buf();
        root.pop();

        let dom = kuchiki::parse_html()
            .from_utf8()
            .read_from(&mut epub.by_name(opf)?)?;

        let metadata = (
            dom.select(r"dc\:title").unwrap()
                .next().map(|e| e.text_contents())
                .ok_or(err_msg("No found <dc:title>"))?,
            dom.select(r"dc\:creator").unwrap()
                .next().map(|e| e.text_contents())
                .unwrap_or_else(|| "anonymous".into()),
            dom.select(r"dc\:description").unwrap()
                .next().map(|e| e.text_contents())
                .unwrap_or_else(|| "None".into())
        );

        let manifest = dom.select("item").unwrap()
            .filter_map(|e| {
                let attr = e.attributes.borrow();
                Some((
                    try_continue!(attr.get("id").map(str::to_string)),
                    try_continue!(attr.get("href").map(str::to_string))
                ))
            })
            .collect::<HashMap<_, _>>();

        let spine = dom.select("itemref").unwrap()
            .filter_map(|e| {
                let attr = e.attributes.borrow();
                let idref = try_continue!(attr.get("idref"));
                let href = try_continue!(manifest.get(idref));

                Url::options().base_url(Some(&BASE_URL)).parse(href).ok()
                    .map(|uri| root.join(uri.path().trim_left_matches('/')))
            })
            .collect::<Vec<_>>();
        if spine.is_empty() { Err(err_msg("spine list is empty!"))? };

        Ok(Book { epub, metadata, spine })
    }

    pub fn from_container(mut epub: ZipArchive<R>) -> Result<Book<R>, Error> {
        let node = kuchiki::parse_html()
            .from_utf8()
            .read_from(&mut epub.by_name("META-INF/container.xml")?)?
            .select("rootfile").unwrap()
            .next().ok_or(err_msg("No found <rootfile>."))?;

        let attrs = node.attributes.borrow();
        Book::new(epub, attrs.get("full-path").ok_or(err_msg("No `full-path` in container."))?)
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

        for path in &self.spine {
            let dom = kuchiki::parse_html()
                .from_utf8()
                .read_from(&mut self.epub.by_name(&path.to_string_lossy())?)?;

            write!(output, "{}", dom.text_contents())?;
            write!(output, "\n-----\n\n")?;
        }

        Ok(())
    }
}


#[inline]
pub fn epub2txt(input: &mut ReadSeek, output: &mut Write) -> Result<(), Error> {
    Book::from_container(ZipArchive::new(input)?)?.write_to(output)
}
