use super::*;

fn error_response(status: StatusCode, error: impl ToString) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
        .into_response()
}

pub(super) async fn list_custom_apis(State(state): State<AppState>) -> Response {
    let (storage, config_dir, data_dir) = {
        let agent = state.agent.read().await;
        (
            agent.storage.clone(),
            agent.config_dir.clone(),
            agent.data_dir.clone(),
        )
    };
    match crate::custom_apis::list_custom_apis(&storage, &config_dir, &data_dir).await {
        Ok(apis) => Json(serde_json::json!({
            "custom_apis": apis,
            "count": apis.len(),
        }))
        .into_response(),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

pub(super) async fn preview_custom_api(
    Json(request): Json<crate::custom_apis::CustomApiPreviewRequest>,
) -> Response {
    match crate::custom_apis::preview_custom_api(request).await {
        Ok(preview) => Json(serde_json::json!({
            "status": "ok",
            "preview": preview,
        }))
        .into_response(),
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}

pub(super) async fn create_custom_api(
    State(state): State<AppState>,
    Json(request): Json<crate::custom_apis::CustomApiUpsertRequest>,
) -> Response {
    let agent = state.agent.read().await;
    match crate::custom_apis::upsert_custom_api(
        &agent.storage,
        &agent.config_dir,
        &agent.data_dir,
        &agent.runtime,
        request,
        None,
    )
    .await
    {
        Ok(api) => {
            agent
                .refresh_action_catalog_index("custom_api_upsert")
                .await;
            Json(serde_json::json!({ "status": "ok", "custom_api": api })).into_response()
        }
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}

pub(super) async fn update_custom_api(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<crate::custom_apis::CustomApiUpsertRequest>,
) -> Response {
    let agent = state.agent.read().await;
    match crate::custom_apis::upsert_custom_api(
        &agent.storage,
        &agent.config_dir,
        &agent.data_dir,
        &agent.runtime,
        request,
        Some(id.as_str()),
    )
    .await
    {
        Ok(api) => {
            agent
                .refresh_action_catalog_index("custom_api_upsert")
                .await;
            Json(serde_json::json!({ "status": "ok", "custom_api": api })).into_response()
        }
        Err(error) if error.to_string().contains("not found") => {
            error_response(StatusCode::NOT_FOUND, error)
        }
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}

pub(super) async fn delete_custom_api(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let agent = state.agent.read().await;
    match crate::custom_apis::delete_custom_api(
        &agent.storage,
        &agent.config_dir,
        &agent.data_dir,
        &agent.runtime,
        id.as_str(),
    )
    .await
    {
        Ok(()) => {
            agent
                .refresh_action_catalog_index("custom_api_delete")
                .await;
            Json(serde_json::json!({ "status": "ok" })).into_response()
        }
        Err(error) if error.to_string().contains("not found") => {
            error_response(StatusCode::NOT_FOUND, error)
        }
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}

pub(super) async fn test_custom_api(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let agent = state.agent.read().await;
    match crate::custom_apis::test_custom_api(
        &agent.storage,
        &agent.config_dir,
        &agent.data_dir,
        &agent.runtime,
        id.as_str(),
    )
    .await
    {
        Ok(result) => Json(serde_json::json!({ "status": "ok", "result": result })).into_response(),
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}
