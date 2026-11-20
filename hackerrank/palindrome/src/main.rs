use std::io::{self, BufRead};

fn isAlphabeticPalindrome(code: &str) -> bool {
    let mut letters = String::new();

    for c in code.chars() {
        if c.is_ascii_alphabetic() {
            letters.push(c.to_ascii_lowercase());
        }
    }
    letters.chars().eq(letters.chars().rev())
}
fn main() {
    let stdin = io::stdin();
    let mut stdin_iterator = stdin.lock().lines();

    let code = stdin_iterator.next().unwrap().unwrap();

    let result = isAlphabeticPalindrome(&code);

    println!("{}", if result { 1 } else { 0 });
}
