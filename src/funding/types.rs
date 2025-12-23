use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Account type for internal transfers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    #[default]
    Spot = 1,
    Funding = 2,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountType::Spot => write!(f, "spot"),
            AccountType::Funding => write!(f, "funding"),
        }
    }
}

impl FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "spot" => Ok(AccountType::Spot),
            "funding" => Ok(AccountType::Funding),
            "1" => Ok(AccountType::Spot),
            "2" => Ok(AccountType::Funding),
            _ => Err(format!("Invalid account type: {}", s)),
        }
    }
}

impl From<i16> for AccountType {
    fn from(val: i16) -> Self {
        match val {
            1 => AccountType::Spot,
            2 => AccountType::Funding,
            _ => AccountType::Spot, // Default fallback
        }
    }
}

impl From<AccountType> for i16 {
    fn from(val: AccountType) -> i16 {
        val as i16
    }
}
