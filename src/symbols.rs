use std::collections::HashMap;

#[derive(Default)]
pub struct SymbolCache {
    map: HashMap<String, f64>,
}

impl SymbolCache {
    pub fn save(&mut self, symbol: String, value: f64) {
        self.map.insert(symbol, value);
    }

    pub fn get(&self, symbol: &str) -> Option<f64> {
        self.map.get(symbol).cloned()
    }

    pub fn del(&mut self, symbol: &str) -> bool {
        self.map.remove(symbol).is_some()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn entries(&self) -> Vec<(&str, f64)> {
        let mut entries: Vec<_> = self
            .map
            .iter()
            .map(|(symbol, value)| (symbol.as_str(), *value))
            .collect();

        entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        entries
    }
}
