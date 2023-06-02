const LINE_COUNT : usize = 1;


pub fn line_at_index(value: &str, index: usize) -> Option<(&str, usize)> {
    let mut index_counter = 0;
    for (i, line) in value.lines().enumerate() {
        index_counter += line.chars().map(|x| x.len_utf8()).sum::<usize>();
        index_counter += LINE_COUNT;

        if index_counter > index {
            return Some((line, i));
        }
    }
    
    Some(("", value.lines().count()))
}

pub fn start_of_line(value: &str, line_number: usize) -> usize {
    let mut counter = 0;

    for (i, line) in value.lines().enumerate() {
        if i == line_number {
            break
        }

        counter += line.chars().map(|x| x.len_utf8()).sum::<usize>();
        counter += LINE_COUNT;
    }

    counter
}