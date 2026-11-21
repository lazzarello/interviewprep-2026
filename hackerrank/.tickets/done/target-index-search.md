# Target Index Search

Given a sorted array of distinct integers and a target value, return the index of the target or -1 if not found.

Example 1

Input:

nums = [1, 2, 3, 4, 5]
target = 3

Output:

2

Explanation:

Initialize low = 0, high = 4. Compute mid = (0 + 4) // 2 = 2. nums[2] = 3 matches target, so return 2.

Example 2

Input:

nums = [2, 4, 6, 8, 10, 12, 14, 16]
target = 16

Output:

7

Explanation:

    Start with low = 0, high = 7. mid = (0 + 7) // 2 = 3, nums[3] = 8 < 16, so set low = 4.
    Now mid = (4 + 7) // 2 = 5, nums[5] = 12 < 16, so low = 6.
    Next mid = (6 + 7) // 2 = 6, nums[6] = 14 < 16, so low = 7.
    Finally mid = (7 + 7) // 2 = 7, nums[7] = 16 equals target, return 7.

Input Format

The function receives two parameters:

    nums (INTEGER_ARRAY): a sorted array of distinct integers in strictly increasing order. Its length n satisfies 0 ≤ n ≤ 10^6 and each element satisfies -10^9 ≤ nums[i] ≤ 10^9.
    target (INTEGER): the integer value to search for in nums, satisfying -10^9 ≤ target ≤ 10^9.

Constraints

    0 <= nums.length <= 10^6
    -10^9 <= nums[i] <= 10^9 for each valid i
    For all 0 <= i < nums.length - 1, nums[i] < nums[i + 1] (strictly increasing order)
    -10^9 <= target <= 10^9

Output Format

Return a single INTEGER: the 0-based index of target in nums if it exists; otherwise return -1.

Sample Input 0

0
5

Sample Output 0

-1

Sample Input 1

1
10
10

Sample Output 1

0
