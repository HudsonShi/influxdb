//! A optimised resolver of a partition [`SortKey`].

use std::sync::Arc;

use backoff::{Backoff, BackoffConfig};
use data_types::{PartitionKey, TableId};
use iox_catalog::interface::Catalog;
use schema::sort::SortKey;

/// A resolver of [`SortKey`] from the catalog for a given [`PartitionKey`]/[`TableId`] pair.
#[derive(Debug)]
pub(crate) struct SortKeyResolver {
    partition_key: PartitionKey,
    table_id: TableId,
    backoff_config: BackoffConfig,
    catalog: Arc<dyn Catalog>,
}

impl SortKeyResolver {
    pub(crate) fn new(
        partition_key: PartitionKey,
        table_id: TableId,
        catalog: Arc<dyn Catalog>,
        backoff_config: BackoffConfig,
    ) -> Self {
        Self {
            partition_key,
            table_id,
            backoff_config,
            catalog,
        }
    }

    /// Fetch the [`SortKey`] from the [`Catalog`] for `partition_id`, retrying
    /// endlessly when errors occur.
    pub(crate) async fn fetch(self) -> Option<SortKey> {
        Backoff::new(&self.backoff_config)
            .retry_all_errors("fetch partition sort key", || async {
                let mut repos = self.catalog.repositories().await;
                let s = repos
                    .partitions()
                    .create_or_get(self.partition_key.clone(), self.table_id)
                    .await?
                    .sort_key();

                Result::<_, iox_catalog::interface::Error>::Ok(s)
            })
            .await
            .expect("retry forever")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::test_util::populate_catalog;

    const TABLE_NAME: &str = "bananas";
    const NAMESPACE_NAME: &str = "platanos";
    const PARTITION_KEY: &str = "platanos";

    #[tokio::test]
    async fn test_fetch() {
        let metrics = Arc::new(metric::Registry::default());
        let backoff_config = BackoffConfig::default();
        let catalog: Arc<dyn Catalog> =
            Arc::new(iox_catalog::mem::MemCatalog::new(Arc::clone(&metrics)));

        // Populate the catalog with the namespace / table
        let (_ns_id, table_id) = populate_catalog(&*catalog, NAMESPACE_NAME, TABLE_NAME).await;

        let partition = catalog
            .repositories()
            .await
            .partitions()
            .create_or_get(PARTITION_KEY.into(), table_id)
            .await
            .expect("should create");
        // Test: sort_key_ids from create_or_get
        assert!(partition.sort_key_ids.is_none());

        let fetcher = SortKeyResolver::new(
            PARTITION_KEY.into(),
            table_id,
            Arc::clone(&catalog),
            backoff_config.clone(),
        );

        // Set the sort key
        let catalog_state = catalog
            .repositories()
            .await
            .partitions()
            .cas_sort_key(
                &partition.transition_partition_id(),
                None,
                &["uno", "dos", "bananas"],
            )
            .await
            .expect("should update existing partition key");

        let fetched = fetcher.fetch().await;
        assert_eq!(fetched, catalog_state.sort_key());

        // Test: sort_key_ids after updating
        assert!(catalog_state.sort_key_ids.is_none());
    }
}
