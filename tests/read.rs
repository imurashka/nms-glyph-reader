#[test]
fn every_picture_reads_as_listed() {
    let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let list = std::fs::read_to_string(format!("{folder}/pictures.txt")).unwrap();
    let mut wrong = Vec::new();
    for line in list.lines() {
        let mut fields = line.split(' ');
        let (name, glyphs, galaxy) = (
            fields.next().unwrap(),
            fields.next().unwrap(),
            fields.next().unwrap().replace('_', " "),
        );
        let bytes = std::fs::read(format!("{folder}/pictures/{name}")).unwrap();
        let reading = nms_glyph_reader::read(&bytes).unwrap();
        let got_glyphs = reading.address.as_deref().unwrap_or("-");
        let got_galaxy = reading.galaxy.as_deref().unwrap_or("-");
        if got_glyphs != glyphs || got_galaxy != galaxy {
            wrong.push(format!(
                "{name}: expected {glyphs} {galaxy}, read {got_glyphs} {got_galaxy}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
