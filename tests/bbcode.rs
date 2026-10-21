//! The Markdown to BBCode converter on whole descriptions: one that uses
//! every supported construct, and one shaped like the team's hand-written
//! Workshop pages.

use std::fs;
use std::path::Path;

use pdx_ws::bbcode::to_bbcode;

fn convert_fixture(name: &str) -> (String, String) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bbcode");
    let markdown = fs::read_to_string(dir.join(format!("{name}.md"))).unwrap();
    let expected = fs::read_to_string(dir.join(format!("{name}.bbcode"))).unwrap();
    (to_bbcode(&markdown).unwrap(), expected)
}

#[test]
fn the_fixture_description_converts() {
    let (converted, expected) = convert_fixture("description");
    assert_eq!(converted, expected);
}

/// `reference.bbcode` is a trimmed copy of the hand-written description of
/// Workshop item 3681465653 (Anbennar - Proper 2K UI), the spacing the
/// converter follows: a single newline after headings and around image
/// dividers and lists, and a blank line between paragraphs and before a
/// heading that follows text or a list. Only the line endings (CRLF) and the
/// indentation of `[*]` differ from the original.
#[test]
fn the_reference_page_converts_with_its_spacing() {
    let (converted, expected) = convert_fixture("reference");
    assert_eq!(converted, expected);
}
