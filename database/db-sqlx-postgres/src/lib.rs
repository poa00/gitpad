#![deny(missing_docs)]
//! # `libadmin` database operations implemented using sqlx postgres
//!
//! [`GistDatabase`](GistDatabase) is implemented on [Database].

use db_core::dev::*;

use sqlx::postgres::PgPoolOptions;
use sqlx::types::time::OffsetDateTime;
use sqlx::PgPool;

mod errors;
#[cfg(test)]
pub mod tests;

/// Database pool. All database functionallity(`libadmin` traits) are implemented on this
/// data structure
#[derive(Clone)]
pub struct Database {
    /// database pool
    pub pool: PgPool,
}

/// Use an existing database pool
pub struct Conn(pub PgPool);

/// Connect to databse
pub enum ConnectionOptions {
    /// fresh connection
    Fresh(Fresh),
    /// existing connection
    Existing(Conn),
}

/// Create a new database pool
pub struct Fresh {
    /// Pool options
    pub pool_options: PgPoolOptions,
    /// database URL
    pub url: String,
}

pub mod dev {
    //! useful imports for supporting a new database
    pub use super::errors::*;
    pub use super::Database;
    pub use db_core::dev::*;
    pub use prelude::*;
    pub use sqlx::Error;
}

pub mod prelude {
    //! useful imports for users working with a supported database
    pub use super::*;
    pub use db_core::prelude::*;
}
use dev::*;

#[async_trait]
impl Connect for ConnectionOptions {
    type Pool = Database;
    /// create connection pool
    async fn connect(self) -> DBResult<Self::Pool> {
        let pool = match self {
            Self::Fresh(fresh) => fresh
                .pool_options
                .connect(&fresh.url)
                .await
                .map_err(|e| DBError::DBError(Box::new(e)))?,
            Self::Existing(conn) => conn.0,
        };
        Ok(Database { pool })
    }
}

#[async_trait]
impl Migrate for Database {
    async fn migrate(&self) -> DBResult<()> {
        sqlx::migrate!("./migrations/")
            .run(&self.pool)
            .await
            .map_err(|e| DBError::DBError(Box::new(e)))?;
        Ok(())
    }
}

#[async_trait]
impl GistDatabase for Database {
    async fn email_login(&self, email: &str) -> DBResult<Creds> {
        sqlx::query_as!(
            Creds,
            r#"SELECT username, password  FROM gists_users WHERE email = ($1)"#,
            email,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::AccountNotFound,
            e => DBError::DBError(Box::new(e)),
        })
    }

    async fn username_login(&self, username: &str) -> DBResult<Password> {
        sqlx::query_as!(
            Password,
            r#"SELECT password  FROM gists_users WHERE username = ($1)"#,
            username,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::AccountNotFound,
            e => DBError::DBError(Box::new(e)),
        })
    }

    async fn email_register(&self, payload: &EmailRegisterPayload) -> DBResult<()> {
        sqlx::query!(
            "INSERT INTO gists_users 
        (username , password, email, secret) VALUES ($1, $2, $3, $4)",
            &payload.username,
            &payload.password,
            &payload.email,
            &payload.secret,
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        Ok(())
    }

    async fn username_register(&self, payload: &UsernameRegisterPayload) -> DBResult<()> {
        sqlx::query!(
            "INSERT INTO gists_users 
        (username , password,  secret) VALUES ($1, $2, $3)",
            &payload.username,
            &payload.password,
            &payload.secret,
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        Ok(())
    }

    async fn update_email(&self, payload: &UpdateEmailPayload) -> DBResult<()> {
        let x = sqlx::query!(
            "UPDATE gists_users set email = $1
        WHERE username = $2",
            &payload.email,
            &payload.username,
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        if x.rows_affected() == 0 {
            return Err(DBError::AccountNotFound);
        }

        Ok(())
    }
    async fn update_password(&self, payload: &Creds) -> DBResult<()> {
        let x = sqlx::query!(
            "UPDATE gists_users set password = $1
        WHERE username = $2",
            &payload.password,
            &payload.username,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DBError::DBError(Box::new(e)))?;
        if x.rows_affected() == 0 {
            return Err(DBError::AccountNotFound);
        }

        Ok(())
    }

    async fn email_exists(&self, email: &str) -> DBResult<bool> {
        let res = sqlx::query!(
            "SELECT EXISTS (SELECT 1 from gists_users WHERE email = $1)",
            &email
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DBError::DBError(Box::new(e)))?;

        let mut exists = false;
        if let Some(x) = res.exists {
            if x {
                exists = true;
            }
        };

        Ok(exists)
    }

    async fn delete_account(&self, username: &str) -> DBResult<()> {
        sqlx::query!("DELETE FROM gists_users WHERE username = ($1)", username,)
            .execute(&self.pool)
            .await
            .map_err(|e| DBError::DBError(Box::new(e)))?;
        Ok(())
    }

    async fn username_exists(&self, username: &str) -> DBResult<bool> {
        let res = sqlx::query!(
            "SELECT EXISTS (SELECT 1 from gists_users WHERE username = $1)",
            &username
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DBError::DBError(Box::new(e)))?;

        let mut exists = false;
        if let Some(x) = res.exists {
            if x {
                exists = true;
            }
        };

        Ok(exists)
    }

    async fn update_username(&self, payload: &UpdateUsernamePayload) -> DBResult<()> {
        let x = sqlx::query!(
            "UPDATE gists_users set username = $1 WHERE username = $2",
            &payload.new_username,
            &payload.old_username,
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        if x.rows_affected() == 0 {
            return Err(DBError::AccountNotFound);
        }
        Ok(())
    }
    async fn update_secret(&self, username: &str, secret: &str) -> DBResult<()> {
        let x = sqlx::query!(
            "UPDATE gists_users set secret = $1
        WHERE username = $2",
            secret,
            username,
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        if x.rows_affected() == 0 {
            return Err(DBError::AccountNotFound);
        }

        Ok(())
    }

    async fn get_secret(&self, username: &str) -> DBResult<String> {
        struct Secret {
            secret: String,
        }
        let secret = sqlx::query_as!(
            Secret,
            r#"SELECT secret  FROM gists_users WHERE username = ($1)"#,
            username,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::AccountNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;

        Ok(secret.secret)
    }

    /// ping DB
    async fn ping(&self) -> bool {
        use sqlx::Connection;

        if let Ok(mut con) = self.pool.acquire().await {
            con.ping().await.is_ok()
        } else {
            false
        }
    }

    /// Check if a Gist with the given ID exists
    async fn gist_exists(&self, public_id: &str) -> DBResult<bool> {
        let res = sqlx::query!(
            "SELECT EXISTS (SELECT 1 from gists_gists WHERE public_id = $1)",
            public_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DBError::DBError(Box::new(e)))?;

        let mut exists = false;
        if let Some(x) = res.exists {
            exists = x;
        };

        Ok(exists)
    }
    /// Create new gists
    async fn new_gist(&self, gist: &CreateGist) -> DBResult<()> {
        let now = OffsetDateTime::now_utc();
        if let Some(description) = &gist.description {
            sqlx::query!(
                "INSERT INTO gists_gists 
        (owner_id , description, public_id, visibility, created, updated)
        VALUES (
            (SELECT ID FROM gists_users WHERE username = $1),
            $2, $3, (SELECT ID FROM gists_visibility WHERE name = $4), $5, $6
        )",
                &gist.owner,
                description,
                &gist.public_id,
                gist.visibility.to_str(),
                &now,
                &now
            )
            .execute(&self.pool)
            .await
            .map_err(map_register_err)?;
        } else {
            sqlx::query!(
                "INSERT INTO gists_gists 
        (owner_id , public_id, visibility, created, updated)
        VALUES (
            (SELECT ID FROM gists_users WHERE username = $1),
            $2, (SELECT ID FROM gists_visibility WHERE name = $3), $4, $5
        )",
                &gist.owner,
                &gist.public_id,
                gist.visibility.to_str(),
                &now,
                &now
            )
            .execute(&self.pool)
            .await
            .map_err(map_register_err)?;
        }
        Ok(())
    }
    /// Retrieve gist from database
    async fn get_gist(&self, public_id: &str) -> DBResult<Gist> {
        let res = sqlx::query_as!(
            InnerGist,
            "SELECT
                owner,
                visibility,
                created,
                updated,
                public_id,
                description
            FROM
                gists_gists_view
            WHERE public_id = $1
            ",
            public_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::GistNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;
        res.to_gist()
    }

    /// Retrieve gists belonging to user from database
    async fn get_user_gists(&self, owner: &str) -> DBResult<Vec<Gist>> {
        let mut res = sqlx::query_as!(
            InnerGist,
            "SELECT
                owner,
                visibility,
                created,
                updated,
                public_id,
                description
            FROM
                gists_gists_view
            WHERE owner = $1
            ",
            owner
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::GistNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;

        let mut gists = Vec::with_capacity(res.len());
        for r in res.drain(..) {
            gists.push(r.to_gist()?);
        }
        Ok(gists)
    }

    /// Retrieve gists belonging to user from database
    async fn get_user_public_gists(&self, owner: &str) -> DBResult<Vec<Gist>> {
        const PUBLIC: &str = GistVisibility::Public.to_str();
        let mut res = sqlx::query_as!(
            InnerGist,
            "SELECT
                owner,
                visibility,
                created,
                updated,
                public_id,
                description
            FROM
                gists_gists_view
            WHERE 
                owner = $1
            AND
                visibility = $2
            ",
            owner,
            PUBLIC
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::GistNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;

        let mut gists = Vec::with_capacity(res.len());
        for r in res.drain(..) {
            gists.push(r.to_gist()?);
        }
        Ok(gists)
    }

    /// Retrieve gists belonging to user from database
    async fn get_user_public_unlisted_gists(&self, owner: &str) -> DBResult<Vec<Gist>> {
        const PRIVATE: &str = GistVisibility::Private.to_str();
        let mut res = sqlx::query_as!(
            InnerGist,
            "SELECT
                owner,
                visibility,
                created,
                updated,
                public_id,
                description
            FROM
                gists_gists_view
            WHERE 
                owner = $1
            AND
                visibility <> $2
            ",
            owner,
            PRIVATE
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::GistNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;

        let mut gists = Vec::with_capacity(res.len());
        for r in res.drain(..) {
            gists.push(r.to_gist()?);
        }
        Ok(gists)
    }

    async fn delete_gist(&self, owner: &str, public_id: &str) -> DBResult<()> {
        sqlx::query!(
            "DELETE FROM gists_gists 
        WHERE 
            public_id = $1
        AND
            owner_id = (SELECT ID FROM gists_users WHERE username = $2)
        ",
            public_id,
            owner
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        Ok(())
    }

    /// Create new comment
    async fn new_comment(&self, comment: &CreateGistComment) -> DBResult<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query!(
            "INSERT INTO gists_comments (owner_id, gist_id, comment, created)
            VALUES (
                (SELECT ID FROM gists_users WHERE username = $1),
                (SELECT ID FROM gists_gists WHERE public_id = $2),
                $3,
                $4
            )",
            comment.owner,
            comment.gist_public_id,
            comment.comment,
            &now,
        )
        .execute(&self.pool)
        .await
        .map_err(map_register_err)?;
        Ok(())
    }
    /// Get comments on a gist
    async fn get_comments_on_gist(&self, public_id: &str) -> DBResult<Vec<GistComment>> {
        let mut res = sqlx::query_as!(
            InnerGistComment,
            "
            SELECT
                ID,
                comment,
                owner,
                created,
                gist_public_id
            FROM
                gists_comments_view
            WHERE
                gist_public_id = $1
            ORDER BY created;
            ",
            public_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::CommentNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;

        let mut comments: Vec<GistComment> = Vec::with_capacity(res.len());
        res.drain(..).for_each(|r| comments.push(r.into()));
        Ok(comments)
    }
    /// Get a specific comment using its database assigned ID
    async fn get_comment_by_id(&self, id: i64) -> DBResult<GistComment> {
        let res = sqlx::query_as!(
            InnerGistComment,
            "
            SELECT
                ID,
                comment,
                owner,
                created,
                gist_public_id
            FROM
                gists_comments_view
            WHERE
                ID = $1
            ",
            id as i32
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            Error::RowNotFound => DBError::CommentNotFound,
            e => DBError::DBError(Box::new(e)),
        })?;

        Ok(res.into())
    }

    /// Delete comment
    async fn delete_comment(&self, owner: &str, id: i64) -> DBResult<()> {
        sqlx::query!(
            "DELETE FROM gists_comments
                    WHERE
                        ID = $1
                    AND
                        owner_id = (SELECT ID FROM gists_users WHERE username = $2)
                    ",
            id as i32,
            owner,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DBError::DBError(Box::new(e)))?;
        Ok(())
    }

    async fn visibility_exists(&self, visibility: &GistVisibility) -> DBResult<bool> {
        let res = sqlx::query!(
            "SELECT EXISTS (SELECT 1 from gists_visibility WHERE name = $1)",
            visibility.to_str()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DBError::DBError(Box::new(e)))?;

        let mut exists = false;
        if let Some(x) = res.exists {
            exists = x
        };

        Ok(exists)
    }
}
struct InnerGist {
    owner: Option<String>,
    description: Option<String>,
    public_id: Option<String>,
    created: Option<OffsetDateTime>,
    updated: Option<OffsetDateTime>,
    visibility: Option<String>,
}

impl InnerGist {
    fn to_gist(self) -> DBResult<Gist> {
        Ok(Gist {
            owner: self.owner.unwrap(),
            description: self.description,
            public_id: self.public_id.unwrap(),
            created: self.created.as_ref().unwrap().unix_timestamp(),
            updated: self.updated.as_ref().unwrap().unix_timestamp(),
            visibility: GistVisibility::from_str(self.visibility.as_ref().unwrap())?,
        })
    }
}

struct InnerGistComment {
    id: Option<i32>,
    owner: Option<String>,
    comment: Option<String>,
    gist_public_id: Option<String>,
    created: Option<OffsetDateTime>,
}

impl From<InnerGistComment> for GistComment {
    fn from(g: InnerGistComment) -> Self {
        Self {
            id: g.id.unwrap() as i64,
            owner: g.owner.unwrap(),
            comment: g.comment.unwrap(),
            gist_public_id: g.gist_public_id.unwrap(),
            created: g.created.unwrap().unix_timestamp(),
        }
    }
}