//! Single-file configuration for ClinicClaw.
//!
//! Resolution order:
//! 1. TOML file (path from `CLINICLAW_CONFIG` env var, or `./cliniclaw.toml`)
//! 2. Individual environment variables (backward compatibility)
//! 3. Compiled defaults
//!
//! Hospital IT teams review one TOML file before deployment.
//! Existing env-var deployments keep working — env vars override TOML values.

use std::path::{Path, PathBuf};

/// Top-level ClinicClaw configuration.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct ClinicLawConfig {
    pub server: ServerConfig,
    pub llm: LlmConfig,
    pub fhir: FhirConfig,
    pub database: DatabaseConfig,
    pub policy: PolicyConfig,
    pub cors: CorsConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// LLM backend: "mock", "ollama", or "claude"
    pub backend: String,
    /// Ollama model name (when backend = "ollama")
    pub ollama_model: String,
    /// Ollama base URL
    pub ollama_url: String,
    /// Claude model ID (when backend = "claude")
    pub claude_model: String,
    /// Max concurrent LLM calls (for pipeline rate limiting)
    pub max_concurrent_calls: usize,
    /// Max tokens per encounter (for pipeline token budget)
    pub max_tokens_per_encounter: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct FhirConfig {
    /// Synthea FHIR bundle directory (when using mock backend)
    pub synthea_dir: Option<String>,
    /// Medplum or FHIR server base URL (when using live backend)
    pub base_url: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    /// SQLite or Postgres connection URL
    pub url: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    /// Directory containing .rego and .toml policy files
    pub policies_dir: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct CorsConfig {
    /// Allowed CORS origins
    pub origins: Vec<String>,
}

// ── Defaults ─────────────────────────────────────────────────────

impl Default for ClinicLawConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            llm: LlmConfig::default(),
            fhir: FhirConfig::default(),
            database: DatabaseConfig::default(),
            policy: PolicyConfig::default(),
            cors: CorsConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:3001".to_string(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            backend: "mock".to_string(),
            ollama_model: "mistral-small".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            claude_model: "claude-sonnet-4-20250514".to_string(),
            max_concurrent_calls: 4,
            max_tokens_per_encounter: 50_000,
        }
    }
}

impl Default for FhirConfig {
    fn default() -> Self {
        Self {
            synthea_dir: None,
            base_url: "http://localhost:8103/fhir/R4".to_string(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:cliniclaw.sqlite".to_string(),
        }
    }
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            policies_dir: "crates/cliniclaw-policy/policies".to_string(),
        }
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            origins: vec!["http://localhost:3000".to_string()],
        }
    }
}

// ── Loading ──────────────────────────────────────────────────────

impl ClinicLawConfig {
    /// Load configuration with resolution order:
    /// 1. TOML file (if it exists)
    /// 2. Environment variable overrides
    /// 3. Compiled defaults
    pub fn load() -> Result<Self, String> {
        // Step 1: Try TOML file
        let config_path = std::env::var("CLINICLAW_CONFIG")
            .unwrap_or_else(|_| "cliniclaw.toml".to_string());
        let path = Path::new(&config_path);

        let mut config = if path.exists() {
            let contents = std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
            tracing::info!(path = %path.display(), "loaded configuration from TOML");
            toml::from_str::<ClinicLawConfig>(&contents)
                .map_err(|e| format!("failed to parse {}: {e}", path.display()))?
        } else {
            tracing::debug!(path = %path.display(), "no TOML config found, using defaults + env vars");
            ClinicLawConfig::default()
        };

        // Step 2: Environment variable overrides (backward compatibility)
        config.apply_env_overrides();

        // Step 3: Validate
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variable overrides on top of TOML/default values.
    /// Env vars take precedence — existing deployments keep working.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("LISTEN_ADDR") {
            self.server.listen_addr = v;
        }

        // LLM backend: check LLM_BACKEND first, then legacy CLINICLAW_MOCK
        if let Ok(v) = std::env::var("LLM_BACKEND") {
            self.llm.backend = v;
        } else if std::env::var("CLINICLAW_MOCK")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
        {
            self.llm.backend = "mock".to_string();
        // Note: we do NOT auto-detect Claude from CLAUDE_API_KEY presence.
        // Paid backends require explicit opt-in via LLM_BACKEND=claude or config.
        }

        if let Ok(v) = std::env::var("OLLAMA_MODEL") {
            self.llm.ollama_model = v;
        }
        if let Ok(v) = std::env::var("OLLAMA_URL") {
            self.llm.ollama_url = v;
        }
        if let Ok(v) = std::env::var("CLAUDE_MODEL") {
            self.llm.claude_model = v;
        }

        if let Ok(v) = std::env::var("SYNTHEA_DIR") {
            self.fhir.synthea_dir = Some(v);
        }
        if let Ok(v) = std::env::var("FHIR_BASE_URL") {
            self.fhir.base_url = v;
        }

        if let Ok(v) = std::env::var("DATABASE_URL") {
            self.database.url = v;
        }
    }

    /// Validate configuration at startup. Fail fast on invalid config.
    fn validate(&self) -> Result<(), String> {
        if self.server.listen_addr.is_empty() {
            return Err("server.listen_addr must not be empty".into());
        }

        let valid_backends = ["mock", "ollama", "claude"];
        if !valid_backends.contains(&self.llm.backend.as_str()) {
            return Err(format!(
                "llm.backend must be one of {:?}, got '{}'",
                valid_backends, self.llm.backend
            ));
        }

        if self.llm.backend == "claude" && std::env::var("CLAUDE_API_KEY").is_err() {
            return Err("llm.backend=claude requires CLAUDE_API_KEY environment variable".into());
        }

        if self.llm.max_concurrent_calls == 0 {
            return Err("llm.max_concurrent_calls must be > 0".into());
        }

        Ok(())
    }

    /// Get the Synthea directory as a PathBuf, if configured.
    pub fn synthea_path(&self) -> Option<PathBuf> {
        self.fhir.synthea_dir.as_ref().map(PathBuf::from)
    }

    /// Check if running in mock mode.
    pub fn is_mock(&self) -> bool {
        self.llm.backend == "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let config = ClinicLawConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn invalid_backend_rejected() {
        let mut config = ClinicLawConfig::default();
        config.llm.backend = "gpt4".to_string();
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("llm.backend"));
    }

    #[test]
    fn empty_listen_addr_rejected() {
        let mut config = ClinicLawConfig::default();
        config.server.listen_addr = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_concurrent_calls_rejected() {
        let mut config = ClinicLawConfig::default();
        config.llm.max_concurrent_calls = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn is_mock_detects_backend() {
        let mut config = ClinicLawConfig::default();
        assert!(config.is_mock());
        config.llm.backend = "ollama".to_string();
        assert!(!config.is_mock());
    }

    #[test]
    fn synthea_path_none_by_default() {
        let config = ClinicLawConfig::default();
        assert!(config.synthea_path().is_none());
    }

    #[test]
    fn synthea_path_from_config() {
        let mut config = ClinicLawConfig::default();
        config.fhir.synthea_dir = Some("data/synthea/fhir".into());
        assert_eq!(config.synthea_path().unwrap(), PathBuf::from("data/synthea/fhir"));
    }

    #[test]
    fn parse_toml_string() {
        let toml_str = r#"
[server]
listen_addr = "0.0.0.0:8080"

[llm]
backend = "ollama"
ollama_model = "llama3"
max_concurrent_calls = 8
max_tokens_per_encounter = 100000

[fhir]
synthea_dir = "data/synthea/fhir"

[database]
url = "sqlite:test.db"

[policy]
policies_dir = "policies/"

[cors]
origins = ["http://localhost:3000", "http://localhost:3001"]
"#;
        let config: ClinicLawConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.listen_addr, "0.0.0.0:8080");
        assert_eq!(config.llm.backend, "ollama");
        assert_eq!(config.llm.ollama_model, "llama3");
        assert_eq!(config.llm.max_concurrent_calls, 8);
        assert_eq!(config.fhir.synthea_dir, Some("data/synthea/fhir".into()));
        assert_eq!(config.database.url, "sqlite:test.db");
        assert_eq!(config.cors.origins.len(), 2);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let toml_str = r#"
[server]
listen_addr = "127.0.0.1:9000"
"#;
        let config: ClinicLawConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.listen_addr, "127.0.0.1:9000");
        // Everything else should be default
        assert_eq!(config.llm.backend, "mock");
        assert_eq!(config.database.url, "sqlite:cliniclaw.sqlite");
    }
}
