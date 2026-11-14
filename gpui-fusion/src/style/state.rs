use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, ops::BitAnd};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Hovered,
    Pressed,
    FocusVisible,
    Disabled,
    Selected,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct StateSelector(pub BTreeSet<State>);
impl From<State> for StateSelector {
    fn from(value: State) -> Self {
        Self(BTreeSet::from([value]))
    }
}
impl BitAnd for State {
    type Output = StateSelector;
    fn bitand(self, rhs: Self) -> Self::Output {
        StateSelector(BTreeSet::from([self, rhs]))
    }
}
impl BitAnd<State> for StateSelector {
    type Output = Self;
    fn bitand(mut self, rhs: State) -> Self {
        self.0.insert(rhs);
        self
    }
}
impl StateSelector {
    pub fn contains(&self, other: &Self) -> bool {
        other.0.is_subset(&self.0)
    }
}
