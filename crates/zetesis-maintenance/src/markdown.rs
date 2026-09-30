//! Markdown documents as the documentation checks read them: each line, and
//! whether it lies outside every fenced code block or inside one, with the
//! block's info string.
//!
//! A fence is a line whose text after its indentation begins with three or
//! more backticks or three or more tildes; the rest of the line is its info
//! string, which after backticks holds no backtick. A block closes at a fence
//! of the same character, at least as long as the fence that opened it, with
//! nothing after it but spaces. Any other fence inside a block is a line of
//! code, so a block of one kind can show fences of the other. A block left
//! open runs to the end of the document.

/// Where one line of a Markdown document lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Line<'a> {
    /// A line outside every fenced code block.
    Prose(&'a str),
    /// A fence opening a block, with its info string.
    Opening(&'a str),
    /// A line of a fenced block, with the info string that opened the block.
    Code {
        /// The line as written.
        text: &'a str,
        /// The info string of the fence that opened the block.
        info: &'a str,
    },
    /// The fence closing a block.
    Closing,
}

/// A fence line: its character, how many of it open the line, and the rest.
struct Fence<'a> {
    marker: char,
    length: usize,
    rest: &'a str,
}

/// The fence `line` is, if it is one.
fn fence(line: &str) -> Option<Fence<'_>> {
    let text = line.trim_start();
    let marker = text
        .chars()
        .next()
        .filter(|first| matches!(first, '`' | '~'))?;
    // Both fence characters are one byte, so the count is also a byte offset.
    let length = text
        .chars()
        .take_while(|character| *character == marker)
        .count();
    let rest = &text[length..];
    (length >= 3 && !(marker == '`' && rest.contains('`'))).then_some(Fence {
        marker,
        length,
        rest,
    })
}

/// Each line of `text` with its one-based number and where it lies.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = (usize, Line<'_>)> {
    let mut open: Option<Fence<'_>> = None;
    text.lines().enumerate().map(move |(index, line)| {
        let kind = match (&open, fence(line)) {
            (None, Some(opening)) => {
                let info = opening.rest.trim();
                open = Some(opening);
                Line::Opening(info)
            }
            (None, None) => Line::Prose(line),
            (Some(block), Some(closing))
                if closing.marker == block.marker
                    && closing.length >= block.length
                    && closing.rest.trim().is_empty() =>
            {
                open = None;
                Line::Closing
            }
            (Some(block), _) => Line::Code {
                text: line,
                info: block.rest.trim(),
            },
        };
        (index + 1, kind)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<Line<'_>> {
        lines(text).map(|(_, line)| line).collect()
    }

    #[test]
    fn backtick_and_tilde_fences_both_open_and_close_a_block() {
        for fence in ["```", "~~~"] {
            let text = format!("before\n{fence}sh\nzetesis solve\n{fence}\nafter");
            assert_eq!(
                kinds(&text),
                [
                    Line::Prose("before"),
                    Line::Opening("sh"),
                    Line::Code {
                        text: "zetesis solve",
                        info: "sh"
                    },
                    Line::Closing,
                    Line::Prose("after"),
                ]
            );
        }
    }

    #[test]
    fn a_fence_of_the_other_character_does_not_close_a_block() {
        let text = "~~~md\n```\n~~~\nafter";
        assert_eq!(
            kinds(text),
            [
                Line::Opening("md"),
                Line::Code {
                    text: "```",
                    info: "md"
                },
                Line::Closing,
                Line::Prose("after"),
            ]
        );
    }

    #[test]
    fn a_shorter_fence_does_not_close_a_block() {
        let text = "````md\n```\n````\nafter";
        assert_eq!(
            kinds(text),
            [
                Line::Opening("md"),
                Line::Code {
                    text: "```",
                    info: "md"
                },
                Line::Closing,
                Line::Prose("after"),
            ]
        );
    }

    #[test]
    fn a_fence_followed_by_text_does_not_close_a_block() {
        let text = "```\n``` sh\n```";
        assert_eq!(
            kinds(text),
            [
                Line::Opening(""),
                Line::Code {
                    text: "``` sh",
                    info: ""
                },
                Line::Closing,
            ]
        );
    }

    #[test]
    fn a_backtick_run_with_a_backtick_after_it_is_inline_code() {
        assert_eq!(kinds("```sh``` here"), [Line::Prose("```sh``` here")]);
    }

    #[test]
    fn line_numbers_count_from_one() {
        let numbers: Vec<_> = lines("a\n```\nb").map(|(number, _)| number).collect();
        assert_eq!(numbers, [1, 2, 3]);
    }
}
