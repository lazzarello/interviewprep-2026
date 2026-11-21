# Check for Non-Identical String Rotation

Given two strings s1 and s2, return 1 if s2 is a rotation of s1 but not identical to s1, otherwise return 0.

Example

Input:

s1 = abcde
s2 = cdeab

Output:

True

Explanation:

- s2 ('cdeab') is a non-trivial rotation of s1 ('abcde').
- If you rotate 'abcde' left by 2 positions, you get 'cdeab'.
- Since s2 is not equal to s1 and is a rotation, the output is true.

Input Format

    The first line contains the string s1, followed by s2 on the next line.

Constraints

    1 <= |s1| <= 1000
    1 <= |s2| <= 1000
    |s1| = |s2|
    s1 & s2 both consists of lowercase English letters ('a'-'z') only

Output Format

    The function returns a single BOOLEAN value, 1 for True and 0 for False

Sample Input 0

a
a

Sample Output 0

0

Sample Input 1

a
b

Sample Output 1

0
