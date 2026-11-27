use std::process::{Command, Output};

fn app(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nms-glyph-reader"))
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pictures"))
        .args(args)
        .output()
        .unwrap()
}

fn printed(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn text_is_one_line_a_picture() {
    let output = app(&["14.jpg"]);
    assert_eq!(
        printed(&output),
        "217EB09D79D7 01D6:002F:01D6:017E Euclid\n"
    );
    assert_eq!(output.status.code(), Some(0));
    let output = app(&["14.jpg", "28.jpg", "13.jpg", "none.jpg"]);
    assert_eq!(
        printed(&output),
        "14.jpg: 217EB09D79D7 01D6:002F:01D6:017E Euclid\n\
         28.jpg: 14070E45A7B3 0FB2:008D:0C59:0407\n\
         13.jpg: no address\n\
         none.jpg: cannot open (file not found)\n"
    );
}

#[test]
fn json_is_one_object_a_line() {
    let output = app(&["-f", "json", "14.jpg", "13.jpg", "none.jpg"]);
    assert_eq!(
        printed(&output),
        "{\"file\":\"14.jpg\",\"address\":\"217EB09D79D7\",\"coords\":\"01D6:002F:01D6:017E\",\"galaxy\":\"Euclid\",\"galaxy_number\":1}\n\
         {\"file\":\"13.jpg\",\"address\":null,\"coords\":null,\"galaxy\":null,\"galaxy_number\":null}\n\
         {\"file\":\"none.jpg\",\"error\":\"file not found\"}\n"
    );
}

#[test]
fn csv_has_a_header_and_a_row_a_picture() {
    let output = app(&["--format", "csv", "14.jpg", "28.jpg", "none.jpg"]);
    assert_eq!(
        printed(&output),
        "file,address,coords,galaxy,galaxy_number,error\n\
         14.jpg,217EB09D79D7,01D6:002F:01D6:017E,Euclid,1,\n\
         28.jpg,14070E45A7B3,0FB2:008D:0C59:0407,,,\n\
         none.jpg,,,,,file not found\n"
    );
}

#[test]
fn the_exit_code_says_the_worst_answer() {
    assert_eq!(app(&["14.jpg", "28.jpg"]).status.code(), Some(0));
    assert_eq!(app(&["14.jpg", "13.jpg"]).status.code(), Some(1));
    assert_eq!(
        app(&["14.jpg", "13.jpg", "none.jpg"]).status.code(),
        Some(2)
    );
    assert_eq!(app(&["-f", "xml", "14.jpg"]).status.code(), Some(2));
}
