use std::io::{self, BufRead};

/*
 * Complete the 'countResponseTimeRegressions' function below.
 *
 * The function is expected to return an INTEGER.
 * The function accepts INTEGER_ARRAY responseTimes as parameter.
 */

fn countResponseTimeRegressions(responseTimes: &[i32]) -> i32 {
    // Write your code here
    let mut sum: i64 = 0;
    let mut target: i32 = 0;

    for (i, v) in responseTimes.iter().enumerate() {
        if i as i64 == 0 {
            sum += i64::from(*v);
            continue;
        }
        let mean: i64 = sum / (i as i64);
        sum += i64::from(*v);
        if i64::from(*v) > mean {
            target += 1;
        }
    }
    target
}

fn main() {
    let stdin = io::stdin();
    let mut stdin_iterator = stdin.lock().lines();

    let responseTimes_count = stdin_iterator
        .next()
        .unwrap()
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();

    let mut responseTimes: Vec<i32> = Vec::with_capacity(responseTimes_count as usize);

    for _ in 0..responseTimes_count {
        let responseTimes_item = stdin_iterator
            .next()
            .unwrap()
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        responseTimes.push(responseTimes_item);
    }

    let result = countResponseTimeRegressions(&responseTimes);

    println!("{}", result);
}
