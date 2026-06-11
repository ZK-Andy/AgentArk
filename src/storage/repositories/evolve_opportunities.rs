use super::super::*;

/// Maximum opportunity rows any listing returns (resource-bounds rule:
/// no unbounded loads on small machines).
const MAX_EVOLVE_OPPORTUNITY_ROWS: u64 = 200;

impl Storage {
    // ==================== ArkEvolve Opportunities ====================

    /// Insert-or-refresh an opportunity by its stable content-hash id.
    /// Lifecycle fields (status/verdict/ledger/decided_at) are preserved on
    /// conflict — re-mining the same opportunity must never resurrect a
    /// dismissed one or wipe its ledger; only evidence freshens.
    pub async fn upsert_evolve_opportunity(
        &self,
        opportunity: &evolve_opportunity::Model,
    ) -> Result<()> {
        evolve_opportunity::Entity::insert(evolve_opportunity::ActiveModel {
            id: Set(opportunity.id.clone()),
            miner_key: Set(opportunity.miner_key.clone()),
            status: Set(opportunity.status.clone()),
            title: Set(opportunity.title.clone()),
            description: Set(opportunity.description.clone()),
            segment_label: Set(opportunity.segment_label.clone()),
            segment_key: Set(opportunity.segment_key.clone()),
            target_surface: Set(opportunity.target_surface.clone()),
            evidence_json: Set(opportunity.evidence_json.clone()),
            expected_benefit_json: Set(opportunity.expected_benefit_json.clone()),
            risk_json: Set(opportunity.risk_json.clone()),
            holdout_run_ids_json: Set(opportunity.holdout_run_ids_json.clone()),
            verdict_json: Set(opportunity.verdict_json.clone()),
            ledger_json: Set(opportunity.ledger_json.clone()),
            gepa_job_id: Set(opportunity.gepa_job_id.clone()),
            decided_at: Set(opportunity.decided_at.clone()),
            created_at: Set(opportunity.created_at.clone()),
            updated_at: Set(opportunity.updated_at.clone()),
        })
        .on_conflict(
            OnConflict::column(evolve_opportunity::Column::Id)
                .update_columns([
                    evolve_opportunity::Column::EvidenceJson,
                    evolve_opportunity::Column::ExpectedBenefitJson,
                    evolve_opportunity::Column::RiskJson,
                    evolve_opportunity::Column::HoldoutRunIdsJson,
                    evolve_opportunity::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    pub async fn get_evolve_opportunity(
        &self,
        id: &str,
    ) -> Result<Option<evolve_opportunity::Model>> {
        Ok(evolve_opportunity::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await?)
    }

    /// Opportunities in any of the given statuses, newest first, capped.
    pub async fn list_evolve_opportunities_by_status(
        &self,
        statuses: &[&str],
        limit: u64,
    ) -> Result<Vec<evolve_opportunity::Model>> {
        let statuses = statuses
            .iter()
            .map(|status| status.to_string())
            .collect::<Vec<_>>();
        Ok(evolve_opportunity::Entity::find()
            .filter(evolve_opportunity::Column::Status.is_in(statuses))
            .order_by_desc(evolve_opportunity::Column::UpdatedAt)
            .limit(Self::db_limit(limit.min(MAX_EVOLVE_OPPORTUNITY_ROWS)))
            .all(&self.db)
            .await?)
    }

    /// All opportunity rows newest-first (capped) — dedupe horizon for miners.
    pub async fn list_recent_evolve_opportunities(
        &self,
        limit: u64,
    ) -> Result<Vec<evolve_opportunity::Model>> {
        Ok(evolve_opportunity::Entity::find()
            .order_by_desc(evolve_opportunity::Column::UpdatedAt)
            .limit(Self::db_limit(limit.min(MAX_EVOLVE_OPPORTUNITY_ROWS)))
            .all(&self.db)
            .await?)
    }

    /// Move an opportunity through its lifecycle. Records decided_at for
    /// terminal user decisions.
    pub async fn update_evolve_opportunity_status(
        &self,
        id: &str,
        status: &str,
        verdict_json: Option<serde_json::Value>,
        gepa_job_id: Option<&str>,
    ) -> Result<Option<evolve_opportunity::Model>> {
        let Some(existing) = evolve_opportunity::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let now = chrono::Utc::now().to_rfc3339();
        let terminal = matches!(status, "dismissed" | "deployed" | "reverted" | "rejected");
        let mut active = evolve_opportunity::ActiveModel {
            id: Unchanged(existing.id.clone()),
            status: Set(status.to_string()),
            updated_at: Set(now.clone()),
            ..Default::default()
        };
        if let Some(verdict) = verdict_json {
            active.verdict_json = Set(verdict);
        }
        if let Some(job_id) = gepa_job_id {
            active.gepa_job_id = Set(Some(job_id.to_string()));
        }
        if terminal {
            active.decided_at = Set(Some(now));
        }
        Ok(Some(active.update(&self.db).await?))
    }

    /// Merge new ledger content (expected/measured/realized sections) into the
    /// opportunity's value ledger.
    pub async fn merge_evolve_opportunity_ledger(
        &self,
        id: &str,
        ledger_patch: serde_json::Value,
    ) -> Result<Option<evolve_opportunity::Model>> {
        let Some(existing) = evolve_opportunity::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let mut ledger = existing.ledger_json.clone();
        match (ledger.as_object_mut(), ledger_patch.as_object()) {
            (Some(current), Some(patch)) => {
                for (key, value) in patch {
                    current.insert(key.clone(), value.clone());
                }
            }
            _ => {
                ledger = ledger_patch;
            }
        }
        let now = chrono::Utc::now().to_rfc3339();
        let active = evolve_opportunity::ActiveModel {
            id: Unchanged(existing.id.clone()),
            ledger_json: Set(ledger),
            updated_at: Set(now),
            ..Default::default()
        };
        Ok(Some(active.update(&self.db).await?))
    }
}
