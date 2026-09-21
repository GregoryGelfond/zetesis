use crate::{ColorMode, Role};
use std::fmt;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Cell alignment in a horizontal table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    /// Align names and descriptive text on the left.
    Left,
    /// Align numbers and units on the right.
    Right,
}

/// One column's label and alignment.
#[derive(Clone, Debug)]
pub struct Column {
    label: String,
    alignment: Alignment,
}
impl Column {
    /// Construct a column, escaping control characters in its label.
    #[must_use]
    pub fn new(label: &str, alignment: Alignment) -> Self {
        Self {
            label: escaped(label),
            alignment,
        }
    }
}

/// One complete row. Emphasis identifies a total or conclusion, not a rank.
#[derive(Clone, Debug)]
pub struct Row {
    cells: Vec<String>,
    conclusion: bool,
}
impl Row {
    /// Construct a row, escaping control characters in every cell.
    #[must_use]
    pub fn new(cells: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self {
            cells: cells
                .into_iter()
                .map(|cell| escaped(cell.as_ref()))
                .collect(),
            conclusion: false,
        }
    }
    /// Emphasize this row as a total or conclusion.
    #[must_use]
    pub const fn conclusion(mut self) -> Self {
        self.conclusion = true;
        self
    }
}

/// Rendering capabilities supplied by the caller.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    width: NonZeroUsize,
    color: ColorMode,
}
impl Layout {
    /// Use this available column count and explicit styling policy.
    #[must_use]
    pub const fn new(width: NonZeroUsize, color: ColorMode) -> Self {
        Self { width, color }
    }
}
impl Default for Layout {
    fn default() -> Self {
        Self::new(NonZeroUsize::new(80).unwrap(), ColorMode::Auto)
    }
}

/// A complete human table view. Construction retains text proportional to its
/// input. Rendering scans it once for widths and once for output; it neither
/// collects solver results nor computes statistical summaries.
#[derive(Clone, Debug)]
pub struct Table {
    title: String,
    columns: Vec<Column>,
    rows: Vec<Row>,
}

/// A table's shape is invalid; no bytes have been written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    /// A table requires at least one column.
    NoColumns,
    /// A row has a different number of cells from the header.
    RowWidth {
        /// Zero-based row index.
        row: usize,
        /// Required column count.
        expected: usize,
        /// Supplied column count.
        actual: usize,
    },
}
impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoColumns => f.write_str("a table requires at least one column"),
            Self::RowWidth {
                row,
                expected,
                actual,
            } => write!(f, "table row {row} has {actual} cells; expected {expected}"),
        }
    }
}
impl std::error::Error for TableError {}

impl Table {
    /// Validate a complete table shape before any publication.
    ///
    /// # Errors
    /// Returns an error for missing columns or a mismatched row.
    pub fn new(title: &str, columns: Vec<Column>, rows: Vec<Row>) -> Result<Self, TableError> {
        if columns.is_empty() {
            return Err(TableError::NoColumns);
        }
        for (row, value) in rows.iter().enumerate() {
            if value.cells.len() != columns.len() {
                return Err(TableError::RowWidth {
                    row,
                    expected: columns.len(),
                    actual: value.cells.len(),
                });
            }
        }
        Ok(Self {
            title: escaped(title),
            columns,
            rows,
        })
    }

    /// Render every cell. Wide tables become labelled records, with wrapping;
    /// values are never truncated to fit. A single wide glyph can exceed a
    /// one-column layout. Styling does not contribute to measured widths.
    ///
    /// # Errors
    /// Returns the writer's first error; no retry or flush is performed.
    pub fn write(&self, output: &mut impl Write, layout: Layout) -> io::Result<()> {
        layout
            .color
            .styled(output, Role::Label, format_args!("{}", self.title))?;
        writeln!(output)?;
        let widths: Vec<usize> = self
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                self.rows
                    .iter()
                    .map(|row| row.cells[index].width())
                    .fold(column.label.width(), usize::max)
            })
            .collect();
        let horizontal = widths
            .iter()
            .try_fold(0usize, |sum, width| sum.checked_add(*width))
            .and_then(|sum| {
                self.columns
                    .len()
                    .checked_sub(1)
                    .and_then(|gaps| gaps.checked_mul(2))
                    .and_then(|gaps| sum.checked_add(gaps))
            })
            .is_some_and(|width| width <= layout.width.get());
        if horizontal {
            self.horizontal(output, layout.color, &widths)
        } else {
            self.vertical(output, layout)
        }
    }

    fn horizontal(
        &self,
        output: &mut impl Write,
        color: ColorMode,
        widths: &[usize],
    ) -> io::Result<()> {
        for (index, column) in self.columns.iter().enumerate() {
            if index > 0 {
                write!(output, "  ")?;
            }
            cell(
                output,
                &column.label,
                widths[index],
                column.alignment,
                color,
                Role::Label,
            )?;
        }
        writeln!(output)?;
        for row in &self.rows {
            for (index, value) in row.cells.iter().enumerate() {
                if index > 0 {
                    write!(output, "  ")?;
                }
                cell(
                    output,
                    value,
                    widths[index],
                    self.columns[index].alignment,
                    color,
                    if row.conclusion {
                        Role::Conclusion
                    } else if index == 0 {
                        Role::Label
                    } else {
                        Role::Metadata
                    },
                )?;
            }
            writeln!(output)?;
        }
        Ok(())
    }

    fn vertical(&self, output: &mut impl Write, layout: Layout) -> io::Result<()> {
        for (index, row) in self.rows.iter().enumerate() {
            if index > 0 {
                writeln!(output)?;
            }
            for (column, value) in self.columns.iter().zip(&row.cells) {
                layout
                    .color
                    .styled(output, Role::Label, format_args!("{}:", column.label))?;
                writeln!(output)?;
                wrapped(
                    output,
                    value,
                    layout.width.get(),
                    layout.color,
                    if row.conclusion {
                        Role::Conclusion
                    } else {
                        Role::Metadata
                    },
                )?;
            }
        }
        Ok(())
    }
}

fn cell(
    output: &mut impl Write,
    text: &str,
    width: usize,
    alignment: Alignment,
    color: ColorMode,
    role: Role,
) -> io::Result<()> {
    let padding = width - text.width();
    if alignment == Alignment::Right {
        write!(output, "{:padding$}", "")?;
    }
    color.styled(output, role, format_args!("{text}"))?;
    if alignment == Alignment::Left {
        write!(output, "{:padding$}", "")?;
    }
    Ok(())
}

fn wrapped(
    output: &mut impl Write,
    text: &str,
    width: usize,
    color: ColorMode,
    role: Role,
) -> io::Result<()> {
    let mut start = 0;
    let mut columns = 0usize;
    for (offset, character) in text.char_indices() {
        let next = character.width().unwrap_or(0);
        if columns.saturating_add(next) > width && offset > start {
            color.styled(output, role, format_args!("{}", &text[start..offset]))?;
            writeln!(output)?;
            start = offset;
            columns = 0;
        }
        columns = columns.saturating_add(next);
    }
    color.styled(output, role, format_args!("{}", &text[start..]))?;
    writeln!(output)
}

fn escaped(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() {
            result.extend(character.escape_default());
        } else {
            result.push(character);
        }
    }
    result
}
