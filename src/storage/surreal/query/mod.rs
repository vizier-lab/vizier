use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AllAnd(pub Vec<Cond>);

impl Display for AllAnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let joined = self
            .0
            .iter()
            .map(|cond| format!("{cond}"))
            .collect::<Vec<String>>()
            .join(" AND ");

        write!(f, "{}", joined)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cond(pub String, pub Op);

impl Display for Cond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} ${}", self.0, self.1, self.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Op {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    Fuzzy,
    NFuzzy,
    AllEq,
    AnyEq,
}

impl Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Eq => "==",
                Self::Neq => "!=",
                Self::Lt => "<",
                Self::Lte => "<=",
                Self::Gt => ">",
                Self::Gte => ">=",
                Self::Fuzzy => "~",
                Self::NFuzzy => "!~",
                Self::AllEq => "?=",
                Self::AnyEq => "*=",
            }
        )
    }
}
