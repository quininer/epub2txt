extern crate clap;
extern crate epub2txt;

use std::io::{ self, Read, Write, Cursor };
use std::fs::File;
use clap::{ Arg, App };
use epub2txt::{ ReadSeek, epub2txt };


fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("input").value_name("INPUT"))
        .arg(Arg::with_name("output").short("o").long("output").value_name("OUTPUT").help("write to file."))
        .get_matches();

    epub2txt(
        &mut if let Some(path) = matches.value_of("input") {
            Box::new(File::open(path).unwrap()) as Box<ReadSeek>
        } else {
            let mut input = Vec::new();
            io::stdin().read_to_end(&mut input).unwrap();
            Box::new(Cursor::new(input)) as Box<ReadSeek>
        },
        &mut if let Some(path) = matches.value_of("output") {
            Box::new(File::create(path).unwrap()) as Box<Write>
        } else {
            Box::new(io::stdout()) as Box<Write>
        }
    ).unwrap();
}
