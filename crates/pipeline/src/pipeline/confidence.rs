use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn to_f64(self) -> f64 {
        match self {
            Confidence::High => 0.95,
            Confidence::Medium => 0.7,
            Confidence::Low => 0.4,
        }
    }

    pub fn to_f32(self) -> f32 {
        self.to_f64() as f32
    }
}
