Check Palindrome by Filtering Non-Letters

Given a string containing letters, digits, and symbols, determine if it reads the same forwards and backwards when considering only alphabetic characters (case-insensitive).

Example

Input

code = A1b2B!a

Output

1

Explanation

- Step 1: Extract only letters → ['A','b','B','a']
- Step 2: Convert to lowercase → ['a','b','b','a']
- Step 3: Compare sequence forward and backward: 'abba' == 'abba' → true

Input Format

    A string code containing letters (A–Z, a–z), digits (0–9), and symbols

Constraints

    0 <= code.length <= 1000
    For all 0 <= i < code.length: 33 <= ASCII(code[i]) <= 126
    code contains only printable ASCII characters (letters, digits, symbols)

Output Format

    Return a boolean value: 1 if true & 0 if false.

Sample Input 0

Z

Sample Output 0

1

Sample Input 1

abc123cba

Sample Output 1

1
