use serde::Serialize;
use std::fmt;
/// An enum over various types of categories
#[derive(Debug, Clone, Eq, PartialEq, Copy, Serialize)]
pub enum Categories {
    #[serde(rename = "sports")]
    Sports,
    #[serde(rename = "society")]
    Society,
    #[serde(rename = "technology")]
    Technology,
    #[serde(rename = "entertainment")]
    Entertainment,
    #[serde(rename = "other")]
    Other,
    #[serde(rename = "science")]
    Science,
    #[serde(rename = "economy")]
    Economy,
    #[serde(rename = "unknown")]
    Unknown,
}
impl fmt::Display for Categories {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Economy => write!(f, "economy"),
            Self::Society => write!(f, "society"),
            Self::Technology => write!(f, "technology"),
            Self::Sports => write!(f, "sports"),
            Self::Entertainment => write!(f, "entertainment"),
            Self::Science => write!(f, "science"),
            Self::Other => write!(f, "other"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
impl Default for Categories {
    fn default() -> Self {
        Categories::Unknown
    }
}
impl Categories {
    /// Convert a category to a i32
    /// to be used in protobuf
    pub fn to_i32(self) -> i32 {
        match self {
            Self::Society => 0,
            Self::Economy => 1,
            Self::Technology => 2,
            Self::Entertainment => 3,
            Self::Sports => 4,
            Self::Science => 5,
            Self::Other => 6,
            // We should not hit unknown
            Self::Unknown => 7,
        }
    }
}
