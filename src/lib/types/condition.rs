use std::fmt;
use std::str::FromStr;

use regex::Regex;
use serde::de::{self, IntoDeserializer, Visitor};
use serde::Deserialize;

use super::variable::Variable;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Op(Operation),
    HasItem(String),
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub name: String,
    pub op: ComparisonOp,
    pub value: Variable,
}

impl<'de> de::Deserialize<'de> for Operation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["name", "op", "value"];
        lazy_static! {
            static ref RE_CONDITION: Regex = Regex::new(
                r#"(?x)
                    ^ \s*
                    (?P<name> \S+)
                    \s+ (?P<op> \S+)
                    \s+ (?P<value> \S+)
                    \s* $
                "#
            )
            .unwrap();
        }

        struct OperationVisitor;

        impl<'de> Visitor<'de> for OperationVisitor {
            type Value = Operation;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an object with fields \"name\", \"op\", \"value\";
                    \" an array of [<name>, <op>, <value>];
                    \" or a string with the format \"<name> <op> <value>\"",
                )
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut name: Option<String> = None;
                let mut op: Option<ComparisonOp> = None;
                let mut value: Option<Variable> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => match name {
                            Some(_) => return Err(de::Error::duplicate_field("name")),
                            None => name = Some(map.next_value()?),
                        },
                        "op" => match op {
                            Some(_) => return Err(de::Error::duplicate_field("op")),
                            None => op = Some(map.next_value()?),
                        },
                        "value" => match value {
                            Some(_) => return Err(de::Error::duplicate_field("value")),
                            None => value = Some(map.next_value()?),
                        },
                        other => return Err(de::Error::unknown_field(other, FIELDS)),
                    }
                }

                Ok(Operation {
                    name: name.ok_or_else(|| de::Error::missing_field("name"))?,
                    op: op.ok_or_else(|| de::Error::missing_field("op"))?,
                    value: value.ok_or_else(|| de::Error::missing_field("value"))?,
                })
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                Ok(Operation {
                    name: seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(0, &self))?,
                    op: seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(1, &self))?,
                    value: seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(2, &self))?,
                })
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match RE_CONDITION.captures(s) {
                    Some(caps) => Ok(Operation {
                        name: caps["name"].to_owned(),
                        op: caps["op"].parse().map_err(|_| {
                            de::Error::invalid_value(
                                de::Unexpected::Str(&caps["op"]),
                                &"a valid operator",
                            )
                        })?,
                        value: {
                            let value = &caps["value"];
                            value
                                .parse::<i32>()
                                .map(Variable::Num)
                                .or_else(|_| value.parse::<bool>().map(Variable::Bool))
                                .or_else(|_| value.parse::<String>().map(Variable::Str))
                                .map_err(|_| {
                                    de::Error::invalid_value(
                                        de::Unexpected::Str(value),
                                        &"a number, boolean, or string",
                                    )
                                })?
                        },
                    }),
                    None => {
                        return Err(de::Error::invalid_value(
                            de::Unexpected::Str(s),
                            &"a string with the format \"<var> <op> <value>\"",
                        ))
                    }
                }
            }
        }

        deserializer.deserialize_any(OperationVisitor)
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    #[serde(rename = "==")]
    EQ,
    #[serde(rename = "!=")]
    NEQ,
    #[serde(rename = ">")]
    GT,
    #[serde(rename = ">=")]
    GTE,
    #[serde(rename = "<")]
    LT,
    #[serde(rename = "<=")]
    LTE,
}

impl fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ComparisonOp::EQ => "==",
            ComparisonOp::NEQ => "!=",
            ComparisonOp::GT => ">",
            ComparisonOp::GTE => ">=",
            ComparisonOp::LT => "<",
            ComparisonOp::LTE => "<=",
        })
    }
}

impl FromStr for ComparisonOp {
    type Err = de::value::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s.into_deserializer())
    }
}
