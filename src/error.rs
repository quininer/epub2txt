error_chain!{
    foreign_links {
        Io(::std::io::Error);
        Zip(::zip::result::ZipError);
        Parse(::std::num::ParseIntError);
        Decode(::std::str::Utf8Error);
    }
}


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
