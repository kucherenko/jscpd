//! Postgres access.

use sqlx::PgPool;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct Article {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub body: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn connect(url: &str, pool_size: u32) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(pool_size)
            .connect(url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn articles_page(&self, page: u32, per_page: u32) -> Result<(Vec<Article>, u32), sqlx::Error> {
        let offset = i64::from((page - 1) * per_page);
        let articles = sqlx::query_as::<_, Article>(
            "SELECT id, slug, title, body, published_at FROM articles
             ORDER BY published_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(i64::from(per_page))
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM articles")
            .fetch_one(&self.pool)
            .await?;
        Ok((articles, total as u32))
    }

    pub async fn article_by_slug(&self, slug: &str) -> Result<Option<Article>, sqlx::Error> {
        sqlx::query_as::<_, Article>(
            "SELECT id, slug, title, body, published_at FROM articles WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
    }
}
