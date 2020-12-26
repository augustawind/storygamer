use std::ops::RangeInclusive;
use std::rc::Rc;

use num_traits::clamp;
use serde::Deserialize;

use super::LinkAction;

pub static ITEM_USES_RANGE: RangeInclusive<i32> = 1..=i16::MAX as i32;
pub static ITEM_DEF_FIELDS: &[&str] = &["description", "max_uses", "effect"];

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    #[serde(default)]
    pub name: String,
    pub description: Option<String>,
    pub max_uses: Option<i32>,
    pub effect: LinkAction,
}

impl PartialEq for ItemDef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for ItemDef {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    def: Rc<ItemDef>,
    used: i32,
}

impl Item {
    pub fn new(def: &Rc<ItemDef>) -> Self {
        Item {
            def: Rc::clone(def),
            used: 0,
        }
    }

    pub fn name(&self) -> &str {
        self.def.name.as_str()
    }
    pub fn description(&self) -> Option<&str> {
        self.def.description.as_ref().map(|s| s.as_str())
    }
    pub fn max_uses(&self) -> Option<i32> {
        self.def.max_uses
    }
    pub fn effect(&self) -> &LinkAction {
        &self.def.effect
    }

    pub fn used(&self) -> i32 {
        self.used
    }
    pub fn uses_left(&self) -> Option<i32> {
        self.def.max_uses.map(|uses| uses - self.used)
    }
    pub fn mod_uses(&mut self, n: i32) -> i32 {
        let max = self.def.max_uses.unwrap_or(i32::MAX);
        let used = clamp(self.used + n, 0, max);
        let diff = used - self.used;
        self.used = used;
        diff
    }
    pub fn use_once(&mut self) -> Option<&LinkAction> {
        match self.mod_uses(1) {
            0 => None,
            _ => Some(&self.def.effect),
        }
    }

    pub fn fmt_uses(&self) -> String {
        match self.def.max_uses {
            Some(max_uses) => format!("{}/{}", self.used, max_uses),
            None => format!("{}/{}", self.used, '∞'),
        }
    }
}
