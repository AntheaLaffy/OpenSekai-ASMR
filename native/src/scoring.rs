//! One chart-normalized rule for manual play, Auto and silent practice.
//! Integer accumulation preserves fractional points instead of losing them at
//! every hit. Same-time chords share their combo bonus, independent of key order.
pub const RULE: &str = "ojsk-dynamic-v1";
pub const MAX_SCORE: u32 = 1_000_000;

pub fn weight(category: i32, critical: bool, generated: bool) -> u16 {
    let base = if generated || category == 12 {
        10 // Hidden sustain checkpoints must not outweigh deliberate contacts.
    } else {
        match category {
            3 | 8 => 125,
            2 | 4 | 6 => 50,
            _ => 100,
        }
    };
    base * if critical { 2 } else { 1 }
}

struct Item {
    weight: u16,
    group: usize,
    ideal_combo: u32,
    awarded: bool,
}
pub struct Score {
    items: Vec<Item>,
    group_combo: Vec<Option<u32>>,
    total: u128,
    maximum: u128,
    earned: u128,
    points: u32,
}
impl Score {
    /// Contacts arrive in chart-time order. Combo bonus grows smoothly from
    /// 1x to 1.5x across the chart; breaks reset it without subtracting points.
    pub fn new(contacts: &[(f32, u16)]) -> Self {
        let total = contacts.len() as u128;
        let mut items = Vec::with_capacity(contacts.len());
        let mut group_combo = Vec::new();
        let mut maximum = 0;
        let mut ideal_combo = 0;
        for (index, &(time, weight)) in contacts.iter().enumerate() {
            if index == 0 || contacts[index - 1].0 != time {
                group_combo.push(None);
                ideal_combo = index as u32;
            }
            maximum += u128::from(weight) * (2 * total + u128::from(ideal_combo)) * 100;
            items.push(Item {
                weight,
                group: group_combo.len() - 1,
                ideal_combo,
                awarded: false,
            });
        }
        Self {
            items,
            group_combo,
            total,
            maximum,
            earned: 0,
            points: 0,
        }
    }
    pub fn award(&mut self, index: usize, accuracy: u32, combo_before: u32) -> u32 {
        let item = &mut self.items[index];
        if item.awarded {
            return 0;
        }
        item.awarded = true;
        let combo = *self.group_combo[item.group].get_or_insert(combo_before.min(item.ideal_combo));
        self.earned += u128::from(item.weight)
            * (2 * self.total + u128::from(combo))
            * u128::from(accuracy.min(100));
        let points = (self.earned * u128::from(MAX_SCORE))
            .checked_div(self.maximum)
            .unwrap_or(0)
            .min(u128::from(MAX_SCORE)) as u32;
        let gain = points - self.points;
        self.points = points;
        gain
    }
}
