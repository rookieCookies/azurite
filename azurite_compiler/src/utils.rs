use std::{cmp::min, collections::HashMap};

pub fn damerau_levenshtein(s: &str, t: &str) -> usize {
    // get length of unicode chars
    let len_s = s.chars().count();
    let len_t = t.chars().count();
    let max_distance = len_t + len_s;

    // initialize the matrix
    let mut mat: Vec<Vec<usize>> = vec![vec![0; len_t + 2]; len_s + 2];
    mat[0][0] = max_distance;
    for i in 0..=len_s {
        mat[i+1][0] = max_distance;
        mat[i+1][1] = i;
    }
    for i in 0..=len_t {
        mat[0][i+1] = max_distance;
        mat[1][i+1] = i;
    }

    let mut char_map: HashMap<char, usize> = HashMap::new();
    // apply edit operations
    for (i, s_char) in s.chars().enumerate() {
        let mut db = 0;
        let i = i + 1;
        
        for (j, t_char) in t.chars().enumerate() {
            let j = j + 1;
            let last = *char_map.get(&t_char).unwrap_or(&0);

            let cost = if s_char == t_char { 0 } else { 1 };
            mat[i+1][j+1] = min4(
                mat[i+1][j] + 1,     // deletion
                mat[i][j+1] + 1,     // insertion 
                mat[i][j] + cost,    // substitution
                mat[last][db] + (i - last - 1) + 1 + (j - db - 1) // transposition
            );

            // that's like s_char == t_char but more efficient
            if cost == 0 {
                db = j;
            }
        }

        char_map.insert(s_char, i);
    }

    mat[len_s + 1][len_t + 1]
}

pub fn find_similar_string<'a>(target_str: &str, strings: &[&'a str], treshold: usize) -> Option<&'a str> {
    let mut min = treshold;
    let mut current_str = "";

    for identifier in strings {
        let value = damerau_levenshtein(target_str, identifier);
        if value < min {
            min = value;
            current_str = identifier;
        }
    }
    if current_str.is_empty() {
        None
    } else {
        Some(current_str)
    }
}

#[inline]
pub fn min4(a: usize, b: usize, c: usize, d: usize) -> usize {
    min(min(min(a, b), c), d)
}