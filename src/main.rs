extern crate clap;
extern crate epub2txt;

use std::io::{ self, Read, Write, Cursor };
use std::fs::File;
use clap::{ Arg, App, ArgMatches };
use epub2txt::{ ReadSeek, epub2txt, Error, ErrorKind };


fn app() -> App<'static, 'static> {
    App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("input").value_name("INPUT"))
        .arg(Arg::with_name("output").short("o").long("output").value_name("OUTPUT").help("write to file."))
}

fn start<'a>(matches: ArgMatches<'a>) -> Result<(), Error> {
    let mut reader = if let Some(path) = matches.value_of("input") {
        Box::new(File::open(path)?) as Box<ReadSeek>
    } else {
        let mut input = Vec::new();
        io::stdin().read_to_end(&mut input)?;
        Box::new(Cursor::new(input)) as Box<ReadSeek>
    };
    let mut writer = if let Some(path) = matches.value_of("output") {
        Box::new(File::create(path)?) as Box<Write>
    } else {
        Box::new(io::stdout()) as Box<Write>
    };

    match epub2txt(&mut reader, &mut writer) {
        Err(Error(ErrorKind::Io(ref err), _)) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        output => output
    }
}

fn main() {
    start(app().get_matches()).unwrap();
}
