use std::io::{self, BufRead};

fn isNonTrivialRotation(s1: &str, s2: &str) -> bool {
    if s1 == s2 {
        return false;
    }
    let a = s1.as_bytes();
    let b = s2.as_bytes();

    for offset in 1..s1.len() {
        let mut all_match = true;
        for i in 0..s2.len() {
            if a[(offset + i) % a.len()] == b[i] {
                continue;
            } else {
                all_match = false;
                break;
            }
        }
        if all_match {
            return true;
        }
    }
    false
}

fn isNonTrivialRotationLin(s1: &str, s2: &str) -> bool {
    if s1 == s2 {
        return false;
    }
    let doubled = format!("{s1}{s1}");
    doubled.contains(s2)
}
fn main() {
    let stdin = io::stdin();
    let mut stdin_iterator = stdin.lock().lines();

    let s1 = stdin_iterator.next().unwrap().unwrap();

    let s2 = stdin_iterator.next().unwrap().unwrap();

    let result = isNonTrivialRotationLin(&s1, &s2);

    println!("{}", if result { 1 } else { 0 });
}
