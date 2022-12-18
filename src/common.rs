pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f
            .to_uppercase()
            .chain(c.flat_map(|c| c.to_lowercase()))
            .collect(),
    }
}

// Two functions for encoding messages with gray and red IRC colors
// c1
pub fn c1(s: &str) -> String {
    format!("\x03\x0314{}\x03", s)
}

// c2
pub fn c2(s: &str) -> String {
    format!("\x03\x0304{}\x03", s)
}

// A function for wrapping a string in brackets that are colored gray
// l
pub fn l(s: &str) -> String {
    format!("{}{}{}", c1("["), c2(s), c1("]"))
}
