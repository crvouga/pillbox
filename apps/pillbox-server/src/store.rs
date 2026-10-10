use crate::{ApiError, ApiResult, User};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    pub pool: PgPool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Version {
    pub id: Uuid,
    pub content_id: Uuid,
    pub owner_id: Uuid,
    pub version: i32,
    pub published: bool,
    pub document: Value,
}
impl Store {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .after_connect(|c, _| {
                Box::pin(async move {
                    sqlx::query("SET statement_timeout = '10s'")
                        .execute(c)
                        .await?;
                    Ok(())
                })
            })
            .connect(url)
            .await?;
        sqlx::query("SELECT id FROM users LIMIT 0")
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }
    pub async fn guest(&self, name: String) -> ApiResult<(User, String)> {
        let user = User {
            id: Uuid::new_v4(),
            name,
        };
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let hash = hash_token(&token);
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO users(id,name) VALUES($1,$2)")
            .bind(user.id)
            .bind(&user.name)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '30 days')").bind(hash).bind(user.id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok((user, token))
    }
    pub async fn authenticate(&self, token: &str) -> ApiResult<User> {
        if token.len() != 64 {
            return Err(ApiError::unauthorized());
        }
        let row=sqlx::query("SELECT u.id,u.name FROM users u JOIN sessions s ON u.id=s.user_id WHERE s.token_hash=$1 AND s.expires_at>now()").bind(hash_token(token)).fetch_optional(&self.pool).await?.ok_or_else(ApiError::unauthorized)?;
        Ok(User {
            id: row.get("id"),
            name: row.get("name"),
        })
    }
    pub async fn list(&self, kind: &str, owner: Uuid) -> ApiResult<Vec<Version>> {
        let (table, versions, key) = tables(kind)?;
        let q = format!(
            "SELECT v.id,v.{key} AS content_id,c.owner_id,v.version,v.document,v.published FROM {versions} v JOIN {table} c ON c.id=v.{key} WHERE v.published OR c.owner_id=$1 ORDER BY v.created_at DESC LIMIT 200"
        );
        let rows = sqlx::query(&q).bind(owner).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(version).collect())
    }
    pub async fn get(&self, kind: &str, id: Uuid, owner: Uuid) -> ApiResult<Version> {
        let (table, versions, key) = tables(kind)?;
        let q = format!(
            "SELECT v.id,v.{key} AS content_id,c.owner_id,v.version,v.document,v.published FROM {versions} v JOIN {table} c ON c.id=v.{key} WHERE v.id=$1 AND (v.published OR c.owner_id=$2)"
        );
        let row = sqlx::query(&q)
            .bind(id)
            .bind(owner)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Version not found"))?;
        Ok(version(&row))
    }
    pub async fn save(
        &self,
        kind: &str,
        owner: Uuid,
        content_id: Option<Uuid>,
        document: Value,
    ) -> ApiResult<Version> {
        let (table, versions, key) = tables(kind)?;
        let mut tx = self.pool.begin().await?;
        let content_id = if let Some(id) = content_id {
            let q = format!("SELECT owner_id FROM {table} WHERE id=$1 FOR UPDATE");
            let row = sqlx::query(&q)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| ApiError::not_found("Content not found"))?;
            if row.get::<Uuid, _>("owner_id") != owner {
                return Err(ApiError::forbidden());
            }
            id
        } else {
            let id = Uuid::new_v4();
            let q = format!("INSERT INTO {table}(id,owner_id) VALUES($1,$2)");
            sqlx::query(&q)
                .bind(id)
                .bind(owner)
                .execute(&mut *tx)
                .await?;
            id
        };
        let q = format!("SELECT COALESCE(MAX(version),0)+1 AS next FROM {versions} WHERE {key}=$1");
        let n: i32 = sqlx::query(&q)
            .bind(content_id)
            .fetch_one(&mut *tx)
            .await?
            .get("next");
        let id = Uuid::new_v4();
        let q = format!("INSERT INTO {versions}(id,{key},version,document) VALUES($1,$2,$3,$4)");
        sqlx::query(&q)
            .bind(id)
            .bind(content_id)
            .bind(n)
            .bind(&document)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Version {
            id,
            content_id,
            owner_id: owner,
            version: n,
            published: false,
            document,
        })
    }
    pub async fn publish(&self, kind: &str, id: Uuid, owner: Uuid) -> ApiResult<()> {
        let (table, versions, key) = tables(kind)?;
        let q = format!(
            "UPDATE {versions} v SET published=true FROM {table} c WHERE v.id=$1 AND c.id=v.{key} AND c.owner_id=$2"
        );
        if sqlx::query(&q)
            .bind(id)
            .bind(owner)
            .execute(&self.pool)
            .await?
            .rows_affected()
            != 1
        {
            return Err(ApiError::forbidden());
        }
        Ok(())
    }
    pub async fn result(&self, id: Uuid) -> ApiResult<Value> {
        let r = sqlx::query("SELECT result FROM match_results WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Result not found or still saving"))?;
        Ok(r.get("result"))
    }
    pub async fn persist(&self, job: &crate::ResultJob) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO match_results(id,server_id,map_version_id,mode_version_id,result) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO NOTHING").bind(job.id).bind(job.server).bind(job.map).bind(job.mode).bind(&job.result).execute(&mut *tx).await?;
        // Aggregate repeat participation (disconnect/rejoin) into one durable player row.
        let mut players = std::collections::BTreeMap::new();
        for p in &job.players {
            let e = players
                .entry(p.id.clone())
                .or_insert((0_i32, 0_u32, 0_u32, p.team));
            e.0 = e.0.saturating_add(p.score);
            e.1 += p.kills;
            e.2 += p.deaths;
        }
        for (id, (score, kills, deaths, team)) in players {
            if let Ok(user) = Uuid::parse_str(&id) {
                sqlx::query("INSERT INTO match_players(match_id,user_id,score,kills,deaths,team) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(match_id,user_id) DO NOTHING").bind(job.id).bind(user).bind(score).bind(kills as i32).bind(deaths as i32).bind(i32::from(team)).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
fn tables(kind: &str) -> ApiResult<(&'static str, &'static str, &'static str)> {
    match kind {
        "maps" => Ok(("maps", "map_versions", "map_id")),
        "modes" => Ok(("modes", "mode_versions", "mode_id")),
        _ => Err(ApiError::not_found("Unknown content type")),
    }
}
fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn version(r: &sqlx::postgres::PgRow) -> Version {
    Version {
        id: r.get("id"),
        content_id: r.get("content_id"),
        owner_id: r.get("owner_id"),
        version: r.get("version"),
        published: r.get("published"),
        document: r.get("document"),
    }
}
