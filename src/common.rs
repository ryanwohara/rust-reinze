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

// Gray
// c1
pub fn c1(s: &str) -> String {
    format!("\x0314{}", s)
}

// Red
// c2
pub fn c2(s: &str) -> String {
    format!("\x0304{}", s)
}

// Red
// c3
pub fn c3(s: &str) -> String {
    format!("\x0305{}", s)
}

// Green
// c4
pub fn c4(s: &str) -> String {
    format!("\x0303{}", s)
}

// Yellow
// c5
pub fn c5(s: &str) -> String {
    format!("\x0307{}", s)
}

// A function for wrapping a string in brackets that are colored gray
// l
pub fn l(s: &str) -> String {
    format!("{}{}{}", c1("["), c2(s), c1("]"))
}

// A function for wrapping a string in parenthesis that are colored gray
// p
pub fn p(s: &str) -> String {
    format!("{}{}{}", c1("("), c2(s), c1(")"))
}
