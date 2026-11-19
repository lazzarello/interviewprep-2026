use std::io::{self, BufRead};

/*
 * Complete the 'findSmallestMissingPositive' function below.
 *
 * The function is expected to return an INTEGER.
 * The function accepts INTEGER_ARRAY orderNumbers as parameter.
 */

fn findSmallestMissingPositive(orderNumbers: &[i32]) -> i32 {
    // shortcut: read-only platform input requires O(n) extra space; remove the copy if mutable input is supported.
    let mut orderNumbers = orderNumbers.to_vec();
    //  With n elements:
    // - The smallest missing positive must be between 1 and n + 1.
    // - So values above n don’t need tracking.
    // - We have exactly n existing slots we can repurpose to track 1 through n.
    for i in 0..orderNumbers.len() {
        loop {
            let v = orderNumbers[i];
            if v < 1 || v > orderNumbers.len() as i32 {
                break;
            }
            let dest_idx = (v - 1) as usize;
            if orderNumbers[dest_idx] != v {
                orderNumbers.swap(i, dest_idx);
            } else {
                break;
            }
        }
    }
    for i in 0..orderNumbers.len() {
        if orderNumbers[i] != (i + 1) as i32 {
            return (i + 1) as i32;
        }
    }
    (orderNumbers.len() + 1) as i32
}

fn main() {
    let stdin = io::stdin();
    let mut stdin_iterator = stdin.lock().lines();

    let orderNumbers_count = stdin_iterator
        .next()
        .unwrap()
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();

    let mut orderNumbers: Vec<i32> = Vec::with_capacity(orderNumbers_count as usize);

    for _ in 0..orderNumbers_count {
        let orderNumbers_item = stdin_iterator
            .next()
            .unwrap()
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        orderNumbers.push(orderNumbers_item);
    }

    let result = findSmallestMissingPositive(&orderNumbers);

    println!("{}", result);
}

#[cfg(test)]
mod tests {
    use super::findSmallestMissingPositive;

    #[test]
    fn finds_first_missing_positive() {
        let cases: &[(&[i32], i32)] = &[
            (&[], 1),
            (&[1], 2),
            (&[3, 4, -1, 1], 2),
            (&[1, 1], 2),
            (&[2, 2], 1),
            (&[0, -1, -2], 1),
            (&[1, 2, 3, 4, 5], 6),
            (&[2, 3, 4, 1], 5),
            (&[1_000_000_000, -1_000_000_000, 1], 2),
        ];

        for &(input, expected) in cases {
            assert_eq!(findSmallestMissingPositive(input), expected, "{input:?}");
        }
    }
}
