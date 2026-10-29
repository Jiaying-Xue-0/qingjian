//! 英文模式（Caps Lock 亮着）的候选：敲的字母不当拼音，按英文词表给精确词、前缀补全和拼错一个字母的纠正。
//!
//! 空格与标点仍原样上屏敲的字母，候选只在按 Tab 或方向键选中时才用上，所以候选再多也不碍事。

mod edit;

pub use edit::within_one_edit;
use edit::{is_transposition, typo_cost};
use qingjian_dictionary::WordList;

/// 拼错纠正至少要几个字母：三个字母以内一处编辑能变出太多词，只补全不纠正。
pub const MIN_CORRECTION_LETTERS: usize = 4;

const COMPLETION_COST: i64 = 350;

const AUTOCORRECT_GAP: u32 = 1000;

const AUTOCORRECT_FLOOR: u32 = 4000;

pub fn suggest(
    lists: &[&WordList],
    typed: &str,
    weight: impl Fn(&str) -> u32,
    limit: usize,
) -> Vec<String> {
    let code = typed.to_ascii_lowercase();
    if code.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut result: Vec<String> = Vec::with_capacity(limit);
    if let Some(words) = lists.iter().find(|words| words.get(&code).is_some()) {
        for word in words.variants(&code).take(limit) {
            let word = adapt_case(word, typed);
            if !result.contains(&word) {
                result.push(word);
            }
        }
    }
    let letters_only = code.bytes().all(|b| b.is_ascii_lowercase());
    let mut hits: Vec<(&str, u32, i64)> = Vec::new();
    for words in lists {
        for (entry_code, word, frequency) in words.prefixed(&code) {
            if entry_code.len() > code.len() {
                let cost = COMPLETION_COST * (entry_code.len() - code.len()) as i64;
                hits.push((word, weight(word), i64::from(frequency) - cost));
            }
        }
        if !letters_only {
            continue;
        }
        for (entry_code, word, frequency) in words.one_edit_away(&code) {
            if entry_code.starts_with(&code) || !corrects(&code, entry_code) {
                continue;
            }
            if let Some(cost) = typo_cost(&code, entry_code) {
                hits.push((word, weight(word), i64::from(frequency) - cost));
            }
        }
    }
    hits.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.0.cmp(b.0))
    });
    for (word, _, _) in hits {
        if result.len() >= limit {
            break;
        }
        let word = adapt_case(word, typed);
        if !result.contains(&word) {
            result.push(word);
        }
    }
    result
}

fn corrects(code: &str, entry_code: &str) -> bool {
    if entry_code.len().abs_diff(code.len()) > 1 {
        return false;
    }
    if code.len() >= MIN_CORRECTION_LETTERS {
        return true;
    }
    code.len() == MIN_CORRECTION_LETTERS - 1 && is_transposition(code, entry_code)
}

pub fn autocorrects(lists: &[&WordList], typed: &str, best: &str) -> bool {
    let code = typed.to_ascii_lowercase();
    if best.to_lowercase() == typed.to_lowercase() || !code.bytes().all(|b| b.is_ascii_lowercase())
    {
        return false;
    }
    let spellings: Vec<(&str, u32)> = lists
        .iter()
        .flat_map(|words| words.spellings(&code))
        .collect();
    let frequency_of = |word: &str| {
        spellings
            .iter()
            .filter(|(spelling, _)| spelling.to_lowercase() == word.to_lowercase())
            .map(|(_, frequency)| *frequency)
            .max()
    };
    match (frequency_of(typed), frequency_of(best)) {
        (Some(own), Some(other)) => other >= own + AUTOCORRECT_GAP,
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (None, None) => {
            if !spellings.is_empty() {
                return false;
            }
            let best_code = fold_code(best);
            corrects(&code, &best_code)
                && typo_cost(&code, &best_code).is_some()
                && lists
                    .iter()
                    .flat_map(|words| words.spellings(&best_code))
                    .any(|(word, frequency)| {
                        word.to_lowercase() == best.to_lowercase() && frequency >= AUTOCORRECT_FLOOR
                    })
        }
    }
}

pub fn fold_code(word: &str) -> String {
    word.chars()
        .flat_map(char::to_lowercase)
        .flat_map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => vec!['a'],
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' => vec!['e'],
            'í' | 'ì' | 'î' | 'ï' | 'ī' | 'į' => vec!['i'],
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' => vec!['o'],
            'ú' | 'ù' | 'û' | 'ü' | 'ū' => vec!['u'],
            'ñ' => vec!['n'],
            'ç' => vec!['c'],
            'ÿ' => vec!['y'],
            'ß' => vec!['s', 's'],
            'œ' => vec!['o', 'e'],
            'æ' => vec!['a', 'e'],
            c if c.is_ascii_lowercase() => vec![c],
            _ => Vec::new(),
        })
        .collect()
}

/// 词表里的写法按敲的大小写调整：全大写就全大写，首字母大写就首字母大写，其余照词表（iPhone 仍是 iPhone）。
fn adapt_case(word: &str, typed: &str) -> String {
    let mut chars = typed.chars();
    let first_upper = chars.next().is_some_and(|c| c.is_ascii_uppercase());
    let rest_upper = typed.len() >= 2 && typed.chars().all(|c| !c.is_ascii_lowercase());
    if first_upper && rest_upper {
        word.to_uppercase()
    } else if first_upper {
        let mut out = String::with_capacity(word.len());
        let mut word_chars = word.chars();
        if let Some(c) = word_chars.next() {
            out.extend(c.to_uppercase());
        }
        out.push_str(word_chars.as_str());
        out
    } else {
        word.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> WordList {
        WordList::parse(
            "company\tcompany\t900\ncompare\tcompare\t500\ncompass\tcompass\t300\ncomma\tcomma\t100\n\
             iPhone\tiphone\t800\nhello\thello\t1000\nhollow\thollow\t200\nhelp\thelp\t700\n",
        )
        .unwrap()
    }

    #[test]
    fn every_spelling_of_the_typed_letters_comes_before_completions() {
        let list = WordList::parse("está\testa\t900\nesta\testa\t800\nestación\testacion\t700\n").unwrap();
        assert_eq!(suggest(&[&list], "esta", |_| 0, 9), ["está", "esta", "estación"]);
        assert_eq!(suggest(&[&list], "Esta", |_| 0, 9), ["Está", "Esta", "Estación"]);
        assert_eq!(suggest(&[&list], "esta", |_| 0, 1), ["está"]);
    }

    #[test]
    fn exact_then_completions_then_corrections() {
        let list = words();
        assert_eq!(
            suggest(&[&list], "comp", |_| 0, 9),
            ["company", "compare", "compass"]
        );
        // 精确词第一，词表写法优先
        assert_eq!(suggest(&[&list], "iphone", |_| 0, 9), ["iPhone"]);
        // 拼错一个字母：helo → hello / help（差一处），hollow 差两处不算
        assert_eq!(suggest(&[&list], "helo", |_| 0, 9), ["hello", "help"]);
        // 三个字母以内不纠正；带数字的是标识符，也不纠正
        assert!(suggest(&[&list], "hlo", |_| 0, 9).is_empty());
        assert!(suggest(&[&list], "hel1", |_| 0, 9).is_empty());
        assert!(suggest(&[&list], "", |_| 0, 9).is_empty());
    }

    #[test]
    fn user_choices_and_limit_shape_the_order() {
        let list = words();
        let weight = |t: &str| u32::from(t == "compass");
        assert_eq!(suggest(&[&list], "comp", weight, 2), ["compass", "company"]);
    }

    #[test]
    fn case_follows_the_typed_letters() {
        let list = words();
        assert_eq!(suggest(&[&list], "Comp", |_| 0, 1), ["Company"]);
        assert_eq!(suggest(&[&list], "COMP", |_| 0, 1), ["COMPANY"]);
        assert_eq!(suggest(&[&list], "Iphone", |_| 0, 1), ["IPhone"]);
        assert_eq!(adapt_case("hello", "H"), "Hello");
    }

    #[test]
    fn earlier_lists_win_for_exact_words_and_pool_for_completions() {
        let user = WordList::parse("gist\tgist\t3\n").unwrap();
        let main = WordList::parse("gist\tgist\t0\ngiant\tgiant\t900\ngift\tgift\t500\n").unwrap();
        // 用户表里的 gist 精确命中，补全从两张表合起来按词频排
        assert_eq!(
            suggest(&[&user, &main], "gi", |_| 0, 9),
            ["giant", "gift", "gist"]
        );
        // 精确词后面跟着差一处编辑的 gift
        assert_eq!(suggest(&[&user, &main], "gist", |_| 0, 9), ["gist", "gift"]);
        assert_eq!(suggest(&[&main], "gis", |_| 0, 9), ["gist"]);
    }

    fn product_words() -> WordList {
        WordList::parse(
            "the\tthe\t7730\nTehran\ttehran\t3900\nhello\thello\t4720\nhelp\thelp\t5700\nheld\theld\t5300\n\
             helot\thelot\t2000\ndon't\tdont\t5900\ncan't\tcant\t5900\ncant\tcant\t3000\nit's\tits\t6100\n\
             its\tits\t5800\nI'm\tim\t6200\nyou\tyou\t7400\nmañana\tmanana\t5100\nestá\testa\t6000\n\
             esta\testa\t5900\nbecause\tbecause\t6000\nzest\tzest\t3200\n",
        )
        .unwrap()
    }

    #[test]
    fn common_corrections_beat_rare_completions() {
        let list = product_words();
        assert_eq!(suggest(&[&list], "teh", |_| 0, 3)[0], "the");
        assert_eq!(suggest(&[&list], "yuo", |_| 0, 3)[0], "you");
        assert_eq!(suggest(&[&list], "helo", |_| 0, 3)[0], "hello");
        assert_eq!(suggest(&[&list], "becuase", |_| 0, 3)[0], "because");
        assert_eq!(suggest(&[&list], "dont", |_| 0, 3)[0], "don't");
        assert_eq!(suggest(&[&list], "im", |_| 0, 3)[0], "I'm");
    }

    #[test]
    fn space_corrects_only_words_that_are_clearly_wrong() {
        let list = product_words();
        let lists = [&list];
        assert!(autocorrects(&lists, "teh", "the"));
        assert!(autocorrects(&lists, "helo", "hello"));
        assert!(autocorrects(&lists, "dont", "don't"));
        assert!(autocorrects(&lists, "Dont", "Don't"));
        assert!(autocorrects(&lists, "im", "I'm"));
        assert!(autocorrects(&lists, "cant", "can't"));
        assert!(autocorrects(&lists, "manana", "mañana"));
        assert!(!autocorrects(&lists, "its", "it's"));
        assert!(!autocorrects(&lists, "esta", "está"));
        assert!(!autocorrects(&lists, "hello", "hello"));
        assert!(!autocorrects(&lists, "tehr", "Tehran"));
        assert!(!autocorrects(&lists, "zesty", "zest"));
        assert!(!autocorrects(&lists, "jiaying", "the"));
        assert_eq!(fold_code("Mañana"), "manana");
        assert_eq!(fold_code("don't"), "dont");
    }

    #[test]
    fn eszett_counts_as_double_s_when_correcting() {
        let list = WordList::parse("Straße\tstrasse\t5000\ngroße\tgrosse\t5500\n").unwrap();
        let lists = [&list];
        assert_eq!(fold_code("Straße"), "strasse");
        assert!(autocorrects(&lists, "strase", "Straße"));
        assert!(autocorrects(&lists, "grose", "große"));
    }
}
