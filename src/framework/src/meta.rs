pub struct PreserveStateId(u64);

impl Into<u64> for PreserveStateId {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<u64> for PreserveStateId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}
