//! HTTP routes. Handlers parse the request, call the domain code and map
//! its errors; nothing here has a counterpart in the frontend.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::db::Db;
use crate::error::ApiError;
use crate::pagination::page_links;
use crate::pricing::{CartLine, Coupon, cart_totals};

#[derive(Deserialize)]
pub struct ListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Deserialize)]
pub struct QuoteRequest {
    pub lines: Vec<CartLine>,
    pub coupon: Option<Coupon>,
}

pub fn router(db: Db) -> Router {
    Router::new()
        .route("/api/articles", get(list_articles))
        .route("/api/articles/{slug}", get(show_article))
        .route("/api/cart/quote", post(quote))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(db)
}

async fn list_articles(
    State(db): State<Db>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let page = params.page.unwrap_or(1).max(1);
    let (articles, total) = db.articles_page(page, per_page).await?;
    let total_pages = total.div_ceil(per_page);
    Ok(Json(serde_json::json!({
        "articles": articles,
        "page": page,
        "links": page_links(page, total_pages, 2),
    })))
}

async fn show_article(
    State(db): State<Db>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let article = db
        .article_by_slug(&slug)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("no article {slug}")))?;
    Ok(Json(serde_json::json!({ "article": article })))
}

async fn quote(Json(request): Json<QuoteRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    if request.lines.iter().any(|line| line.quantity == 0) {
        return Err(ApiError::BadRequest("quantity must be positive".into()));
    }
    let totals = cart_totals(&request.lines, request.coupon.as_ref());
    Ok(Json(serde_json::json!({ "totals": totals })))
}
