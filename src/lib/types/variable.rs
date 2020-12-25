use std::fmt;
use std::str::FromStr;

use serde::de::{self, IntoDeserializer};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Variable {
    Num(i32),
    Bool(bool),
    Str(String),
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Variable::Num(value) => value.to_string(),
            Variable::Bool(value) => value.to_string(),
            Variable::Str(value) => value.escape_default().to_string(),
        };
        f.write_str(&s)
    }
}

impl FromStr for Variable {
    type Err = de::value::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s.into_deserializer())
    }
}

impl Variable {
    pub fn type_(&self) -> VarType {
        match self {
            Variable::Num(_) => VarType::Num,
            Variable::Bool(_) => VarType::Bool,
            Variable::Str(_) => VarType::Str,
        }
    }

    pub fn type_eq(&self, other: &Variable) -> bool {
        self.type_() == other.type_()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Num,
    Bool,
    Str,
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            VarType::Num => "number",
            VarType::Bool => "boolean",
            VarType::Str => "string",
        })
    }
}
