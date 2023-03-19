use std::collections::HashMap;

use colored::{Color, Colorize};

use crate::static_analysis::Scope;

trait LineAt {
    fn line_at(&self, index: usize) -> Option<usize>;
    fn start_of_line(&self, line: usize) -> Option<usize>;
}

impl LineAt for String {
    fn line_at(&self, index: usize) -> Option<usize> {
        let mut line_count = 0;
        for (indx, chr) in self.chars().enumerate() {
            if indx == index {
                return Some(line_count)
            }
            if chr == '\n' {
                line_count += 1;
            }
        }
        Some(self.lines().count() - 1)
    }

    fn start_of_line(&self, target_line: usize) -> Option<usize> {
        let mut line_count = 0;
        let mut last_index = 0;
        for (index, chr) in self.chars().enumerate() {
            if chr == '\n' {
                if line_count >= target_line {
                    return Some(last_index)
                }
                last_index = index;
                line_count += 1;
            }
        }
        if line_count == target_line {
            return Some(last_index)
        }
        None
    }
}

const ORANGE: Color = Color::TrueColor {
    r: 255,
    g: 160,
    b: 100,
};

pub const FATAL: ErrorColourScheme = ErrorColourScheme {
    arrow_to_message: Color::BrightRed,
    line_number: ORANGE,
    separator: Color::BrightRed,
    arrow_to_error: Color::BrightRed,
    equal: Color::BrightRed,
    note_colour: ORANGE,
    title: "error",
    title_colour: Color::BrightRed,
};

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct ErrorColourScheme {
    pub separator: Color,
    pub title: &'static str,
    pub title_colour: Color,
    pub arrow_to_message: Color,
    pub arrow_to_error: Color,
    pub line_number: Color,
    pub note_colour: Color,
    pub equal: Color,
}

#[derive(Debug)]
pub struct Error {
    positions: Vec<(u32, u32, Highlight)>,
    name: &'static str,
    note: String,
    colour_scheme: &'static ErrorColourScheme,
    file_name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Highlight {
    Red,
    None,
}

impl Error {
    pub fn new(
        positions: Vec<(u32, u32, Highlight)>,
        name: &'static str,
        note: String,
        colour_scheme: &'static ErrorColourScheme,
        file_name: String,
    ) -> Self {
        // panic!();
        Self {
            positions,
            name,
            note,
            colour_scheme,
            file_name,
        }
    }

    pub fn trigger(mut self, file_data: &HashMap<String, Scope>) {
        let scope = file_data.get(&self.file_name).unwrap();
        let (path, data) = (self.file_name.clone(), scope.current_file.data.clone());
        let lines: Vec<_> = data.lines().collect();

        let start_line = data
            .line_at(self.positions[0].0 as usize)
            .unwrap_or_else(|| panic!("{self:?}"));
        let end_line = data.line_at(self.positions[0].1 as usize).unwrap();

        let biggest_line_number_size = (end_line + 1).to_string().len();
        let smallest_line_number_size = start_line.to_string().len();

        let empty_line_number_display = format!("{} |", " ".repeat(biggest_line_number_size))
            .color(self.colour_scheme.separator)
            .to_string();

        let mut message = String::new();
        message.push_str(
            format!(
                "\n{}: {}\n",
                self.colour_scheme
                    .title
                    .bold()
                    .color(self.colour_scheme.title_colour),
                self.name.bold()
            )
            .as_str(),
        );

        message.push_str(
            format!(
                "{}{} {}:{}:{}\n",
                " ".repeat(biggest_line_number_size),
                " -->".color(self.colour_scheme.arrow_to_message),
                path,
                start_line + 1,
                self.positions[0].0 as usize - data.start_of_line(start_line).unwrap()
            )
            .as_str(),
        );

        message.push_str(format!("{empty_line_number_display}\n",).as_str());
        let mem = std::mem::take(&mut self.positions);
        for pos in mem {
            let start_line = data.line_at(pos.0 as usize).unwrap();
            let end_line = data.line_at(pos.1 as usize).unwrap();

            self.generate_detail(
                &mut message,
                &lines,
                &data,
                (pos, (start_line, end_line)),
                biggest_line_number_size,
                &empty_line_number_display,
            );
        }

        let mut note = self.note.to_string();
        if note.contains('\n') {
            let mut first = false;
            for line in note.clone().split('\n') {
                if first {
                    note.push_str(
                        format!(
                            "\n{}         {}",
                            " ".repeat(smallest_line_number_size),
                            line.trim()
                        )
                        .as_str(),
                    );
                } else {
                    first = true;
                    note.push_str(line.trim());
                }
            }
        }

        // Add the note
        if !note.is_empty() {
            message.push_str(
                format!(
                    "{}{} {} {}\n",
                    " ".repeat(smallest_line_number_size),
                    " =".color(self.colour_scheme.equal),
                    "note:".bold().color(self.colour_scheme.note_colour),
                    note
                )
                .as_str(),
            );
        }
        println!("{message}");
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_sign_loss)]
    fn generate_detail(
        &self,
        message: &mut String,
        lines: &[&str],
        file_data: &String,
        (positions, (start_line, end_line)): ((u32, u32, Highlight), (usize, usize)),
        biggest_line_number_size: usize,
        empty_line_number_display: &String,
    ) {
        for (line_number, _) in lines.iter().enumerate().take(end_line + 1).skip(start_line) {
            let current_line: String = lines[line_number].to_string();
            let line_number_display = format!(
                "{}{} {}",
                " ".repeat(
                    ((biggest_line_number_size as i32
                        - (line_number + 1).to_string().len() as i32)
                        .max(0))
                    .try_into()
                    .unwrap()
                ),
                (line_number + 1)
                    .to_string()
                    .color(self.colour_scheme.line_number),
                "|".color(self.colour_scheme.separator)
            );

            message.push_str(format!("{line_number_display} {current_line}\n").as_str());

            if positions.2 == Highlight::None {
                message.push_str(empty_line_number_display);
                message.push('\n');
                continue;
            }

            message.push_str(
                format!(
                    "{} {}\n",
                    empty_line_number_display,
                    if line_number == start_line {
                        format!(
                            "{}{}",
                            " ".repeat(
                                (positions.0 as i32
                                    - file_data.start_of_line(line_number).unwrap() as i32
                                    - 1)
                                    .max(0) as usize
                            ),
                            "^".repeat(
                                file_data
                                    .start_of_line(line_number + 1)
                                    .unwrap_or(positions.1 as usize + 1)
                                    .max(positions.0 as usize)
                                    - positions.0 as usize
                            )
                            .color(self.colour_scheme.arrow_to_error)
                        )
                    } else if line_number == end_line {
                        format!(
                            "{}",
                            "^".repeat(
                                (positions.1 + 1) as usize
                                    - file_data.start_of_line(line_number-1).unwrap()
                                    
                            )
                            .color(self.colour_scheme.arrow_to_error)
                        )
                    } else {
                        "^".repeat(current_line.trim_end().len())
                            .color(self.colour_scheme.arrow_to_error)
                            .to_string()
                    }
                )
                .as_str(),
            );
        }
    }
}
