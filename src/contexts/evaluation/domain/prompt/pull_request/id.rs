/// PR 番号を表す値オブジェクト。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrId(u64);

impl PrId {
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for PrId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_shows_number() {
        assert_eq!(PrId::new(42).to_string(), "42");
    }

    #[test]
    fn as_u64_returns_inner_value() {
        assert_eq!(PrId::new(200).as_u64(), 200);
    }

    #[test]
    fn ordering_is_by_number() {
        assert!(PrId::new(1) < PrId::new(2));
    }
}
