use serde::Serialize;

pub use inventory;

#[derive(Debug, Clone, Serialize)]
pub struct ParameterSpec {
    pub name: &'static str,
    pub kind: &'static str,
    pub required: bool,
    pub default_value: Option<&'static str>,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct StrategyMetadata {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub parameters: &'static [ParameterSpec],
}

pub struct StrategyRegistration {
    pub metadata: StrategyMetadata,
}

inventory::collect!(StrategyRegistration);

pub fn all_metadata() -> Vec<StrategyMetadata> {
    let mut values: Vec<StrategyMetadata> = inventory::iter::<StrategyRegistration>
        .into_iter()
        .map(|entry| entry.metadata.clone())
        .collect();
    values.sort_by_key(|item| item.id);
    values
}
