use super::{green, green_transparent, red, red_transparent, set_bg, white, yellow};
use iced::{Element, Length};
use iced::widget::{Column, Container, Row, text};
use similar::{Algorithm, ChangeTag, TextDiffConfig};

pub fn render_udiff<'d, Message: 'd>(
    udiff: &'d str,
    width: impl Into<Length>,
    zoom: f32,
) -> Element<'d, Message> {
    fn render_context<'a, 'b, 'c, 'd, Message: 'd>(context: &'a mut Vec<&'b str>, lines: &'c mut Vec<Element<'d, Message>>, zoom: f32) {
        for line in context.drain(..) {
            lines.push(text!("{line}").size(zoom * 14.0).color(white()).into());
        }
    }

    fn render_hunk<'a, 'b, 'c, 'd, 'e, 'f, Message: 'f>(add: &'a mut Vec<&'b str>, remove: &'c mut Vec<&'d str>, lines: &'e mut Vec<Element<'f, Message>>, zoom: f32) {
        match (add.len(), remove.len()) {
            (0, _) => {
                for line in remove.drain(..) {
                    lines.push(text!("{line}").size(zoom * 14.0).color(red()).into());
                }
            },
            (_, 0) => {
                for line in add.drain(..) {
                    lines.push(text!("{line}").size(zoom * 14.0).color(green()).into());
                }
            },
            // TODO: more fine-grained diff!
            (..3, ..3) => {
                let add = add.drain(..).collect::<Vec<_>>().join("\n");
                let add: Vec<&str> = add.split_inclusive(|ch| matches!(ch, ' ' | '\t' | '\n' | '(' | ')' | '{' | '}' | '[' | ']' | '<' | '>' | '.' | ',' | '+' | '-')).collect();
                let remove = remove.drain(..).collect::<Vec<_>>().join("\n");
                let remove: Vec<&str> = remove.split_inclusive(|ch| matches!(ch, ' ' | '\t' | '\n' | '(' | ')' | '{' | '}' | '[' | ']' | '<' | '>' | '.' | ',' | '+' | '-')).collect();

                let mut text_diff = TextDiffConfig::new();
                text_diff.algorithm(Algorithm::Patience);
                let text_diff = text_diff.diff_slices(&remove, &add);
                let mut diff1: Vec<(ChangeTag, &str)> = text_diff.iter_all_changes().filter(
                    |diff| diff.tag() != ChangeTag::Insert
                ).map(
                    |diff| (diff.tag(), diff.as_str().unwrap())
                ).collect();
                diff1.push((ChangeTag::Equal, "\n"));
                let diff2: Vec<(ChangeTag, &str)> = text_diff.iter_all_changes().filter(
                    |diff| diff.tag() != ChangeTag::Delete
                ).map(
                    |diff| (diff.tag(), diff.as_str().unwrap())
                ).collect();
                let mut curr_line = vec![];

                for (tag, s) in diff1.iter().chain(diff2.iter()) {
                    let color = match tag {
                        ChangeTag::Equal => None,
                        ChangeTag::Delete => Some(red_transparent()),
                        ChangeTag::Insert => Some(green_transparent()),
                    };

                    if s.ends_with("\n") {
                        let word = text!("{}", s.get(..(s.len() - 1)).unwrap()).size(zoom * 14.0);
                        curr_line.push(
                            if let Some(color) = color {
                                Container::new(word).style(move |_| set_bg(color)).into()
                            } else {
                                word.into()
                            },
                        );
                        lines.push(Row::from_vec(curr_line).into());
                        curr_line = vec![];
                    }

                    else {
                        let word = text!("{s}").size(zoom * 14.0);
                        curr_line.push(
                            if let Some(color) = color {
                                Container::new(word).style(move |_| set_bg(color)).into()
                            } else {
                                word.into()
                            },
                        );
                    }
                }

                if !curr_line.is_empty() {
                    lines.push(Row::from_vec(curr_line).into());
                }
            },
            _ => {
                for line in remove.drain(..) {
                    lines.push(text!("{line}").size(zoom * 14.0).color(red()).into());
                }

                for line in add.drain(..) {
                    lines.push(text!("{line}").size(zoom * 14.0).color(green()).into());
                }
            },
        }
    }

    let mut lines = vec![];
    let mut curr_context = vec![];
    let mut curr_add = vec![];
    let mut curr_remove = vec![];

    for line in udiff.lines() {
        if line.starts_with(" ") {
            if !curr_add.is_empty() || !curr_remove.is_empty() {
                render_hunk(&mut curr_add, &mut curr_remove, &mut lines, zoom);
            }

            curr_context.push(line);
        }

        else if line.starts_with("+") || line.starts_with("-") {
            if !curr_context.is_empty() {
                render_context(&mut curr_context, &mut lines, zoom);
            }

            if line.starts_with("+") {
                curr_add.push(line);
            } else {
                curr_remove.push(line);
            }
        }

        else {
            if !curr_context.is_empty() {
                render_context(&mut curr_context, &mut lines, zoom);
            }

            else if !curr_add.is_empty() || !curr_remove.is_empty() {
                render_hunk(&mut curr_add, &mut curr_remove, &mut lines, zoom);
            }

            lines.push(text!("{line}").size(zoom * 14.0).color(yellow()).into());
        }
    }

    if !curr_context.is_empty() {
        render_context(&mut curr_context, &mut lines, zoom);
    }

    else if !curr_add.is_empty() || !curr_remove.is_empty() {
        render_hunk(&mut curr_add, &mut curr_remove, &mut lines, zoom);
    }

    Column::from_vec(lines).width(width).into()
}
