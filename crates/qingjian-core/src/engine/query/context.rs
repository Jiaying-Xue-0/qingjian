use std::collections::HashMap;

use qingjian_dictionary::Dictionary;

use crate::engine::Engine;
use crate::sentence;

impl Engine {
    pub(super) fn context_log_prob(&self, word: &str, fallback: f64, reading: f64) -> f64 {
        let log_prob = sentence::transition_log_prob(
            &*self.language_model,
            self.personal(),
            self.chain.context(),
            word,
            fallback - reading,
        );
        self.chain
            .previous()
            .and_then(|previous| self.language_model.joined_log_prob(previous, word))
            .map_or(log_prob, |joined| log_prob.max(joined))
            + reading
    }

    pub(super) fn reading_log_share(&self, word: &str, frequency: u32) -> f64 {
        let mut chars = word.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return 0.0;
        };
        self.reading_totals.get(&c).map_or(0.0, |&total| {
            ((f64::from(frequency) + 1.0) / (total as f64 + 1.0)).ln()
        })
    }
}

pub(in crate::engine) fn reading_totals(dictionary: &Dictionary) -> HashMap<char, u64> {
    let mut totals: HashMap<char, (u64, u32)> = HashMap::new();
    for entry in dictionary.entries() {
        let mut chars = entry.text.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            let total = totals.entry(c).or_default();
            total.0 += u64::from(entry.frequency);
            total.1 += 1;
        }
    }
    totals
        .into_iter()
        .filter(|(_, (_, readings))| *readings > 1)
        .map(|(c, (total, _))| (c, total))
        .collect()
}
