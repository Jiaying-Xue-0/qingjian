use super::*;

const FLUENT_READINGS: usize = 4;

impl Engine {
    pub(super) fn prefer_fluent_reading(&self, segmentations: &mut [Segmentation]) {
        let Some(first) = segmentations.first() else {
            return;
        };
        let syllables = first.syllables.len();
        let incomplete = first.incomplete_count();
        if syllables < 2 {
            return;
        }
        let ties = segmentations
            .iter()
            .take(FLUENT_READINGS)
            .take_while(|s| s.syllables.len() == syllables && s.incomplete_count() == incomplete)
            .count();
        if ties < 2 {
            return;
        }
        let best = segmentations[..ties]
            .iter()
            .enumerate()
            .filter_map(|(index, segmentation)| {
                self.convert_sentence_with(&segmentation.patterns(), false, true)
                    .filter(|conversion| !conversion.has_placeholder())
                    .map(|conversion| (index, conversion.score))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)));
        if let Some((index, _)) = best
            && index > 0
        {
            segmentations[..=index].rotate_right(1);
        }
    }
}
