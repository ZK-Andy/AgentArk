use super::super::*;

const MAX_EVOLVE_EVAL_CASE_ROWS: u64 = 240;
#[allow(dead_code)]
const MAX_EVOLVE_LEDGER_ROWS: u64 = 400;

impl Storage {
    pub async fn upsert_evolve_eval_case(&self, case: &evolve_eval_case::Model) -> Result<()> {
        evolve_eval_case::Entity::insert(evolve_eval_case::ActiveModel {
            id: Set(case.id.clone()),
            opportunity_id: Set(case.opportunity_id.clone()),
            case_kind: Set(case.case_kind.clone()),
            source_kind: Set(case.source_kind.clone()),
            source_ref: Set(case.source_ref.clone()),
            source_run_ids_json: Set(case.source_run_ids_json.clone()),
            request_text: Set(case.request_text.clone()),
            contract_event_json: Set(case.contract_event_json.clone()),
            expected_behavior: Set(case.expected_behavior.clone()),
            disallowed_behavior: Set(case.disallowed_behavior.clone()),
            missing_info_policy: Set(case.missing_info_policy.clone()),
            secret_policy: Set(case.secret_policy.clone()),
            holdout: Set(case.holdout),
            status: Set(case.status.clone()),
            created_at: Set(case.created_at.clone()),
            updated_at: Set(case.updated_at.clone()),
        })
        .on_conflict(
            OnConflict::column(evolve_eval_case::Column::Id)
                .update_columns([
                    evolve_eval_case::Column::OpportunityId,
                    evolve_eval_case::Column::CaseKind,
                    evolve_eval_case::Column::SourceRunIdsJson,
                    evolve_eval_case::Column::RequestText,
                    evolve_eval_case::Column::ContractEventJson,
                    evolve_eval_case::Column::ExpectedBehavior,
                    evolve_eval_case::Column::DisallowedBehavior,
                    evolve_eval_case::Column::MissingInfoPolicy,
                    evolve_eval_case::Column::SecretPolicy,
                    evolve_eval_case::Column::Holdout,
                    evolve_eval_case::Column::Status,
                    evolve_eval_case::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    pub async fn upsert_evolve_eval_cases(
        &self,
        cases: &[evolve_eval_case::Model],
    ) -> Result<usize> {
        let mut written = 0usize;
        for case in cases.iter().take(MAX_EVOLVE_EVAL_CASE_ROWS as usize) {
            self.upsert_evolve_eval_case(case).await?;
            written += 1;
        }
        Ok(written)
    }

    #[allow(dead_code)]
    pub async fn list_evolve_eval_cases_for_opportunity(
        &self,
        opportunity_id: &str,
        limit: u64,
    ) -> Result<Vec<evolve_eval_case::Model>> {
        Ok(evolve_eval_case::Entity::find()
            .filter(evolve_eval_case::Column::OpportunityId.eq(opportunity_id.to_string()))
            .filter(evolve_eval_case::Column::Status.eq("active"))
            .order_by_desc(evolve_eval_case::Column::Holdout)
            .order_by_desc(evolve_eval_case::Column::UpdatedAt)
            .limit(Self::db_limit(limit.min(MAX_EVOLVE_EVAL_CASE_ROWS)))
            .all(&self.db)
            .await?)
    }

    pub async fn list_recent_evolve_eval_cases(
        &self,
        limit: u64,
    ) -> Result<Vec<evolve_eval_case::Model>> {
        Ok(evolve_eval_case::Entity::find()
            .filter(evolve_eval_case::Column::Status.eq("active"))
            .order_by_desc(evolve_eval_case::Column::UpdatedAt)
            .limit(Self::db_limit(limit.min(MAX_EVOLVE_EVAL_CASE_ROWS)))
            .all(&self.db)
            .await?)
    }

    pub async fn append_evolve_value_ledger_entry(
        &self,
        entry: &evolve_value_ledger::Model,
    ) -> Result<()> {
        evolve_value_ledger::Entity::insert(evolve_value_ledger::ActiveModel {
            id: Set(entry.id.clone()),
            opportunity_id: Set(entry.opportunity_id.clone()),
            phase: Set(entry.phase.clone()),
            value_json: Set(entry.value_json.clone()),
            source_ref: Set(entry.source_ref.clone()),
            created_at: Set(entry.created_at.clone()),
            updated_at: Set(entry.updated_at.clone()),
        })
        .on_conflict(
            OnConflict::column(evolve_value_ledger::Column::Id)
                .update_columns([
                    evolve_value_ledger::Column::ValueJson,
                    evolve_value_ledger::Column::SourceRef,
                    evolve_value_ledger::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn list_evolve_value_ledger_for_opportunity(
        &self,
        opportunity_id: &str,
        limit: u64,
    ) -> Result<Vec<evolve_value_ledger::Model>> {
        Ok(evolve_value_ledger::Entity::find()
            .filter(evolve_value_ledger::Column::OpportunityId.eq(opportunity_id.to_string()))
            .order_by_desc(evolve_value_ledger::Column::UpdatedAt)
            .limit(Self::db_limit(limit.min(MAX_EVOLVE_LEDGER_ROWS)))
            .all(&self.db)
            .await?)
    }
}
