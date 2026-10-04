//! Maps game ids to localization keys.
//!
//! Most ids are their own key. The rest follow conventions the games resolve
//! in code: aliases tables, quality variants (`meal:burger:1`), and object
//! variants with suffixes (`mf_saw_1`, `garden_of_stones_place`).

use std::collections::{BTreeMap, HashMap, HashSet};

pub struct Names {
    keys: HashSet<String>,
    aliases: HashMap<String, String>,
    used: HashSet<String>,
}

impl Names {
    pub fn new(keys: impl IntoIterator<Item = String>, aliases: HashMap<String, String>) -> Self {
        Self { keys: keys.into_iter().collect(), aliases, used: HashSet::new() }
    }

    fn has(&self, k: &str) -> bool {
        self.keys.contains(k)
    }

    fn lookup(&self, id: &str) -> Option<String> {
        if self.has(id) {
            return Some(id.to_string());
        }
        if let Some(a) = self.aliases.get(id).filter(|a| self.has(a)) {
            return Some(a.clone());
        }
        None
    }

    /// Name key only if the id (or its alias) is itself a key: no guessing.
    /// For ids whose fallbacks would name the wrong thing (crafts).
    pub fn exact(&mut self, id: &str) -> Option<String> {
        let key = self.lookup(id)?;
        self.used.insert(key.clone());
        Some(key)
    }

    /// Name key for an id, recording it as used.
    pub fn name(&mut self, id: &str) -> Option<String> {
        let key = candidates(id).into_iter().find_map(|c| self.lookup(&c))?;
        self.used.insert(key.clone());
        Some(key)
    }

    /// Description key for a name key (`x_d` or `d_x`), recording it.
    pub fn desc(&mut self, key: &str) -> Option<String> {
        let found = [format!("{key}_d"), format!("d_{key}")].into_iter().find(|k| self.has(k))?;
        self.used.insert(found.clone());
        Some(found)
    }

    /// Keep only the keys that were used, for every language.
    pub fn filter_locales(
        &self,
        locales: BTreeMap<String, HashMap<String, String>>,
    ) -> BTreeMap<String, BTreeMap<String, String>> {
        locales
            .into_iter()
            .map(|(lang, table)| {
                let kept = table.into_iter().filter(|(k, _)| self.used.contains(k)).collect();
                (lang, kept)
            })
            .collect()
    }
}

/// Candidate keys for an id, most specific first.
fn candidates(id: &str) -> Vec<String> {
    let mut out = vec![id.to_string()];
    // Tech names in gk1 are display strings ("Primitive forging").
    if id.contains(' ') {
        out.push(id.to_lowercase().replace(' ', "_"));
    }
    if id.contains(':') {
        let parts: Vec<&str> = id.split(':').filter(|p| !p.is_empty() && !is_number(p)).collect();
        // `meal:burger:1` -> burger, then meal.
        for p in parts.iter().skip(1).chain(parts.first()) {
            out.extend(stripped(p));
        }
    } else {
        out.extend(stripped(id).into_iter().skip(1));
    }
    out
}

/// The id, then the id with trailing `_segment`s removed one by one
/// (`tree_apple_1_3_0` -> `tree_apple_1_3` -> ... -> `tree`).
fn stripped(id: &str) -> Vec<String> {
    let mut out = vec![id.to_string()];
    let mut cur = id;
    while let Some(i) = cur.rfind('_') {
        cur = &cur[..i];
        if cur.is_empty() {
            break;
        }
        out.push(cur.to_string());
    }
    out
}

fn is_number(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(keys: &[&str]) -> Names {
        Names::new(keys.iter().map(|s| s.to_string()), HashMap::new())
    }

    #[test]
    fn direct_and_suffixes() {
        let mut n = names(&["mf_saw", "tree_apple", "nails"]);
        assert_eq!(n.name("nails").as_deref(), Some("nails"));
        assert_eq!(n.name("mf_saw_1").as_deref(), Some("mf_saw"));
        assert_eq!(n.name("tree_apple_1_3_0").as_deref(), Some("tree_apple"));
        assert_eq!(n.name("unknown"), None);
    }

    #[test]
    fn quality_variants_prefer_specific_part() {
        let mut n = names(&["meal", "burger", "flesh"]);
        assert_eq!(n.name("meal:burger:1").as_deref(), Some("burger"));
        assert_eq!(n.name("flesh:flesh_3_3").as_deref(), Some("flesh"));
    }

    #[test]
    fn aliases_and_descriptions() {
        let mut n = Names::new(
            ["t_iron_ore_2", "wooden_plank", "wooden_plank_d"].map(String::from),
            HashMap::from([("ore_metal".to_string(), "t_iron_ore_2".to_string())]),
        );
        assert_eq!(n.name("ore_metal").as_deref(), Some("t_iron_ore_2"));
        assert_eq!(n.desc("wooden_plank").as_deref(), Some("wooden_plank_d"));
        let locales = BTreeMap::from([(
            "fr".to_string(),
            HashMap::from([
                ("t_iron_ore_2".to_string(), "Minerai".to_string()),
                ("unused".to_string(), "x".to_string()),
            ]),
        )]);
        let kept = n.filter_locales(locales);
        assert!(kept["fr"].contains_key("t_iron_ore_2"));
        assert!(!kept["fr"].contains_key("unused"));
    }
}
