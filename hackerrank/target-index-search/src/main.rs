use std::io::{self, BufRead};

fn binarySearch(nums: &[i32], target: i32) -> i32 {
    if !nums.is_empty() {
        let mut low = 0;
        let mut high = nums.len() - 1;
        while low <= high {
            let mid = (low + high) / 2;
            if nums[mid] == target {
                return mid as i32;
            }
            if nums[mid] < target {
                low = mid + 1;
            }
            if nums[mid] > target {
                if mid == 0 {
                    return -1;
                };
                high = mid - 1;
            }
        }
    }
    -1
}

fn main() {
    let stdin = io::stdin();
    let mut stdin_iterator = stdin.lock().lines();

    let nums_count = stdin_iterator
        .next()
        .unwrap()
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();

    let mut nums: Vec<i32> = Vec::with_capacity(nums_count as usize);

    for _ in 0..nums_count {
        let nums_item = stdin_iterator
            .next()
            .unwrap()
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        nums.push(nums_item);
    }

    let target = stdin_iterator
        .next()
        .unwrap()
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();

    let result = binarySearch(&nums, target);

    println!("{}", result);
}

#[cfg(test)]
mod tests {
    use super::binarySearch;

    #[test]
    fn boundary_and_search_cases() {
        let cases: &[(&[i32], i32, i32)] = &[
            (&[], 5, -1),
            (&[], -1_000_000_000, -1),
            (&[], 1_000_000_000, -1),
            (&[10], 10, 0),
            (&[10], 9, -1),
            (&[10], 11, -1),
            (&[2, 4], 2, 0),
            (&[2, 4], 4, 1),
            (&[2, 4], 1, -1),
            (&[2, 4], 3, -1),
            (&[2, 4], 5, -1),
            (&[1, 2, 3, 4, 5], 3, 2),
            (&[2, 4, 6, 8, 10, 12, 14, 16], 16, 7),
            (&[-9, -5, -1], -5, 1),
            (&[-1_000_000_000, 0, 1_000_000_000], -1_000_000_000, 0),
            (&[-1_000_000_000, 0, 1_000_000_000], 1_000_000_000, 2),
            (&[-1_000_000_000, 0, 1_000_000_000], -1, -1),
        ];

        for &(nums, target, expected) in cases {
            assert_eq!(
                binarySearch(nums, target),
                expected,
                "nums={nums:?}, target={target}"
            );
        }
    }

    #[test]
    fn maximum_length() {
        let nums: Vec<i32> = (0..1_000_000).map(|n| n * 2).collect();

        for (target, expected) in [
            (0, 0),
            (1_000_000, 500_000),
            (1_999_998, 999_999),
            (1, -1),
            (1_999_997, -1),
            (-1_000_000_000, -1),
            (1_000_000_000, -1),
        ] {
            assert_eq!(binarySearch(&nums, target), expected, "target={target}");
        }
    }
}
