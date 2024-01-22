extern crate clap;
extern crate failure;
extern crate epub2txt;

use std::io::{ self, Read, Write, Cursor };
use std::fs::File;
use clap::{ Arg, App, ArgMatches };
use failure::Error;
use epub2txt::{ ReadSeek, epub2txt };


#[inline]
fn app() -> App<'static, 'static> {
    App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("input").value_name("INPUT"))
        .arg(Arg::with_name("output").short("o").long("output").value_name("OUTPUT").help("write to file."))
}

fn start(matches: &ArgMatches) -> Result<(), Error> {
    let mut reader = if let Some(path) = matches.value_of("input") {
        Box::new(File::open(path)?) as Box<dyn ReadSeek>
    } else {
        let mut input = Vec::new();
        io::stdin().read_to_end(&mut input)?;
        Box::new(Cursor::new(input)) as Box<dyn ReadSeek>
    };
    let mut writer = if let Some(path) = matches.value_of("output") {
        Box::new(File::create(path)?) as Box<dyn Write>
    } else {
        Box::new(io::stdout()) as Box<dyn Write>
    };

    match epub2txt(&mut reader, &mut writer) {
        Err(ref err) if err.downcast_ref::<io::Error>()
            .map(|err| err.kind() == io::ErrorKind::BrokenPipe)
            .unwrap_or(false) => Ok(()),
        output => output
    }
}

fn main() {
    start(&app().get_matches()).unwrap();
}
