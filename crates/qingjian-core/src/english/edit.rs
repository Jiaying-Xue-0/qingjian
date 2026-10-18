//! 两个小写串是否只差一处编辑（替换、插入、删除、相邻换位）。

/// `a` 与 `b` 相等，或只差一处编辑：换一个字母、多 / 少一个字母、相邻两个字母对调。
/// 英文模式拼错一个字母时用它找词，串都很短，逐字节比就够了。
pub fn within_one_edit(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    match a.len().abs_diff(b.len()) {
        0 => same_length(a, b),
        1 => {
            let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };
            one_insertion(short, long)
        }
        _ => false,
    }
}

/// 等长：最多一个位置不同，或恰好相邻两个位置对调。
fn same_length(a: &[u8], b: &[u8]) -> bool {
    let mut differences = a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y);
    let Some((first, _)) = differences.next() else {
        return true;
    };
    match differences.next() {
        None => true,
        Some((second, _)) => {
            second == first + 1
                && a[first] == b[second]
                && a[second] == b[first]
                && differences.next().is_none()
        }
    }
}

/// `long` 去掉一个字节后等于 `short`。
fn one_insertion(short: &[u8], long: &[u8]) -> bool {
    let mut i = 0;
    let mut skipped = false;
    for &byte in long {
        if i < short.len() && short[i] == byte {
            i += 1;
        } else if skipped {
            return false;
        } else {
            skipped = true;
        }
    }
    true
}

pub const TRANSPOSITION_COST: i64 = 800;
pub const DOUBLED_LETTER_COST: i64 = 300;
pub const NEAR_KEY_COST: i64 = 1500;
pub const INSERTION_COST: i64 = 1200;
pub const SUBSTITUTION_COST: i64 = 2000;

const KEY_ROWS: [&[u8]; 3] = [b"qwertyuiop", b"asdfghjkl", b"zxcvbnm"];
const ROW_OFFSETS: [f32; 3] = [0.0, 0.5, 1.5];

pub fn typo_cost(typed: &str, word: &str) -> Option<i64> {
    let (typed, word) = (typed.as_bytes(), word.as_bytes());
    if typed.len() == word.len() {
        let differences: Vec<usize> = (0..typed.len()).filter(|&i| typed[i] != word[i]).collect();
        return match differences.as_slice() {
            [i] if near_keys(typed[*i], word[*i]) => Some(NEAR_KEY_COST),
            [_] => Some(SUBSTITUTION_COST),
            [i, j] if *j == i + 1 && typed[*i] == word[*j] && typed[*j] == word[*i] => {
                Some(TRANSPOSITION_COST)
            }
            _ => None,
        };
    }
    let (short, long) = if typed.len() < word.len() {
        (typed, word)
    } else {
        (word, typed)
    };
    if long.len() != short.len() + 1 {
        return None;
    }
    let k = (0..short.len())
        .find(|&i| short[i] != long[i])
        .unwrap_or(short.len());
    if long[..k] != short[..k] || long[k + 1..] != short[k..] {
        return None;
    }
    let doubled =
        (k > 0 && long[k] == long[k - 1]) || (k + 1 < long.len() && long[k] == long[k + 1]);
    Some(if doubled {
        DOUBLED_LETTER_COST
    } else {
        INSERTION_COST
    })
}

pub fn is_transposition(typed: &str, word: &str) -> bool {
    typo_cost(typed, word) == Some(TRANSPOSITION_COST) && typed.len() == word.len()
}

fn key_position(key: u8) -> Option<(usize, f32)> {
    KEY_ROWS.iter().enumerate().find_map(|(row, keys)| {
        keys.iter()
            .position(|&k| k == key)
            .map(|column| (row, column as f32 + ROW_OFFSETS[row]))
    })
}

fn near_keys(a: u8, b: u8) -> bool {
    match (key_position(a), key_position(b)) {
        (Some((row_a, x_a)), Some((row_b, x_b))) => {
            row_a.abs_diff(row_b) <= 1 && (x_a - x_b).abs() <= 1.0
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_each_kind_of_edit() {
        assert!(within_one_edit("hello", "hello"));
        assert!(within_one_edit("hallo", "hello")); // 替换
        assert!(within_one_edit("helo", "hello")); // 少一个
        assert!(within_one_edit("helllo", "hello")); // 多一个
        assert!(within_one_edit("hlelo", "hello")); // 换位
        assert!(within_one_edit("ello", "hello")); // 开头少一个
        assert!(within_one_edit("hell", "hello")); // 结尾少一个
    }

    #[test]
    fn rejects_two_edits() {
        assert!(!within_one_edit("hxllx", "hello"));
        assert!(!within_one_edit("hlleo", "hello"));
        assert!(!within_one_edit("he", "hello"));
        assert!(!within_one_edit("hello", "olleh"));
        assert!(!within_one_edit("abcd", "acbe"));
    }

    #[test]
    fn typo_costs_follow_how_people_mistype() {
        assert_eq!(typo_cost("teh", "the"), Some(TRANSPOSITION_COST));
        assert_eq!(typo_cost("helo", "hello"), Some(DOUBLED_LETTER_COST));
        assert_eq!(typo_cost("untill", "until"), Some(DOUBLED_LETTER_COST));
        assert_eq!(typo_cost("helo", "help"), Some(NEAR_KEY_COST));
        assert_eq!(typo_cost("helo", "held"), Some(SUBSTITUTION_COST));
        assert_eq!(typo_cost("realy", "real"), Some(INSERTION_COST));
        assert_eq!(typo_cost("tommorow", "tomorrow"), None);
        assert_eq!(typo_cost("hello", "hello"), None);
        assert!(is_transposition("yuo", "you"));
        assert!(!is_transposition("yoo", "you"));
    }
}
