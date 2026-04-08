use crate::domain::effect::Effect;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::state::State;

/// The result of `decide` - captures all changes to be made
#[derive(Debug)]
pub struct Decision {
    pub next_state: State,
    pub metadata_updates: MetadataUpdates,
    pub effects: Vec<Effect>,
}

impl Decision {
    pub fn new(
        next_state: State,
        metadata_updates: MetadataUpdates,
        mut effects: Vec<Effect>,
    ) -> Self {
        effects.sort_by_key(|e| e.priority());
        Self {
            next_state,
            metadata_updates,
            effects,
        }
    }

    pub fn stay(current_state: State) -> Self {
        Self::new(current_state, MetadataUpdates::default(), vec![])
    }
}
