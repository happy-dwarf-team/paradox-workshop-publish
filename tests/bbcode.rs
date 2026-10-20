//! The Markdown to BBCode converter on a description that uses every
//! supported construct.

use std::fs;
use std::path::Path;

use pdx_ws::bbcode::to_bbcode;

#[test]
fn the_fixture_description_converts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bbcode");
    let markdown = fs::read_to_string(dir.join("description.md")).unwrap();
    let expected = fs::read_to_string(dir.join("description.bbcode")).unwrap();
    assert_eq!(to_bbcode(&markdown).unwrap(), expected);
}
