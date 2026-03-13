use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub id: String,
    pub label: String,
}

impl Term {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

pub fn root_term() -> Term {
    Term::new("BFO:0000001", "entity")
}

#[cfg(test)]
mod tests {
    use super::{Term, root_term};

    #[test]
    fn root_term_matches_expected_minimal_definition() {
        assert_eq!(root_term(), Term::new("BFO:0000001", "entity"));
    }
}
