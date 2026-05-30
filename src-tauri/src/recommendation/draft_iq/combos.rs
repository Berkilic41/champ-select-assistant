use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ComboPair {
    pub a: String,
    pub b: String,
    pub name: String,
    #[serde(rename = "type")]
    pub combo_type: String,
    pub ability_ref: String,
    pub tr: String,
    pub strength: f32,
    pub confidence: String,
}

#[derive(Debug)]
pub struct ComboDirectory {
    pairs: Vec<ComboPair>,
}

impl ComboDirectory {
    pub fn new(pairs: Vec<ComboPair>) -> Self {
        Self { pairs }
    }

    /// Returns all combos where `my_key` is one side and any of `ally_keys` is the other.
    /// Lookup is case-insensitive and symmetric (a↔b order does not matter).
    pub fn find_for_ally<'a>(&'a self, my_key: &str, ally_keys: &[&str]) -> Vec<&'a ComboPair> {
        self.pairs
            .iter()
            .filter(|p| {
                let partner = if p.a.eq_ignore_ascii_case(my_key) {
                    Some(p.b.as_str())
                } else if p.b.eq_ignore_ascii_case(my_key) {
                    Some(p.a.as_str())
                } else {
                    None
                };
                partner.is_some_and(|partner_key| {
                    ally_keys
                        .iter()
                        .any(|k| partner_key.eq_ignore_ascii_case(k))
                })
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn all_pairs(&self) -> &[ComboPair] {
        &self.pairs
    }
}

pub fn load_combos(raw_json: &str) -> Result<ComboDirectory> {
    let pairs: Vec<ComboPair> = serde_json::from_str(raw_json)?;
    Ok(ComboDirectory::new(pairs))
}
