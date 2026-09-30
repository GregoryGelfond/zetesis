//! Table integrity, terminal styling and publication failure contracts.
use std::io::{self, Write};
use std::num::NonZeroUsize;
use zetesis_presentation::{Alignment, ColorMode, Column, Layout, Row, Table, TableError};

fn table() -> Table {
    Table::new(
        "Timing",
        vec![
            Column::new("Phase", Alignment::Left),
            Column::new("ms", Alignment::Right),
        ],
        vec![
            Row::new(["Grounding", "12.5"]),
            Row::new(["Solving", "2.0"]).conclusion(),
        ],
    )
    .unwrap()
}

#[test]
fn styled_tables_preserve_plain_content() {
    let mut plain = Vec::new();
    let mut styled = Vec::new();
    table()
        .write(
            &mut plain,
            Layout::new(NonZeroUsize::new(80).unwrap(), ColorMode::Never),
        )
        .unwrap();
    table()
        .write(
            &mut styled,
            Layout::new(NonZeroUsize::new(80).unwrap(), ColorMode::Always),
        )
        .unwrap();
    let mut text = String::from_utf8(styled).unwrap();
    for escape in ["\u{1b}[34m", "\u{1b}[3;90m", "\u{1b}[1;3;90m", "\u{1b}[0m"] {
        assert!(text.contains(escape));
        text = text.replace(escape, "");
    }
    assert_eq!(text.as_bytes(), plain);
    assert!(text.contains("Solving     2.0"));
}

#[test]
fn malformed_rows_are_rejected_before_publication() {
    let result = Table::new(
        "",
        vec![Column::new("A", Alignment::Left)],
        vec![Row::new(["1", "2"])],
    );
    assert_eq!(
        result.unwrap_err(),
        TableError::RowWidth {
            row: 0,
            expected: 1,
            actual: 2
        }
    );
    assert_eq!(
        Table::new("", vec![], vec![]).unwrap_err(),
        TableError::NoColumns
    );
}

#[test]
fn narrow_layout_retains_cell_content() {
    let table = Table::new(
        "Name",
        vec![Column::new("Text", Alignment::Left)],
        vec![Row::new(["abcdefgh"])],
    )
    .unwrap();
    let mut bytes = Vec::new();
    table
        .write(
            &mut bytes,
            Layout::new(NonZeroUsize::new(4).unwrap(), ColorMode::Never),
        )
        .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "Name\nText:\nabcd\nefgh\n"
    );
}

#[test]
fn cell_controls_cannot_inject_terminal_sequences() {
    let table = Table::new(
        "",
        vec![Column::new("value", Alignment::Left)],
        vec![Row::new(["\u{1b}[31m\n"])],
    )
    .unwrap();
    let mut bytes = Vec::new();
    table.write(&mut bytes, Layout::default()).unwrap();
    assert!(!bytes.contains(&0x1b));
    assert!(String::from_utf8(bytes).unwrap().contains("\\u{1b}[31m\\n"));
}

#[test]
fn wide_characters_align_by_terminal_columns() {
    let table = Table::new(
        "",
        vec![
            Column::new("A", Alignment::Left),
            Column::new("B", Alignment::Right),
        ],
        vec![Row::new(["界", "1"]), Row::new(["a", "2"])],
    )
    .unwrap();
    let mut bytes = Vec::new();
    table.write(&mut bytes, Layout::default()).unwrap();
    assert_eq!(String::from_utf8(bytes).unwrap(), "\nA   B\n界  1\na   2\n");
}

#[test]
fn writer_failure_stops_publication() {
    struct Refuse;
    impl Write for Refuse {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("renderer must not flush")
        }
    }
    assert_eq!(
        table()
            .write(&mut Refuse, Layout::default())
            .unwrap_err()
            .to_string(),
        "closed"
    );
}
