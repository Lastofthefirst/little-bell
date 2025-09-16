use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub id: i64,
    pub tenant_id: String,
    pub subject: Option<String>,
    pub recipient: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub email_id: i64,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub total_opens: i64,
    pub total_clicks: i64,
    pub unique_opens: i64,
    pub unique_clicks: i64,
    pub recent_events: Vec<Event>,
}

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub async fn new(db_path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let database = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        database.initialize().await?;
        Ok(database)
    }

    async fn initialize(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        
        // Create tenants table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tenants (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            params![],
        )?;

        // Create emails table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS emails (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id TEXT NOT NULL,
                subject TEXT,
                recipient TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (tenant_id) REFERENCES tenants (id)
            )",
            params![],
        )?;

        // Create events table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                user_agent TEXT,
                ip_address TEXT,
                FOREIGN KEY (email_id) REFERENCES emails (id)
            )",
            params![],
        )?;

        // Create indexes for better performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_email_id ON events(email_id)",
            params![],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type)",
            params![],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_emails_tenant ON emails(tenant_id)",
            params![],
        )?;

        Ok(())
    }

    pub async fn create_tenant(&self, tenant_id: &str, name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        
        conn.execute(
            "INSERT OR IGNORE INTO tenants (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![tenant_id, name, now.to_rfc3339()],
        )?;
        Ok(())
    }

    pub async fn get_tenant(&self, tenant_id: &str) -> SqliteResult<Option<Tenant>> {
        let conn = self.conn.lock().await;
        
        let mut stmt = conn.prepare("SELECT id, name, created_at FROM tenants WHERE id = ?1")?;
        let mut tenant_iter = stmt.query_map(params![tenant_id], |row| {
            Ok(Tenant {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;

        if let Some(tenant) = tenant_iter.next() {
            return Ok(Some(tenant?));
        }
        Ok(None)
    }

    pub async fn create_email(&self, tenant_id: &str, subject: Option<&str>, recipient: Option<&str>) -> SqliteResult<i64> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        
        conn.execute(
            "INSERT INTO emails (tenant_id, subject, recipient, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![tenant_id, subject, recipient, now.to_rfc3339()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub async fn get_email(&self, email_id: i64, tenant_id: &str) -> SqliteResult<Option<Email>> {
        let conn = self.conn.lock().await;
        
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, subject, recipient, created_at FROM emails WHERE id = ?1 AND tenant_id = ?2"
        )?;
        let mut email_iter = stmt.query_map(params![email_id, tenant_id], |row| {
            Ok(Email {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                subject: row.get(2)?,
                recipient: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;

        if let Some(email) = email_iter.next() {
            return Ok(Some(email?));
        }
        Ok(None)
    }

    pub async fn log_event(
        &self,
        email_id: i64,
        event_type: &str,
        user_agent: Option<&str>,
        ip_address: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        
        conn.execute(
            "INSERT INTO events (email_id, event_type, timestamp, user_agent, ip_address) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![email_id, event_type, now.to_rfc3339(), user_agent, ip_address],
        )?;
        Ok(())
    }

    pub async fn get_tenant_stats(&self, tenant_id: &str) -> SqliteResult<EventStats> {
        let conn = self.conn.lock().await;
        
        // Get total opens and clicks
        let mut stmt = conn.prepare(
            "SELECT 
                COUNT(CASE WHEN e.event_type = 'open' THEN 1 END) as total_opens,
                COUNT(CASE WHEN e.event_type = 'click' THEN 1 END) as total_clicks,
                COUNT(DISTINCT CASE WHEN e.event_type = 'open' THEN e.email_id END) as unique_opens,
                COUNT(DISTINCT CASE WHEN e.event_type = 'click' THEN e.email_id END) as unique_clicks
             FROM events e 
             JOIN emails em ON e.email_id = em.id 
             WHERE em.tenant_id = ?1"
        )?;
        
        let stats = stmt.query_row(params![tenant_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;

        // Get recent events
        let mut stmt = conn.prepare(
            "SELECT e.id, e.email_id, e.event_type, e.timestamp, e.user_agent, e.ip_address
             FROM events e 
             JOIN emails em ON e.email_id = em.id 
             WHERE em.tenant_id = ?1 
             ORDER BY e.timestamp DESC 
             LIMIT 50"
        )?;
        
        let event_iter = stmt.query_map(params![tenant_id], |row| {
            Ok(Event {
                id: row.get(0)?,
                email_id: row.get(1)?,
                event_type: row.get(2)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
                user_agent: row.get(4)?,
                ip_address: row.get(5)?,
            })
        })?;

        let mut recent_events = Vec::new();
        for event in event_iter {
            recent_events.push(event?);
        }

        Ok(EventStats {
            total_opens: stats.0,
            total_clicks: stats.1,
            unique_opens: stats.2,
            unique_clicks: stats.3,
            recent_events,
        })
    }
}