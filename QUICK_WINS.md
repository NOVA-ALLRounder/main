# Steer OS Agent - Quick Wins (Immediate Improvements)

This document outlines the highest-impact, lowest-effort improvements you can implement immediately.

---

## 🚀 Immediate Actions (Can Do Today)

### 1. Fix Database Performance Bottleneck (30 minutes)

**Impact:** 10x throughput improvement
**Difficulty:** Easy
**File:** `core/Cargo.toml`

Add dependency:
```toml
[dependencies]
r2d2 = "0.8"
r2d2_sqlite = "0.24"
```

Replace in `core/src/db.rs`:
```rust
use r2d2_sqlite::SqliteConnectionManager;
use r2d2::Pool;

lazy_static! {
    static ref DB_POOL: Pool<SqliteConnectionManager> = {
        let db_path = get_db_path();
        let manager = SqliteConnectionManager::file(db_path)
            .with_init(|conn| {
                conn.busy_timeout(Duration::from_secs(5))?;
                conn.execute("PRAGMA journal_mode=WAL", [])?;
                Ok(())
            });
        Pool::builder()
            .max_size(10)
            .build(manager)
            .expect("Failed to create pool")
    };
}

pub fn get_connection() -> Result<PooledConnection<SqliteConnectionManager>> {
    DB_POOL.get().map_err(|e| anyhow!("Failed to get connection: {}", e))
}
```

---

### 2. Replace Unmaintained Dependencies (10 minutes)

**Impact:** Security & maintenance
**Difficulty:** Trivial

In `core/Cargo.toml`:
```toml
# OLD
dotenv = "0.15"

# NEW
dotenvy = "0.15"
```

Replace all occurrences in code:
```bash
cd core
find src -name "*.rs" -exec sed -i 's/dotenv::/dotenvy::/g' {} \;
```

---

### 3. Fix Chrono Version Pin (2 minutes)

**Impact:** Security (CVE fixes)
**Difficulty:** Trivial

In `core/Cargo.toml`:
```toml
# OLD
chrono = { version = "=0.4.38", features = ["serde"] }

# NEW
chrono = { version = "0.4", features = ["serde"] }
```

---

### 4. Add Structured Logging (20 minutes)

**Impact:** Debugging & monitoring
**Difficulty:** Easy

Add to `core/Cargo.toml`:
```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

Replace in `core/src/main.rs`:
```rust
use tracing::{info, warn, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("steer=debug,warn")
        .with_target(false)
        .json()
        .init();

    info!("🤖 Steer OS Agent starting...");

    // Rest of main logic...
}
```

Replace all `println!` with `info!`, `eprintln!` with `error!`.

---

### 5. Add Basic Tests (45 minutes)

**Impact:** Catch regressions early
**Difficulty:** Medium

Create `core/src/policy_test.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_lock_blocks_caution_actions() {
        let policy = PolicyEngine::new();
        let action = AgentAction::UiClick {
            element_id: "test".to_string(),
        };
        assert!(policy.check(&action).is_err());
    }

    #[test]
    fn safe_actions_always_allowed() {
        let policy = PolicyEngine::new();
        let action = AgentAction::UiSnapshot { scope: None };
        assert!(policy.check(&action).is_ok());
    }

    #[test]
    fn unlock_allows_caution_actions() {
        let mut policy = PolicyEngine::new();
        policy.unlock();
        let action = AgentAction::UiClick {
            element_id: "test".to_string(),
        };
        assert!(policy.check(&action).is_ok());
    }
}
```

Run tests:
```bash
cd core
cargo test
```

---

## 📊 Week 1 Improvements

### 6. Reorganize Module Structure (2 hours)

**Impact:** Maintainability
**Difficulty:** Medium

Create new directory structure:
```bash
cd core/src
mkdir -p domain/models domain/services
mkdir -p infrastructure/persistence infrastructure/llm
mkdir -p application/commands application/queries
```

Move files:
```bash
# Domain models
mv schema.rs domain/models/
mv recommendation.rs domain/models/

# Infrastructure
mv db.rs infrastructure/persistence/
mv llm_gateway.rs infrastructure/llm/

# Update lib.rs with new module hierarchy
```

---

### 7. Add API Rate Limiting (1 hour)

**Impact:** Security & stability
**Difficulty:** Easy

Add to `core/Cargo.toml`:
```toml
[dependencies]
tower-governor = "0.1"
```

In `api_server.rs`:
```rust
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

pub fn create_app(state: AppState) -> Router {
    let governor_conf = Box::new(
        GovernorConfigBuilder::default()
            .per_second(10)  // 10 requests per second
            .burst_size(20)  // Allow bursts up to 20
            .finish()
            .unwrap()
    );

    Router::new()
        .route("/api/chat", post(chat_handler))
        .layer(GovernorLayer { config: governor_conf })
        .with_state(state)
}
```

---

### 8. Environment Variable Validation (30 minutes)

**Impact:** Better error messages
**Difficulty:** Easy

Create `core/src/config.rs`:
```rust
use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub openai_api_key: String,
    pub n8n_api_url: String,
    pub n8n_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let openai_api_key = env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY not set. Please add it to .env file")?;

        if !openai_api_key.starts_with("sk-") {
            anyhow::bail!("Invalid OPENAI_API_KEY format. Must start with 'sk-'");
        }

        let n8n_api_url = env::var("N8N_API_URL")
            .unwrap_or_else(|_| "http://localhost:5678/api/v1".to_string());

        Ok(Self {
            openai_api_key,
            n8n_api_url,
            n8n_api_key: env::var("N8N_API_KEY").ok(),
        })
    }
}
```

---

## 🛡️ Security Hardening (Week 2)

### 9. Add API Authentication (2 hours)

**Impact:** Security
**Difficulty:** Medium

Add to `Cargo.toml`:
```toml
[dependencies]
jsonwebtoken = "9"
axum-extra = { version = "0.9", features = ["cookie"] }
```

Create `core/src/auth.rs`:
```rust
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};

pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "default-secret-change-in-production".to_string());

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(next.run(request).await)
}
```

---

### 10. Audit Log Enhancement (1 hour)

Add to `db.rs`:
```rust
pub fn log_security_event(
    action: &str,
    user: &str,
    result: &str,
    risk_level: &str,
) -> Result<()> {
    let conn = get_connection()?;
    conn.execute(
        "INSERT INTO security_audit (timestamp, action, user, result, risk_level)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            chrono::Utc::now().to_rfc3339(),
            action,
            user,
            result,
            risk_level
        ],
    )?;
    Ok(())
}
```

---

## 📈 Measuring Impact

After implementing quick wins, measure:

```bash
# Performance test
cd core
cargo build --release
time ./target/release/core test-command

# Database performance
sqlite3 ~/.local/share/steer/steer.db "PRAGMA journal_mode;"
# Should output: WAL

# Test coverage
cargo tarpaulin --out Html
# Open tarpaulin-report.html

# Check for outdated dependencies
cargo outdated
```

---

## Priority Order

1. **Database Connection Pool** ← Do this first! Biggest impact
2. **Fix Chrono Version** ← Security fix
3. **Add Structured Logging** ← Makes debugging 10x easier
4. **Replace dotenv** ← Unmaintained dependency
5. **Add Basic Tests** ← Prevent regressions
6. **Environment Variable Validation** ← Better UX
7. **API Rate Limiting** ← Prevent abuse
8. **API Authentication** ← Security
9. **Module Reorganization** ← Long-term maintainability
10. **Audit Logging** ← Compliance

---

## Validation Checklist

After each change:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` shows no warnings
- [ ] Application starts successfully
- [ ] Basic functionality works (test with UI)

---

## Need Help?

For each improvement, refer to:
- Full details in `ARCHITECTURE_REVIEW.md`
- Code examples in relevant sections
- Official crate documentation on docs.rs
