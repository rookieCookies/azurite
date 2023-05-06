mod utils;

use std::fmt::Write;

use colored::{Color, Colorize};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

impl SourceRange {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }

    pub fn combine(start: SourceRange, end: SourceRange) -> Self {
        Self {
            start: start.start,
            end: end.end,
        }
    }
}

// Error Creation

#[derive(Debug, PartialEq)]
pub struct Error {
    body: Vec<ErrorOption>
}

impl Error {
    pub fn new(body: Vec<ErrorOption>) -> Self { Self { body } }

    pub fn build(self, (file, source): (&str, &str)) -> String {
        self.body.into_iter().map(|x| x.build((file, source))).collect()
    }
}

pub trait CombineIntoError {
    fn combine_into_error(self) -> Error;
}

impl CombineIntoError for Vec<Error> {
    fn combine_into_error(self) -> Error {
        let mut body = Vec::with_capacity(self.iter().map(|x| x.body.len()).sum());
        self.into_iter().for_each(|mut x| {
            body.append(&mut x.body);
            if !body.last().map(|x| {
                match x {
                    ErrorOption::Text(v) => v.as_str() == "\n",
                    _ => false,
                }
            }).unwrap_or(false) {
                body.push(ErrorOption::Text(String::from("\n")))
            }
        });

        Error { body }
    }
}

#[derive(Debug, PartialEq)]
pub enum ErrorOption {
    Text(String),
    Highlight {
        range: SourceRange,
        note: Option<String>,
        colour: Color,
    }
}

pub trait ErrorBuilder {
    fn highlight(self, range: SourceRange) -> Highlight<Self> 
    where
        Self: Sized
    {
        Highlight {
            parent: self,
            range,
            note: None,
            colour: Color::BrightRed,
        }
    }



    fn text(self, text: String) -> Text<Self> 
    where
        Self: Sized
    {
        Text {
            parent: self,
            text
        }
    }


    fn empty_line(self) -> Text<Self> 
    where
        Self: Sized
    {
        Text {
            parent: self,
            text: String::from('\n')
        }
    }


    
    fn flatten(self, vec: &mut Vec<ErrorOption>);
    
    fn build(self) -> Error
    where 
        Self: Sized
    {
        let mut buffer = vec![];

        self.flatten(&mut buffer);
        
        Error::new(buffer)
    }
}

impl ErrorOption {
    pub fn build(self, (file, source): (&str, &str)) -> String {
        match self {
            ErrorOption::Text(text) => text,


            ErrorOption::Highlight { range, note, colour } => {
                let mut string = String::new();

                let start_line = utils::line_at_index(source, range.start).unwrap().1;
                let end_line   = utils::line_at_index(source, range.end - 1).unwrap().1;
                let line_size  = end_line.to_string().len();

                
                {
                    let _ = writeln!(string, "{}--> {}:{}:{}", " ".repeat(line_size), file, start_line, range.start - utils::start_of_line(source, start_line));
                    let _ = write!(string, "{} |", " ".repeat(line_size));
                }



                for (line_number, line) in source.lines().enumerate().take(end_line + 1).skip(start_line) {
                    let _ = writeln!(string);

                    let _ = writeln!(string, "{:>w$} | {}", line_number, line, w = line_size);

                    if line_number == start_line {
                        let start_of_line = utils::start_of_line(source, line_number);
                        
                        let _ = write!(string, "{:>w$} | {}{}",
                            " ".repeat(line_number.to_string().len()),
                            " ".repeat({
                                let mut count = 0;
                                for (index, i) in line.chars().enumerate() {
                                    if count >= range.start - start_of_line - 1 {
                                        count = index;
                                        break
                                    }
                                    count += i.len_utf8();
                                }
                                count
                            }),
                            "^".repeat({
                                if end_line == line_number {
                                    (range.end-range.start).max(1)
                                } else {
                                    (line.len() - (range.start - start_of_line)).max(1)
                                }
                            }).color(colour),

                            w = line_size,
                        );

                        
                    } else if line_number == end_line {
                        let _ = write!(string, "{:>w$} | {}",
                            " ".repeat(line_number.to_string().len()),
                            "^".repeat({
                                let start_of_end = utils::start_of_line(source, end_line);
                                range.end - start_of_end
                            }).color(colour),

                            w = line_size,
                        );

                        
                    } else {
                        let _ = write!(string, "{:>w$} | {}",
                            " ".repeat(line_number.to_string().len()),
                            "^".repeat(line.len()).color(colour),
                            w = line_size,
                        );
                    }

                }

                
                if let Some(note) = note {
                    let _ = writeln!(string, " {note}");
                } else {
                    let _ = writeln!(string);
                }
        
                string
            },
        }
    }
}



pub struct Highlight<T: ErrorBuilder> {
    parent: T,
    
    range: SourceRange,
    note: Option<String>,
    colour: Color,
}

impl<T: ErrorBuilder> ErrorBuilder for Highlight<T> {
    fn flatten(self, vec: &mut Vec<ErrorOption>) {
        self.parent.flatten(vec);

        vec.push(ErrorOption::Highlight { range: self.range, note: self.note, colour: self.colour })
    }
}

impl<T: ErrorBuilder> Highlight<T> {
    pub fn note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }

    pub fn colour(mut self, colour: Color) -> Self {
        self.colour = colour;
        self
    }
}



pub struct Text<T: ErrorBuilder> {
    parent: T,
    
    text: String,
}

impl<T: ErrorBuilder> ErrorBuilder for Text<T> {
    fn flatten(self, vec: &mut Vec<ErrorOption>) {
        self.parent.flatten(vec);

        vec.push(ErrorOption::Text(self.text))
    }
}



pub struct CompilerError<'a>(usize, &'a str);

impl CompilerError<'_> {
    pub fn new(id: usize, text: &str) -> CompilerError {
        CompilerError(id, text)
    }
}

impl ErrorBuilder for CompilerError<'_> {
    fn flatten(self, vec: &mut Vec<ErrorOption>) {
        let mut string = String::new();

        let _ = write!(string, "error[{:>03}]", self.0);

        string = string.red().bold().to_string();
                
        let _ = writeln!(string, " {}", self.1.white().bold());
        
        vec.push(ErrorOption::Text(string))
    }
}
