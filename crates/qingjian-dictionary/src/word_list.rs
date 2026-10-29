use std::ops::Range;
use std::path::Path;

use crate::error::DictionaryError;

type Span = (u32, u32);

/// 英文词表：按小写编码查原样大小写的词，也能按前缀补全。
/// 文件格式 TSV：`词\t编码[\t词频]`（编码缺省为词的小写，词频缺省 0）。
#[derive(Debug, Default)]
pub struct WordList {
    text: String,

    /// (编码, 原样词, 词频)，按编码字节序排好，前缀补全二分定位。
    entries: Vec<(Span, Span, u32)>,

    alphabet: Vec<u8>,
}

impl WordList {
    pub fn parse(source: &str) -> Result<Self, DictionaryError> {
        let mut text = String::with_capacity(source.len());
        let mut entries: Vec<(Span, Span, u32)> = Vec::new();
        for (index, raw) in source.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            let word = fields
                .next()
                .filter(|s| !s.is_empty())
                .ok_or(DictionaryError::Line {
                    line: index + 1,
                    reason: "missing word",
                })?;
            let code = fields
                .next()
                .filter(|s| !s.is_empty())
                .map(str::to_ascii_lowercase)
                .unwrap_or_else(|| word.to_ascii_lowercase());
            let frequency = fields
                .next()
                .map(|s| s.trim().parse::<u32>())
                .transpose()
                .map_err(|_| DictionaryError::Line {
                    line: index + 1,
                    reason: "frequency is not a non-negative integer",
                })?
                .unwrap_or(0);
            let code = append(&mut text, &code);
            let word = if word == slice(&text, code) {
                code
            } else {
                append(&mut text, word)
            };
            entries.push((code, word, frequency));
        }
        entries.sort_by(|a, b| slice(&text, a.0).cmp(slice(&text, b.0)));
        let used = entries
            .iter()
            .flat_map(|&(code, _, _)| slice(&text, code).bytes())
            .filter(u8::is_ascii)
            .fold(0u128, |used, byte| used | (1 << byte));
        let alphabet = (0..128u8).filter(|&byte| used & (1 << byte) != 0).collect();
        text.shrink_to_fit();
        entries.shrink_to_fit();
        tracing::debug!(words = entries.len(), "英文词表加载完成");
        Ok(Self {
            text,
            entries,
            alphabet,
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, DictionaryError> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    /// 输入（已小写）对应的英文词。
    pub fn get(&self, code: &str) -> Option<&str> {
        self.exactly(code)
            .first()
            .map(|&(_, word, _)| self.at(word))
    }

    pub fn variants<'a>(&'a self, code: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.exactly(code).iter().map(|&(_, word, _)| self.at(word))
    }

    pub fn spellings<'a>(&'a self, code: &'a str) -> impl Iterator<Item = (&'a str, u32)> + 'a {
        self.exactly(code)
            .iter()
            .map(|&(_, word, frequency)| (self.at(word), frequency))
    }

    /// 输入（已小写）对应的词频；词表里没有为 `None`。
    pub fn frequency(&self, code: &str) -> Option<u32> {
        self.exactly(code)
            .first()
            .map(|&(_, _, frequency)| frequency)
    }

    pub fn prefixed<'a>(
        &'a self,
        prefix: &str,
    ) -> impl Iterator<Item = (&'a str, &'a str, u32)> + use<'a> {
        let start = self.position(|code| code < prefix);
        let end = self.position(|code| code < prefix || code.starts_with(prefix));
        self.entries[start..end]
            .iter()
            .map(|&entry| self.row(entry))
    }

    /// 以 `prefix` 开头（不含正好相等的）的词里词频最高的 `limit` 个，按词频降序。
    pub fn complete(&self, prefix: &str, limit: usize) -> Vec<&str> {
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<(&str, &str, u32)> = self
            .prefixed(prefix)
            .filter(|(code, _, _)| *code != prefix)
            .collect();
        hits.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));
        hits.into_iter()
            .take(limit)
            .map(|(_, word, _)| word)
            .collect()
    }

    pub fn one_edit_away<'a>(
        &'a self,
        code: &str,
    ) -> impl Iterator<Item = (&'a str, &'a str, u32)> + use<'a> {
        let typed = code.as_bytes();
        let mut found: Vec<usize> = Vec::new();
        let mut look_up = |candidate: &[u8]| {
            if let Ok(candidate) = std::str::from_utf8(candidate) {
                found.extend(self.range(candidate));
            }
        };
        let mut candidate: Vec<u8> = Vec::with_capacity(typed.len() + 1);
        for at in 0..typed.len() {
            candidate.clear();
            candidate.extend_from_slice(&typed[..at]);
            candidate.extend_from_slice(&typed[at + 1..]);
            look_up(&candidate);
            candidate.clear();
            candidate.extend_from_slice(typed);
            if at + 1 < typed.len() && typed[at] != typed[at + 1] {
                candidate.swap(at, at + 1);
                look_up(&candidate);
                candidate.swap(at, at + 1);
            }
            for &byte in self.alphabet.iter().filter(|&&byte| byte != typed[at]) {
                candidate[at] = byte;
                look_up(&candidate);
            }
        }
        for at in 0..=typed.len() {
            candidate.clear();
            candidate.extend_from_slice(&typed[..at]);
            candidate.push(0);
            candidate.extend_from_slice(&typed[at..]);
            for &byte in &self.alphabet {
                candidate[at] = byte;
                look_up(&candidate);
            }
        }
        found.sort_unstable();
        found.dedup();
        found
            .into_iter()
            .map(move |index| self.row(self.entries[index]))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn position(&self, before: impl Fn(&str) -> bool) -> usize {
        self.entries
            .partition_point(|&(code, _, _)| before(self.at(code)))
    }

    fn range(&self, code: &str) -> Range<usize> {
        let start = self.position(|entry| entry < code);
        let count = self.entries[start..]
            .iter()
            .take_while(|&&(entry, _, _)| self.at(entry) == code)
            .count();
        start..start + count
    }

    fn exactly(&self, code: &str) -> &[(Span, Span, u32)] {
        &self.entries[self.range(code)]
    }

    fn at(&self, span: Span) -> &str {
        slice(&self.text, span)
    }

    fn row(&self, (code, word, frequency): (Span, Span, u32)) -> (&str, &str, u32) {
        (self.at(code), self.at(word), frequency)
    }
}

fn append(text: &mut String, part: &str) -> Span {
    let start = text.len() as u32;
    text.push_str(part);
    (start, text.len() as u32)
}

fn slice(text: &str, (start, end): Span) -> &str {
    &text[start as usize..end as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_by_lowercase_code() {
        let list = WordList::parse("GitHub\tgithub\nhello\thello\niPhone\n").unwrap();
        assert_eq!(list.get("github"), Some("GitHub"));
        assert_eq!(list.get("iphone"), Some("iPhone"));
        assert_eq!(list.get("hello"), Some("hello"));
        assert_eq!(list.get("nope"), None);
    }

    #[test]
    fn keeps_every_spelling_of_a_code_most_common_first() {
        let list = WordList::parse("está\testa\t900\nesta\testa\t800\neste\teste\t700\n").unwrap();
        assert_eq!(list.get("esta"), Some("está"));
        assert_eq!(list.variants("esta").collect::<Vec<_>>(), ["está", "esta"]);
        assert_eq!(list.variants("nope").count(), 0);
        assert_eq!(list.complete("est", 9), ["está", "esta", "este"]);
    }

    #[test]
    fn completes_prefixes_by_frequency() {
        let list = WordList::parse(
            "compass\tcompass\t300\ncompany\tcompany\t900\ncompare\tcompare\t500\ncom\tcom\t100\ncomma\tcomma\n",
        )
        .unwrap();
        assert_eq!(list.complete("comp", 2), ["company", "compare"]);
        assert_eq!(
            list.complete("com", 10),
            ["company", "compare", "compass", "comma"]
        );
        assert!(list.complete("zzz", 3).is_empty());
        assert!(list.complete("", 3).is_empty());
    }

    #[test]
    fn lists_every_code_under_a_prefix_in_code_order() {
        let list = WordList::parse("compare\ncom\ncone\ncomma\ncompany\n").unwrap();
        let codes: Vec<&str> = list.prefixed("com").map(|(code, _, _)| code).collect();
        assert_eq!(codes, ["com", "comma", "company", "compare"]);
        assert_eq!(list.prefixed("x").count(), 0);
    }

    #[test]
    fn finds_every_code_one_edit_away() {
        let list =
            WordList::parse("hello\nhelp\nheld\nhel\nheol\nhole\nhel-o\nworld\nhelo\nHELO\thelo\n")
                .unwrap();
        let codes: Vec<&str> = list
            .one_edit_away("helo")
            .map(|(code, _, _)| code)
            .collect();
        assert_eq!(codes, ["hel", "hel-o", "held", "hello", "help", "heol"]);
        let hello: Vec<&str> = list
            .one_edit_away("hello")
            .map(|(_, word, _)| word)
            .collect();
        assert_eq!(hello, ["hel-o", "helo", "HELO"]);
    }

    #[test]
    fn one_edit_away_returns_every_spelling_once() {
        let list = WordList::parse("está\testa\t900\nesta\testa\t800\nfeed\n").unwrap();
        let words: Vec<(&str, u32)> = list
            .one_edit_away("esto")
            .map(|(_, word, frequency)| (word, frequency))
            .collect();
        assert_eq!(words, [("está", 900), ("esta", 800)]);
        assert_eq!(
            list.one_edit_away("fed")
                .map(|(_, word, _)| word)
                .collect::<Vec<_>>(),
            ["feed"]
        );
    }
}
