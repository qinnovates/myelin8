use std::collections::{HashMap, HashSet};

/// Semantic metadata extracted from artifact content at ingest time.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SemanticFields {
    /// Recognized technology terms (postgresql, redis, kubernetes, etc.)
    pub technology: Vec<String>,
    /// Domain category (database, caching, security, devops, etc.)
    pub category: Vec<String>,
    /// Action verbs (decided, fixed, deployed, discovered, etc.)
    pub action: Vec<String>,
    /// Area of work (backend, frontend, infrastructure, auth, etc.)
    pub domain: Vec<String>,
    /// Sentiment polarity: "positive", "negative", or "neutral"
    pub polarity: String,
    /// People, services, and tools mentioned
    pub entities: Vec<String>,
}

// ---------------------------------------------------------------------------
// Static term sets for extraction
// ---------------------------------------------------------------------------

/// Technology terms and the category they map to.
/// Returns (term, category).
fn tech_terms() -> &'static [(&'static str, &'static str)] {
    &[
        // Databases
        ("postgresql", "database"), ("postgres", "database"), ("mysql", "database"),
        ("sqlite", "database"), ("mongodb", "database"), ("redis", "database"),
        ("dynamodb", "database"), ("cassandra", "database"), ("mariadb", "database"),
        ("cockroachdb", "database"), ("neo4j", "database"), ("elasticsearch", "database"),
        ("opensearch", "database"), ("clickhouse", "database"), ("timescaledb", "database"),
        ("influxdb", "database"), ("duckdb", "database"), ("parquet", "database"),
        ("supabase", "database"), ("firestore", "database"),
        // Caching
        ("memcached", "caching"), ("varnish", "caching"), ("cdn", "caching"),
        ("cloudflare", "caching"),
        // Container / orchestration
        ("docker", "container"), ("kubernetes", "container"), ("k8s", "container"),
        ("podman", "container"), ("helm", "container"), ("istio", "container"),
        ("envoy", "container"),
        // Cloud
        ("aws", "cloud"), ("gcp", "cloud"), ("azure", "cloud"),
        ("lambda", "cloud"), ("s3", "cloud"), ("ec2", "cloud"),
        ("fargate", "cloud"), ("cloudrun", "cloud"), ("vercel", "cloud"),
        ("netlify", "cloud"), ("heroku", "cloud"), ("railway", "cloud"),
        // CI/CD
        ("github_actions", "cicd"), ("jenkins", "cicd"), ("circleci", "cicd"),
        ("gitlab_ci", "cicd"), ("argocd", "cicd"), ("terraform", "cicd"),
        ("pulumi", "cicd"), ("ansible", "cicd"),
        // Languages / runtimes
        ("rust", "language"), ("python", "language"), ("typescript", "language"),
        ("javascript", "language"), ("golang", "language"), ("java", "language"),
        ("swift", "language"), ("kotlin", "language"), ("ruby", "language"),
        ("elixir", "language"), ("haskell", "language"), ("zig", "language"),
        ("node", "language"), ("deno", "language"), ("bun", "language"),
        // Frameworks
        ("react", "framework"), ("nextjs", "framework"), ("astro", "framework"),
        ("django", "framework"), ("flask", "framework"), ("fastapi", "framework"),
        ("express", "framework"), ("axum", "framework"), ("actix", "framework"),
        ("spring", "framework"), ("rails", "framework"), ("phoenix", "framework"),
        ("svelte", "framework"), ("vue", "framework"), ("angular", "framework"),
        ("tailwind", "framework"), ("htmx", "framework"),
        // Security
        ("oauth", "security"), ("jwt", "security"), ("tls", "security"),
        ("ssl", "security"), ("cors", "security"), ("csrf", "security"),
        ("xss", "security"), ("sqli", "security"), ("rbac", "security"),
        ("iam", "security"), ("vault", "security"), ("keycloak", "security"),
        ("argon2", "security"), ("bcrypt", "security"), ("sha256", "security"),
        ("pqc", "security"), ("kyber", "security"), ("dilithium", "security"),
        // Messaging / queues
        ("kafka", "messaging"), ("rabbitmq", "messaging"), ("nats", "messaging"),
        ("sqs", "messaging"), ("pubsub", "messaging"), ("zeromq", "messaging"),
        // Monitoring / observability
        ("prometheus", "monitoring"), ("grafana", "monitoring"), ("datadog", "monitoring"),
        ("splunk", "monitoring"), ("sentry", "monitoring"), ("opentelemetry", "monitoring"),
        ("jaeger", "monitoring"), ("loki", "monitoring"),
        // Search
        ("tantivy", "search"), ("solr", "search"), ("meilisearch", "search"),
        ("typesense", "search"), ("algolia", "search"),
        // ML / AI
        ("pytorch", "ml"), ("tensorflow", "ml"), ("transformers", "ml"),
        ("llm", "ml"), ("gpt", "ml"), ("claude", "ml"), ("embeddings", "ml"),
        ("onnx", "ml"), ("huggingface", "ml"),
        // Protocols
        ("grpc", "protocol"), ("graphql", "protocol"), ("rest", "protocol"),
        ("websocket", "protocol"), ("mqtt", "protocol"), ("http2", "protocol"),
        ("http3", "protocol"), ("quic", "protocol"),
        // OS / infra
        ("linux", "infrastructure"), ("nginx", "infrastructure"),
        ("caddy", "infrastructure"), ("systemd", "infrastructure"),
        ("wireguard", "infrastructure"),
        // Testing
        ("pytest", "testing"), ("jest", "testing"), ("vitest", "testing"),
        ("playwright", "testing"), ("selenium", "testing"), ("cypress", "testing"),
        // Version control
        ("git", "vcs"), ("github", "vcs"), ("gitlab", "vcs"), ("bitbucket", "vcs"),
    ]
}

/// Action verbs to detect in content.
fn action_terms() -> &'static [&'static str] {
    &[
        "decided", "chose", "selected", "picked", "agreed",
        "fixed", "resolved", "patched", "repaired", "corrected",
        "deployed", "shipped", "released", "launched", "published",
        "discovered", "found", "noticed", "identified", "detected",
        "implemented", "built", "created", "added", "wrote",
        "removed", "deleted", "dropped", "deprecated", "killed",
        "migrated", "moved", "transferred", "ported", "upgraded",
        "configured", "set up", "initialized", "provisioned",
        "refactored", "restructured", "reorganized", "simplified",
        "tested", "verified", "validated", "benchmarked", "profiled",
        "reviewed", "audited", "inspected", "analyzed",
        "documented", "noted", "recorded", "logged",
        "blocked", "failed", "broke", "crashed", "errored",
        "reverted", "rolled back", "undid", "restored",
        "optimized", "improved", "accelerated", "reduced",
        "integrated", "connected", "linked", "wired", "hooked",
        "disabled", "enabled", "toggled", "switched",
        "investigated", "debugged", "traced", "diagnosed",
        "proposed", "suggested", "recommended", "planned",
        "rejected", "declined", "vetoed", "denied",
        "approved", "accepted", "confirmed", "signed off",
    ]
}

/// Domain areas.
fn domain_terms() -> &'static [(&'static str, &'static str)] {
    &[
        // Backend
        ("api", "backend"), ("endpoint", "backend"), ("route", "backend"),
        ("handler", "backend"), ("middleware", "backend"), ("server", "backend"),
        ("microservice", "backend"), ("service", "backend"), ("rpc", "backend"),
        // Frontend
        ("component", "frontend"), ("page", "frontend"), ("layout", "frontend"),
        ("ui", "frontend"), ("ux", "frontend"), ("css", "frontend"),
        ("style", "frontend"), ("render", "frontend"), ("dom", "frontend"),
        ("responsive", "frontend"), ("animation", "frontend"),
        // Infrastructure
        ("deploy", "infrastructure"), ("pipeline", "infrastructure"),
        ("container", "infrastructure"), ("cluster", "infrastructure"),
        ("load balancer", "infrastructure"), ("dns", "infrastructure"),
        ("cert", "infrastructure"), ("firewall", "infrastructure"),
        ("network", "infrastructure"), ("proxy", "infrastructure"),
        ("scaling", "infrastructure"), ("autoscale", "infrastructure"),
        // Auth
        ("auth", "auth"), ("login", "auth"), ("signup", "auth"),
        ("password", "auth"), ("session", "auth"), ("token", "auth"),
        ("permission", "auth"), ("role", "auth"), ("access control", "auth"),
        ("sso", "auth"), ("mfa", "auth"), ("2fa", "auth"),
        // Data
        ("schema", "data"), ("migration", "data"), ("query", "data"),
        ("index", "data"), ("table", "data"), ("column", "data"),
        ("row", "data"), ("record", "data"), ("etl", "data"),
        ("pipeline", "data"), ("warehouse", "data"), ("lakehouse", "data"),
        // DevOps
        ("ci", "devops"), ("cd", "devops"), ("build", "devops"),
        ("release", "devops"), ("rollback", "devops"), ("canary", "devops"),
        ("blue green", "devops"), ("feature flag", "devops"),
        ("monitoring", "devops"), ("alerting", "devops"), ("incident", "devops"),
        // Testing
        ("test", "testing"), ("spec", "testing"), ("coverage", "testing"),
        ("assertion", "testing"), ("mock", "testing"), ("fixture", "testing"),
        ("integration test", "testing"), ("unit test", "testing"),
        ("e2e", "testing"), ("regression", "testing"),
        // Documentation
        ("readme", "documentation"), ("changelog", "documentation"),
        ("docs", "documentation"), ("docstring", "documentation"),
        ("tutorial", "documentation"), ("guide", "documentation"),
        // Performance
        ("latency", "performance"), ("throughput", "performance"),
        ("benchmark", "performance"), ("profiling", "performance"),
        ("cache hit", "performance"), ("bottleneck", "performance"),
        ("memory leak", "performance"), ("cpu", "performance"),
    ]
}

/// Negative signal words for polarity detection.
fn negative_signals() -> &'static [&'static str] {
    &[
        "not", "didn't", "didn't", "rejected", "against", "failed",
        "broken", "broke", "crash", "crashed", "error", "bug",
        "problem", "issue", "wrong", "bad", "worse", "worst",
        "blocked", "stuck", "impossible", "never", "won't",
        "can't", "cannot", "shouldn't", "shouldn't", "unable",
        "deprecated", "removed", "killed", "dropped", "reverted",
        "rollback", "rolled back", "downgrade", "regression",
        "vulnerability", "exploit", "breach", "leak", "incident",
        "outage", "downtime", "degraded", "slow", "timeout",
    ]
}

/// Positive signal words for polarity detection.
fn positive_signals() -> &'static [&'static str] {
    &[
        "fixed", "resolved", "shipped", "deployed", "launched",
        "improved", "optimized", "faster", "better", "solved",
        "completed", "done", "finished", "succeeded", "passed",
        "approved", "accepted", "merged", "released", "stable",
        "secure", "validated", "verified", "confirmed", "green",
        "upgraded", "enhanced", "added", "new", "enabled",
        "working", "ready", "clean", "refactored", "simplified",
    ]
}

// ---------------------------------------------------------------------------
// Synonym map for query expansion
// ---------------------------------------------------------------------------

/// Build the static synonym map. Each key maps to a set of synonyms
/// (including itself).
pub fn synonym_map() -> HashMap<&'static str, Vec<&'static str>> {
    let groups: &[&[&str]] = &[
        // Databases
        &["database", "db", "rdbms", "postgresql", "postgres", "mysql", "sqlite", "mongodb", "datastore", "sql"],
        &["cache", "caching", "redis", "memcached", "varnish", "cdn"],
        &["queue", "kafka", "rabbitmq", "nats", "sqs", "pubsub", "message broker", "messaging"],
        // Actions: decide
        &["decided", "chose", "selected", "picked", "agreed", "went with", "opted for"],
        // Actions: fix
        &["fixed", "resolved", "patched", "repaired", "corrected", "solved", "addressed"],
        // Actions: deploy
        &["deployed", "shipped", "released", "launched", "published", "pushed", "went live"],
        // Actions: discover
        &["discovered", "found", "noticed", "identified", "detected", "spotted", "uncovered"],
        // Actions: create
        &["created", "built", "implemented", "added", "wrote", "developed", "authored", "introduced"],
        // Actions: remove
        &["removed", "deleted", "dropped", "deprecated", "killed", "purged", "eliminated"],
        // Actions: migrate
        &["migrated", "moved", "transferred", "ported", "upgraded", "transitioned"],
        // Actions: refactor
        &["refactored", "restructured", "reorganized", "simplified", "cleaned up", "reworked"],
        // Actions: test
        &["tested", "verified", "validated", "benchmarked", "profiled", "checked", "confirmed"],
        // Actions: review
        &["reviewed", "audited", "inspected", "analyzed", "examined", "evaluated", "assessed"],
        // Actions: break
        &["broke", "broken", "crashed", "failed", "errored", "degraded"],
        // Actions: revert
        &["reverted", "rolled back", "undid", "restored", "reset"],
        // Actions: optimize
        &["optimized", "improved", "accelerated", "reduced", "sped up", "tuned", "enhanced"],
        // Actions: integrate
        &["integrated", "connected", "linked", "wired", "hooked", "coupled"],
        // Actions: debug
        &["debugged", "investigated", "traced", "diagnosed", "troubleshot"],
        // Actions: reject
        &["rejected", "declined", "vetoed", "denied", "refused", "turned down"],
        // Actions: approve
        &["approved", "accepted", "confirmed", "signed off", "green-lit", "okayed"],
        // Tech: container
        &["container", "docker", "kubernetes", "k8s", "podman", "containerized"],
        // Tech: cloud
        &["cloud", "aws", "gcp", "azure", "cloud provider", "iaas", "paas", "saas"],
        // Domain: backend
        &["backend", "server", "api", "endpoint", "service", "microservice", "serverside"],
        // Domain: frontend
        &["frontend", "ui", "ux", "client", "browser", "clientside", "web app"],
        // Domain: infrastructure
        &["infrastructure", "infra", "devops", "platform", "ops", "sre"],
        // Domain: auth
        &["auth", "authentication", "authorization", "login", "sso", "oauth", "identity"],
        // Domain: security
        &["security", "sec", "appsec", "infosec", "cybersecurity", "vulnerability", "threat"],
        // Domain: testing
        &["test", "testing", "spec", "unit test", "integration test", "e2e", "qa", "quality"],
        // Domain: monitoring
        &["monitoring", "observability", "logging", "alerting", "tracing", "metrics", "apm"],
        // Domain: performance
        &["performance", "perf", "latency", "throughput", "benchmark", "speed", "optimization"],
        // Domain: data
        &["data", "schema", "migration", "etl", "pipeline", "warehouse", "lakehouse", "dataset"],
        // Domain: documentation
        &["documentation", "docs", "readme", "changelog", "wiki", "guide", "tutorial"],
        // Language: Rust
        &["rust", "cargo", "crate", "rustc", "clippy"],
        // Language: Python
        &["python", "pip", "pytest", "pypi", "virtualenv", "conda"],
        // Language: JavaScript/TypeScript
        &["javascript", "typescript", "js", "ts", "node", "npm", "yarn", "pnpm", "deno", "bun"],
        // Language: Swift
        &["swift", "swiftui", "uikit", "xcode", "cocoapods", "spm"],
        // Framework: React
        &["react", "nextjs", "jsx", "tsx", "hooks", "react native"],
        // Config
        &["config", "configuration", "settings", "env", "environment", "dotenv", "toml", "yaml"],
        // Error handling
        &["error", "exception", "panic", "crash", "fault", "failure", "bug", "defect", "issue"],
        // CI/CD
        &["ci", "cd", "cicd", "pipeline", "workflow", "github actions", "jenkins", "build"],
        // API
        &["api", "rest", "graphql", "grpc", "endpoint", "route", "rpc", "openapi", "swagger"],
        // Git
        &["git", "commit", "branch", "merge", "rebase", "pull request", "pr", "diff"],
        // Search
        &["search", "query", "index", "tantivy", "elasticsearch", "full text", "fts"],
        // ML/AI
        &["ml", "ai", "machine learning", "deep learning", "model", "inference", "training", "llm"],
        // Encryption
        &["encryption", "encrypt", "decrypt", "cipher", "aes", "rsa", "pqc", "kyber", "tls", "ssl"],
        // File
        &["file", "directory", "folder", "path", "filesystem", "fs"],
        // Network
        &["network", "dns", "http", "https", "tcp", "udp", "websocket", "socket", "proxy"],
    ];

    let mut map: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    for group in groups {
        for &term in *group {
            map.insert(term, group.to_vec());
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Extraction (ingest-time)
// ---------------------------------------------------------------------------

/// Extract semantic metadata from artifact content. Pure string matching,
/// no regex, no external calls.
pub fn extract(text: &str) -> SemanticFields {
    let lower = text.to_lowercase();
    // Pre-split into words for word-boundary matching
    let words: HashSet<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .collect();

    // Technology + category
    let mut technology = Vec::new();
    let mut category_set: HashSet<String> = HashSet::new();
    for &(term, cat) in tech_terms() {
        if term.contains(' ') || term.contains('_') {
            // Multi-word: substring match on lowered text
            if lower.contains(term) {
                if !technology.contains(&term.to_string()) {
                    technology.push(term.to_string());
                }
                category_set.insert(cat.to_string());
            }
        } else if words.contains(term) {
            if !technology.contains(&term.to_string()) {
                technology.push(term.to_string());
            }
            category_set.insert(cat.to_string());
        }
    }

    // Actions
    let mut action = Vec::new();
    for &term in action_terms() {
        if term.contains(' ') {
            if lower.contains(term) && !action.contains(&term.to_string()) {
                action.push(term.to_string());
            }
        } else if words.contains(term) && !action.contains(&term.to_string()) {
            action.push(term.to_string());
        }
    }

    // Domain
    let mut domain_set: HashSet<String> = HashSet::new();
    for &(term, dom) in domain_terms() {
        if term.contains(' ') {
            if lower.contains(term) {
                domain_set.insert(dom.to_string());
            }
        } else if words.contains(term) {
            domain_set.insert(dom.to_string());
        }
    }

    // Polarity
    let mut neg_count = 0u32;
    let mut pos_count = 0u32;
    for &signal in negative_signals() {
        if signal.contains(' ') {
            if lower.contains(signal) { neg_count += 1; }
        } else if words.contains(signal) {
            neg_count += 1;
        }
    }
    for &signal in positive_signals() {
        if signal.contains(' ') {
            if lower.contains(signal) { pos_count += 1; }
        } else if words.contains(signal) {
            pos_count += 1;
        }
    }
    let polarity = if neg_count > pos_count + 2 {
        "negative".to_string()
    } else if pos_count > neg_count + 2 {
        "positive".to_string()
    } else {
        "neutral".to_string()
    };

    // Entities: extract capitalized multi-word sequences that look like names
    // or service names (PascalCase, ALLCAPS acronyms, etc.)
    let entities = extract_entities(text);

    let category: Vec<String> = category_set.into_iter().collect();
    let domain: Vec<String> = domain_set.into_iter().collect();

    SemanticFields {
        technology,
        category,
        action,
        domain,
        polarity,
        entities,
    }
}

/// Extract likely entity names from text. Looks for:
/// - Capitalized words that aren't sentence starters (heuristic: preceded by non-period)
/// - ALLCAPS tokens (3+ chars, not common abbreviations)
/// - PascalCase words
fn extract_entities(text: &str) -> Vec<String> {
    let common_words: HashSet<&str> = [
        "The", "This", "That", "These", "Those", "When", "Where", "What", "Which",
        "How", "Why", "For", "From", "With", "Into", "After", "Before", "About",
        "Also", "But", "And", "Not", "All", "Can", "Has", "Was", "One", "Our",
        "Out", "Are", "Were", "Will", "Would", "Could", "Should", "May", "Might",
        "Just", "Like", "Only", "Very", "Most", "Some", "Many", "Each", "Every",
        "Any", "Few", "More", "Other", "Such", "Than", "Then", "Now", "Here",
        "There", "If", "So", "Yet", "Still", "Even", "Well", "Too", "Much",
        "Back", "Over", "Down", "Off", "On", "Up", "No", "Yes", "Do", "Did",
        "Does", "Done", "Get", "Got", "Let", "Make", "Say", "Said", "See",
        "Need", "Know", "Take", "Come", "Want", "Look", "Use", "Find", "Give",
        "Tell", "Work", "Call", "Try", "Ask", "Keep", "Put", "Run", "Move",
        "Live", "Next", "It", "We", "He", "She", "Its", "His", "Her",
        "My", "We", "They", "You", "I", "A", "An", "To", "In", "Of",
        "Is", "At", "By", "Be", "As", "Or",
        // Common markdown / doc words that appear capitalized
        "TODO", "NOTE", "FIXME", "WARNING", "ERROR", "INFO", "DEBUG",
        "IMPORTANT", "CRITICAL", "MUST", "SHALL", "SHOULD", "MAY",
        "TRUE", "FALSE", "NULL", "NONE", "OK", "HTTP", "HTTPS", "URL",
        "API", "CLI", "SQL", "CSS", "HTML", "JSON", "XML", "YAML", "TOML",
        "UTF", "ASCII", "EOF", "EOM",
    ].into_iter().collect();

    let mut seen = HashSet::new();
    let mut entities = Vec::new();

    for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ':' || c == '(' || c == ')') {
        // Clean trailing punctuation
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-');
        if clean.len() < 2 {
            continue;
        }

        // Skip if it's a common word
        if common_words.contains(clean) {
            continue;
        }

        let is_capitalized = clean.chars().next().map_or(false, |c| c.is_uppercase());
        let is_all_caps = clean.len() >= 3 && clean.chars().all(|c| c.is_uppercase() || c.is_ascii_digit());
        let is_pascal = is_capitalized
            && clean.len() >= 4
            && clean.chars().skip(1).any(|c| c.is_uppercase())
            && clean.chars().any(|c| c.is_lowercase());

        if (is_all_caps || is_pascal) && seen.insert(clean.to_string()) {
            entities.push(clean.to_string());
        }
    }

    entities
}

// ---------------------------------------------------------------------------
// Query expansion (query-time)
// ---------------------------------------------------------------------------

/// Expand a query string using the synonym map. Each recognized term is
/// expanded into an OR group. Unrecognized terms pass through unchanged.
///
/// Example: "database decided" -> "(database OR db OR rdbms OR postgresql ...) (decided OR chose OR selected ...)"
pub fn expand_query(query: &str) -> String {
    let map = synonym_map();
    let mut parts: Vec<String> = Vec::new();

    for token in query.split_whitespace() {
        let lower = token.to_lowercase();
        if let Some(synonyms) = map.get(lower.as_str()) {
            // Build OR group, quoting multi-word synonyms
            let alternatives: Vec<String> = synonyms.iter()
                .map(|s| {
                    if s.contains(' ') {
                        format!("\"{}\"", s)
                    } else {
                        s.to_string()
                    }
                })
                .collect();
            parts.push(format!("({})", alternatives.join(" OR ")));
        } else {
            parts.push(token.to_string());
        }
    }

    parts.join(" ")
}

/// Join a Vec<String> into a single space-separated string for indexing.
pub fn join_field(values: &[String]) -> String {
    values.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_technology() {
        let text = "We deployed PostgreSQL and Redis for the new backend service.";
        let fields = extract(text);
        assert!(fields.technology.contains(&"postgresql".to_string()));
        assert!(fields.technology.contains(&"redis".to_string()));
    }

    #[test]
    fn test_extract_category() {
        let text = "Switched from MySQL to PostgreSQL for better JSON support.";
        let fields = extract(text);
        assert!(fields.category.contains(&"database".to_string()));
    }

    #[test]
    fn test_extract_action() {
        let text = "We decided to use Rust and deployed the service yesterday.";
        let fields = extract(text);
        assert!(fields.action.contains(&"decided".to_string()));
        assert!(fields.action.contains(&"deployed".to_string()));
    }

    #[test]
    fn test_extract_domain() {
        let text = "Updated the API endpoint and fixed the login flow.";
        let fields = extract(text);
        assert!(fields.domain.contains(&"backend".to_string()));
        assert!(fields.domain.contains(&"auth".to_string()));
    }

    #[test]
    fn test_polarity_negative() {
        let text = "The deploy failed and crashed the server. Everything broke. The error was a bug in the rollback code. Problems everywhere. Issue after issue. Unable to fix.";
        let fields = extract(text);
        assert_eq!(fields.polarity, "negative");
    }

    #[test]
    fn test_polarity_positive() {
        let text = "Fixed the bug, deployed successfully, all tests passed. Merged the PR, released v2.0. Everything is stable and working. Clean refactored code ready to ship.";
        let fields = extract(text);
        assert_eq!(fields.polarity, "positive");
    }

    #[test]
    fn test_polarity_neutral() {
        let text = "Added a new configuration file for the service.";
        let fields = extract(text);
        assert_eq!(fields.polarity, "neutral");
    }

    #[test]
    fn test_expand_query_with_synonyms() {
        let expanded = expand_query("database decided");
        assert!(expanded.contains("postgresql"));
        assert!(expanded.contains("chose"));
    }

    #[test]
    fn test_expand_query_unknown_term() {
        let expanded = expand_query("foobar");
        assert_eq!(expanded, "foobar");
    }

    #[test]
    fn test_extract_entities() {
        let text = "Kevin mentioned that CloudFlare and DataDog were options.";
        let entities = extract_entities(text);
        assert!(entities.contains(&"CloudFlare".to_string()));
        assert!(entities.contains(&"DataDog".to_string()));
    }

    #[test]
    fn test_join_field() {
        let v = vec!["rust".to_string(), "python".to_string()];
        assert_eq!(join_field(&v), "rust python");
    }
}
