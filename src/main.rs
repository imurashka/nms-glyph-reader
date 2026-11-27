use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use nms_glyph_reader::{GALAXIES, Reading};

const USAGE: &str = "Usage: nms-glyph-reader [-f text|json|csv] <screenshot>...\nOr drag a screenshot onto nms-glyph-reader.exe.";
const CSV_HEADER: &str = "file,address,coords,galaxy,galaxy_number,error";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Csv,
}

fn main() -> ExitCode {
    let (format, paths) = match parse(std::env::args_os().skip(1)) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if paths.is_empty() {
        println!("{USAGE}");
        wait_if_opened_from_explorer();
        return ExitCode::from(2);
    }
    if format == Format::Csv {
        println!("{CSV_HEADER}");
    }
    let mut code = 0;
    for path in &paths {
        let file = path.display().to_string();
        let reading = std::fs::read(path)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => "file not found".to_owned(),
                _ => error.to_string(),
            })
            .and_then(|bytes| nms_glyph_reader::read(&bytes).map_err(|error| error.to_string()));
        code = code.max(match &reading {
            Ok(Reading {
                address: Some(_), ..
            }) => 0,
            Ok(_) => 1,
            Err(_) => 2,
        });
        let line = match format {
            Format::Text if paths.len() == 1 => text(&reading),
            Format::Text => format!("{file}: {}", text(&reading)),
            Format::Json => json(&file, &reading),
            Format::Csv => csv(&file, &reading),
        };
        println!("{line}");
    }
    wait_if_opened_from_explorer();
    ExitCode::from(code)
}

fn parse(mut args: impl Iterator<Item = OsString>) -> Result<(Format, Vec<PathBuf>), String> {
    let mut format = Format::Text;
    let mut paths = Vec::new();
    while let Some(arg) = args.next() {
        let name = match arg.to_str() {
            Some("-f" | "--format") => args.next().ok_or("-f needs a format: text, json or csv")?,
            Some(option) if option.starts_with("--format=") => option["--format=".len()..].into(),
            Some(option) if option.starts_with('-') && option.len() > 1 => {
                return Err(format!("unknown option {option}"));
            }
            _ => {
                paths.push(PathBuf::from(arg));
                continue;
            }
        };
        format = match name.to_str() {
            Some("text") => Format::Text,
            Some("json") => Format::Json,
            Some("csv") => Format::Csv,
            _ => {
                return Err(format!(
                    "unknown format {}: use text, json or csv",
                    name.to_string_lossy()
                ));
            }
        };
    }
    Ok((format, paths))
}

fn coords(address: &str) -> String {
    let digit = |range: std::ops::Range<usize>| u32::from_str_radix(&address[range], 16).unwrap();
    let x = (digit(9..12) + 0x7FF) & 0xFFF;
    let y = (digit(4..6) + 0x7F) & 0xFF;
    let z = (digit(6..9) + 0x7FF) & 0xFFF;
    format!("{x:04X}:{y:04X}:{z:04X}:{:04X}", digit(1..4))
}

fn galaxy_number(galaxy: &str) -> Option<usize> {
    GALAXIES
        .iter()
        .position(|name| *name == galaxy)
        .map(|index| index + 1)
}

fn text(reading: &Result<Reading, String>) -> String {
    let reading = match reading {
        Ok(reading) => reading,
        Err(error) => return format!("cannot open ({error})"),
    };
    match (&reading.address, &reading.galaxy) {
        (Some(address), Some(galaxy)) => format!("{address} {} {galaxy}", coords(address)),
        (Some(address), None) => format!("{address} {}", coords(address)),
        (None, Some(galaxy)) => format!("no address, {galaxy}"),
        (None, None) => "no address".to_owned(),
    }
}

fn json(file: &str, reading: &Result<Reading, String>) -> String {
    let string = |value: Option<&str>| value.map_or("null".to_owned(), json_string);
    let reading = match reading {
        Ok(reading) => reading,
        Err(error) => {
            return format!(
                "{{\"file\":{},\"error\":{}}}",
                json_string(file),
                json_string(error)
            );
        }
    };
    let address = reading.address.as_deref();
    let galaxy = reading.galaxy.as_deref();
    format!(
        "{{\"file\":{},\"address\":{},\"coords\":{},\"galaxy\":{},\"galaxy_number\":{}}}",
        json_string(file),
        string(address),
        string(address.map(coords).as_deref()),
        string(galaxy),
        galaxy
            .and_then(galaxy_number)
            .map_or("null".to_owned(), |number| number.to_string()),
    )
}

fn json_string(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if control < ' ' => quoted.push_str(&format!("\\u{:04x}", control as u32)),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn csv(file: &str, reading: &Result<Reading, String>) -> String {
    let (address, galaxy, error) = match reading {
        Ok(reading) => (reading.address.as_deref(), reading.galaxy.as_deref(), None),
        Err(error) => (None, None, Some(error.as_str())),
    };
    [
        Some(file.to_owned()),
        address.map(str::to_owned),
        address.map(coords),
        galaxy.map(str::to_owned),
        galaxy
            .and_then(galaxy_number)
            .map(|number| number.to_string()),
        error.map(str::to_owned),
    ]
    .iter()
    .map(|cell| csv_cell(cell.as_deref().unwrap_or("")))
    .collect::<Vec<_>>()
    .join(",")
}

fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(windows)]
fn wait_if_opened_from_explorer() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
    }
    let mut processes = [0u32; 2];
    if unsafe { GetConsoleProcessList(processes.as_mut_ptr(), 2) } == 1 {
        println!();
        println!("Press Enter to close.");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

#[cfg(not(windows))]
fn wait_if_opened_from_explorer() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(address: Option<&str>, galaxy: Option<&str>) -> Result<Reading, String> {
        let mut reading = Reading::default();
        reading.address = address.map(str::to_owned);
        reading.galaxy = galaxy.map(str::to_owned);
        Ok(reading)
    }

    fn not_found() -> Result<Reading, String> {
        Err("file not found".to_owned())
    }

    fn parsed(args: &[&str]) -> Result<(Format, Vec<PathBuf>), String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn coords_are_the_signal_booster_spelling() {
        assert_eq!(coords("217EB09D79D7"), "01D6:002F:01D6:017E");
        assert_eq!(coords("00380256EC6B"), "046A:0081:0D6D:0038");
        assert_eq!(coords("0079009D9690"), "0E8F:007F:01D8:0079");
    }

    #[test]
    fn galaxies_count_from_one() {
        assert_eq!(galaxy_number("Euclid"), Some(1));
        assert_eq!(galaxy_number("Hilbert Dimension"), Some(2));
        assert_eq!(galaxy_number(GALAXIES[255]), Some(256));
    }

    #[test]
    fn text_names_what_was_found() {
        let address = Some("217EB09D79D7");
        assert_eq!(
            text(&reading(address, Some("Euclid"))),
            "217EB09D79D7 01D6:002F:01D6:017E Euclid"
        );
        assert_eq!(
            text(&reading(address, Some("Hilbert Dimension"))),
            "217EB09D79D7 01D6:002F:01D6:017E Hilbert Dimension"
        );
        assert_eq!(
            text(&reading(address, None)),
            "217EB09D79D7 01D6:002F:01D6:017E"
        );
        assert_eq!(text(&reading(None, Some("Euclid"))), "no address, Euclid");
        assert_eq!(text(&reading(None, None)), "no address");
        assert_eq!(text(&not_found()), "cannot open (file not found)");
    }

    #[test]
    fn json_gives_every_field_and_null_for_a_missing_one() {
        assert_eq!(
            json("a.png", &reading(Some("217EB09D79D7"), Some("Euclid"))),
            r#"{"file":"a.png","address":"217EB09D79D7","coords":"01D6:002F:01D6:017E","galaxy":"Euclid","galaxy_number":1}"#
        );
        assert_eq!(
            json("b.png", &reading(None, Some("Euclid"))),
            r#"{"file":"b.png","address":null,"coords":null,"galaxy":"Euclid","galaxy_number":1}"#
        );
        assert_eq!(
            json("b.png", &reading(Some("217EB09D79D7"), None)),
            r#"{"file":"b.png","address":"217EB09D79D7","coords":"01D6:002F:01D6:017E","galaxy":null,"galaxy_number":null}"#
        );
        assert_eq!(
            json("c.png", &not_found()),
            r#"{"file":"c.png","error":"file not found"}"#
        );
        assert_eq!(
            json_string("C:\\a \"b\"\n\u{1}"),
            r#""C:\\a \"b\"\n\u0001""#
        );
    }

    #[test]
    fn csv_leaves_a_missing_cell_empty_and_quotes_as_rfc_4180() {
        assert_eq!(
            csv("a.png", &reading(Some("217EB09D79D7"), Some("Euclid"))),
            "a.png,217EB09D79D7,01D6:002F:01D6:017E,Euclid,1,"
        );
        assert_eq!(
            csv("b.png", &reading(None, Some("Euclid"))),
            "b.png,,,Euclid,1,"
        );
        assert_eq!(
            csv("b.png", &reading(Some("217EB09D79D7"), None)),
            "b.png,217EB09D79D7,01D6:002F:01D6:017E,,,"
        );
        assert_eq!(csv("c.png", &not_found()), "c.png,,,,,file not found");
        assert_eq!(
            csv("a, \"b\".png", &reading(None, None)),
            "\"a, \"\"b\"\".png\",,,,,"
        );
    }

    #[test]
    fn the_format_is_chosen_with_an_option() {
        let one = vec![PathBuf::from("a.png")];
        assert_eq!(parsed(&["a.png"]), Ok((Format::Text, one.clone())));
        assert_eq!(
            parsed(&["-f", "json", "a.png"]),
            Ok((Format::Json, one.clone()))
        );
        assert_eq!(
            parsed(&["a.png", "--format", "csv"]),
            Ok((Format::Csv, one.clone()))
        );
        assert_eq!(parsed(&["--format=text", "a.png"]), Ok((Format::Text, one)));
        assert!(parsed(&["-f", "xml", "a.png"]).is_err());
        assert!(parsed(&["-f"]).is_err());
        assert!(parsed(&["-x", "a.png"]).is_err());
    }
}
