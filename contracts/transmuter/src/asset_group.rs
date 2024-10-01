use std::collections::BTreeMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::ensure;

use crate::{corruptable::Corruptable, ContractError};

#[cw_serde]
pub struct AssetGroup {
    denoms: Vec<String>,
    is_corrupted: bool,
}

impl AssetGroup {
    pub fn new(denoms: Vec<String>) -> Self {
        Self {
            denoms,
            is_corrupted: false,
        }
    }

    pub fn denoms(&self) -> &[String] {
        &self.denoms
    }

    pub fn into_denoms(self) -> Vec<String> {
        self.denoms
    }

    pub fn add_denoms(&mut self, denoms: Vec<String>) -> &mut Self {
        self.denoms.extend(denoms);
        self
    }

    pub fn remove_denoms(&mut self, denoms: Vec<String>) -> &mut Self {
        self.denoms.retain(|d| !denoms.contains(d));
        self
    }
}

impl Corruptable for AssetGroup {
    fn is_corrupted(&self) -> bool {
        self.is_corrupted
    }

    fn mark_as_corrupted(&mut self) -> &mut Self {
        self.is_corrupted = true;
        self
    }

    fn unmark_as_corrupted(&mut self) -> &mut Self {
        self.is_corrupted = false;
        self
    }
}

#[cw_serde]
pub struct AssetGroups(BTreeMap<String, AssetGroup>);

impl Default for AssetGroups {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetGroups {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn has(&self, label: &str) -> bool {
        self.0.contains_key(label)
    }

    pub fn mark_corrupted_asset_group(&mut self, label: &str) -> Result<&mut Self, ContractError> {
        let Self(asset_groups) = self;

        asset_groups
            .get_mut(label)
            .ok_or_else(|| ContractError::AssetGroupNotFound {
                label: label.to_string(),
            })?
            .mark_as_corrupted();

        Ok(self)
    }

    pub fn unmark_corrupted_asset_group(
        &mut self,
        label: &str,
    ) -> Result<&mut Self, ContractError> {
        let Self(asset_groups) = self;

        asset_groups
            .get_mut(label)
            .ok_or_else(|| ContractError::AssetGroupNotFound {
                label: label.to_string(),
            })?
            .unmark_as_corrupted();

        Ok(self)
    }

    pub fn create_asset_group(
        &mut self,
        label: String,
        denoms: Vec<String>,
    ) -> Result<&mut Self, ContractError> {
        let Self(asset_groups) = self;

        ensure!(
            !asset_groups.contains_key(&label),
            ContractError::AssetGroupAlreadyExists {
                label: label.clone()
            }
        );

        asset_groups.insert(label, AssetGroup::new(denoms));

        Ok(self)
    }

    pub fn remove_asset_group(&mut self, label: &str) -> Result<&mut Self, ContractError> {
        let Self(asset_groups) = self;

        ensure!(
            asset_groups.remove(label).is_some(),
            ContractError::AssetGroupNotFound {
                label: label.to_string()
            }
        );

        Ok(self)
    }

    pub fn into_inner(self) -> BTreeMap<String, AssetGroup> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_remove_denoms() {
        let mut group = AssetGroup::new(vec!["denom1".to_string(), "denom2".to_string()]);

        // Test initial state
        assert_eq!(group.denoms(), &["denom1", "denom2"]);

        // Test adding denoms
        group.add_denoms(vec!["denom3".to_string(), "denom4".to_string()]);
        assert_eq!(group.denoms(), &["denom1", "denom2", "denom3", "denom4"]);

        // Test adding duplicate denom
        group.add_denoms(vec!["denom2".to_string(), "denom5".to_string()]);
        assert_eq!(
            group.denoms(),
            &["denom1", "denom2", "denom3", "denom4", "denom2", "denom5"]
        );

        // Test removing denoms
        group.remove_denoms(vec!["denom2".to_string(), "denom4".to_string()]);
        assert_eq!(group.denoms(), &["denom1", "denom3", "denom5"]);

        // Test removing non-existent denom
        group.remove_denoms(vec!["denom6".to_string()]);
        assert_eq!(group.denoms(), &["denom1", "denom3", "denom5"]);
    }

    #[test]
    fn test_mark_unmark_corrupted() {
        let mut group = AssetGroup::new(vec!["denom1".to_string(), "denom2".to_string()]);

        // Test initial state
        assert!(!group.is_corrupted());

        // Test marking as corrupted
        group.mark_as_corrupted();
        assert!(group.is_corrupted());

        // Test unmarking as corrupted
        group.unmark_as_corrupted();
        assert!(!group.is_corrupted());

        // Test marking and unmarking multiple times
        group.mark_as_corrupted().mark_as_corrupted();
        assert!(group.is_corrupted());
        group.unmark_as_corrupted().unmark_as_corrupted();
        assert!(!group.is_corrupted());
    }
}
