//! Temporal supersession: when a newer artifact on the same topic replaces an older one,
//! search returns only the latest decision, not the entire history.
//!
//! Detection: compare topic signatures (technology + domain + category terms).
//! If >70% Jaccard overlap AND same artifact type AND newer date → supersedes.

use std::collections::{HashMap, HashSet};

use crate::ingest::Artifact;
use crate::index::SearchResult;

/// Minimum Jaccard similarity to consider two artifacts as covering the same topic.
const TOPIC_OVERLAP_THRESHOLD: f64 = 0.70;

/// Configuration for supersession behavior at search time.
#[derive(Debug, Clone)]
pub struct SupersessionConfig {
    /// If true, superseded artifacts are removed from results entirely.
    /// If false, they are flagged and ranked lower.
    pub hide_superseded: bool,
}

impl Default for SupersessionConfig {
    fn default() -> Self {
        Self {
            hide_superseded: true,
        }
    }
}

/// A topic signature extracted from an artifact's content.
/// Consists of technology terms, domain terms, and category markers.
#[derive(Debug, Clone)]
pub struct TopicSignature {
    pub terms: HashSet<String>,
    pub artifact_type: Option<String>,
}

/// Tracks supersession relationships across all known artifacts.
#[derive(Debug, Default)]
pub struct SupersessionIndex {
    /// artifact_id -> id of the artifact that supersedes it (if any)
    superseded_by: HashMap<String, String>,
    /// artifact_id -> TopicSignature
    signatures: HashMap<String, TopicSignature>,
    /// artifact_id -> created_date (YYYY-MM-DD)
    dates: HashMap<String, String>,
}

// --- Term extraction ---

/// Technology and protocol terms that signal a specific decision domain.
const TECH_TERMS: &[&str] = &[
    "rest", "graphql", "grpc", "http", "websocket", "mqtt", "amqp",
    "postgres", "mysql", "sqlite", "redis", "mongodb", "dynamodb", "cassandra",
    "docker", "kubernetes", "terraform", "ansible", "nginx", "caddy",
    "react", "vue", "svelte", "angular", "nextjs", "astro", "remix",
    "python", "rust", "typescript", "golang", "java", "swift", "kotlin",
    "aws", "gcp", "azure", "cloudflare", "vercel", "netlify",
    "jwt", "oauth", "saml", "oidc", "argon2", "bcrypt",
    "parquet", "arrow", "json", "yaml", "toml", "protobuf", "avro",
    "tantivy", "elasticsearch", "meilisearch", "typesense",
    "kafka", "rabbitmq", "nats", "pulsar",
    "s3", "gcs", "blob",
];

/// Domain and category terms that identify what area a decision covers.
const DOMAIN_TERMS: &[&str] = &[
    "api", "protocol", "database", "storage", "cache", "queue", "messaging",
    "authentication", "authorization", "security", "encryption", "hashing",
    "deployment", "infrastructure", "ci", "cd", "pipeline",
    "frontend", "backend", "fullstack", "server", "client",
    "framework", "library", "language", "runtime",
    "architecture", "design", "pattern", "model", "schema",
    "testing", "monitoring", "logging", "observability",
    "config", "configuration", "settings", "environment",
    "migration", "upgrade", "refactor", "rewrite",
    "memory", "search", "index", "ingest", "recall",
];

/// Decision-type markers. If the content contains these, it's likely a decision artifact.
const DECISION_MARKERS: &[&str] = &[
    "decision", "decided", "choosing", "chose", "switch", "switched",
    "migrate", "migrated", "adopt", "adopted", "replace", "replaced",
    "moving to", "moved to", "use instead", "going with", "picked",
    "selected", "selecting", "prefer", "preferred",
];

/// Extract a topic signature from artifact content.
pub fn extract_topic_signature(content: &str) -> TopicSignature {
    let lower = content.to_lowercase();
    let mut terms = HashSet::new();

    // Extract technology terms present in content
    for &term in TECH_TERMS {
        if lower.contains(term) {
            terms.insert(term.to_string());
        }
    }

    // Extract domain terms present in content
    for &term in DOMAIN_TERMS {
        if lower.contains(term) {
            terms.insert(format!("domain:{}", term));
        }
    }

    // Determine artifact type from decision markers
    let artifact_type = if DECISION_MARKERS.iter().any(|m| lower.contains(m)) {
        Some("decision".to_string())
    } else if lower.contains("error") || lower.contains("bug") || lower.contains("fix") {
        Some("incident".to_string())
    } else if lower.contains("todo") || lower.contains("task") || lower.contains("plan") {
        Some("plan".to_string())
    } else {
        None
    };

    TopicSignature {
        terms,
        artifact_type,
    }
}

/// Jaccard similarity between two term sets: |A ∩ B| / |A ∪ B|.
fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    intersection / union
}

impl SupersessionIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an artifact. Returns the artifact_id it supersedes, if any.
    pub fn register(&mut self, artifact: &Artifact) -> Option<String> {
        let sig = extract_topic_signature(&artifact.content);
        let new_date = &artifact.created_date;

        let mut supersedes_id: Option<String> = None;
        let mut best_similarity: f64 = 0.0;

        // Compare against all existing signatures
        for (existing_id, existing_sig) in &self.signatures {
            // Skip if different artifact types (both must be decisions, etc.)
            match (&sig.artifact_type, &existing_sig.artifact_type) {
                (Some(a), Some(b)) if a == b => {}
                _ => continue,
            }

            // Must have enough topic terms to compare meaningfully
            if sig.terms.len() < 2 || existing_sig.terms.len() < 2 {
                continue;
            }

            let similarity = jaccard_similarity(&sig.terms, &existing_sig.terms);

            if similarity >= TOPIC_OVERLAP_THRESHOLD && similarity > best_similarity {
                // Check date: new artifact must be newer
                let existing_date = self.dates.get(existing_id).map(|s| s.as_str()).unwrap_or("");
                if new_date.as_str() > existing_date {
                    // Don't supersede something that's already superseded by something else newer
                    best_similarity = similarity;
                    supersedes_id = Some(existing_id.clone());
                }
            }
        }

        // Record the supersession relationship
        if let Some(ref old_id) = supersedes_id {
            self.superseded_by.insert(old_id.clone(), artifact.artifact_id.clone());
        }

        // Store this artifact's signature and date
        self.signatures.insert(artifact.artifact_id.clone(), sig);
        self.dates.insert(artifact.artifact_id.clone(), artifact.created_date.clone());

        supersedes_id
    }

    /// Check if an artifact has been superseded.
    pub fn is_superseded(&self, artifact_id: &str) -> bool {
        self.superseded_by.contains_key(artifact_id)
    }

    /// Get the id of the artifact that supersedes the given one, if any.
    pub fn superseded_by(&self, artifact_id: &str) -> Option<&str> {
        self.superseded_by.get(artifact_id).map(|s| s.as_str())
    }

    /// Resolve a supersession chain: given an artifact_id, find the latest
    /// artifact in its chain. A supersedes B supersedes C → returns A.
    pub fn resolve_chain(&self, artifact_id: &str) -> String {
        // Build reverse map: superseder -> superseded (we have superseded -> superseder)
        // Walk forward from artifact_id through the chain
        let mut current = artifact_id.to_string();
        let mut visited = HashSet::new();
        visited.insert(current.clone());

        while let Some(newer) = self.superseded_by.get(current.as_str()) {
            if !visited.insert(newer.clone()) {
                break; // cycle guard
            }
            current = newer.clone();
        }
        current
    }

    /// Filter and re-rank search results based on supersession relationships.
    pub fn filter_results(
        &self,
        results: Vec<SearchResult>,
        config: &SupersessionConfig,
    ) -> Vec<SearchResult> {
        // Collect all artifact IDs in the result set
        let result_ids: HashSet<String> = results.iter().map(|r| r.artifact_id.clone()).collect();

        let mut filtered: Vec<SearchResult> = Vec::with_capacity(results.len());

        for mut result in results {
            let id = result.artifact_id.as_str();

            if self.is_superseded(id) {
                if config.hide_superseded {
                    // Check if the superseding artifact is in the result set
                    let latest = self.resolve_chain(id);
                    if latest.as_str() != id && result_ids.contains(&latest) {
                        // The newer version is present, safe to hide this one
                        continue;
                    }
                    // If the superseding artifact isn't in results, keep this one
                    // (it's better to show an old decision than nothing)
                    result.superseded = true;
                    result.score *= 0.3; // heavy penalty
                    filtered.push(result);
                } else {
                    // Flag but keep
                    result.superseded = true;
                    result.score *= 0.3;
                    filtered.push(result);
                }
            } else {
                // Not superseded: check if this artifact supersedes something
                // and give it a small boost for being the latest decision
                let is_superseder = self.superseded_by.values().any(|v| v == id);
                if is_superseder {
                    result.score *= 1.2; // boost for being the latest
                }
                filtered.push(result);
            }
        }

        // Re-sort by adjusted score (descending)
        filtered.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        filtered
    }

    /// Load supersession relationships from stored artifacts (e.g., from hot/ JSON files).
    /// Call this at startup to rebuild the in-memory index.
    pub fn load_from_artifacts(&mut self, artifacts: &[Artifact]) {
        // Sort by date so older artifacts are registered first
        let mut sorted: Vec<&Artifact> = artifacts.iter().collect();
        sorted.sort_by(|a, b| a.created_date.cmp(&b.created_date));

        for artifact in sorted {
            // Check if the artifact already has a supersedes field set
            if let Some(ref old_id) = artifact.supersedes {
                self.superseded_by.insert(old_id.clone(), artifact.artifact_id.clone());
                let sig = extract_topic_signature(&artifact.content);
                self.signatures.insert(artifact.artifact_id.clone(), sig);
                self.dates.insert(artifact.artifact_id.clone(), artifact.created_date.clone());
            } else {
                self.register(artifact);
            }
        }
    }

    /// Serialize the supersession map to JSON for persistence.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "superseded_by": self.superseded_by,
        })
    }

    /// Load supersession relationships from a JSON value.
    pub fn load_from_json(&mut self, value: &serde_json::Value) {
        if let Some(map) = value.get("superseded_by").and_then(|v| v.as_object()) {
            for (old_id, new_id) in map {
                if let Some(new_id_str) = new_id.as_str() {
                    self.superseded_by.insert(old_id.clone(), new_id_str.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_artifact(id: &str, content: &str, date: &str) -> Artifact {
        Artifact {
            artifact_id: id.to_string(),
            content: content.to_string(),
            content_hash: format!("{}_hash", id),
            summary: String::new(),
            keywords: vec![],
            significance: 0.5,
            source_label: "test".to_string(),
            source_path: "/test".to_string(),
            created_date: date.to_string(),
            original_size: content.len() as u64,
            supersedes: None,
            semantic: crate::semantic::SemanticFields::default(),
        }
    }

    #[test]
    fn test_topic_signature_extraction() {
        let sig = extract_topic_signature(
            "We decided to use GraphQL for the API protocol instead of REST",
        );
        assert!(sig.terms.contains("graphql"));
        assert!(sig.terms.contains("rest"));
        assert!(sig.terms.contains("domain:api"));
        assert!(sig.terms.contains("domain:protocol"));
        assert_eq!(sig.artifact_type, Some("decision".to_string()));
    }

    #[test]
    fn test_jaccard_similarity() {
        let a: HashSet<String> = ["x", "y", "z"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["x", "y", "w"].iter().map(|s| s.to_string()).collect();
        let sim = jaccard_similarity(&a, &b);
        // intersection = {x, y} = 2, union = {x, y, z, w} = 4 → 0.5
        assert!((sim - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_supersession_detection() {
        let mut idx = SupersessionIndex::new();

        let old = make_artifact(
            "old1",
            "We decided to use REST for the API protocol. This is our architecture decision for the backend server.",
            "2026-01-15",
        );
        let new = make_artifact(
            "new1",
            "We decided to switch to GraphQL for the API protocol. Replacing REST on the backend server.",
            "2026-03-10",
        );

        let result1 = idx.register(&old);
        assert!(result1.is_none()); // nothing to supersede yet

        let result2 = idx.register(&new);
        assert_eq!(result2, Some("old1".to_string()));

        assert!(idx.is_superseded("old1"));
        assert!(!idx.is_superseded("new1"));
    }

    #[test]
    fn test_chain_resolution() {
        let mut idx = SupersessionIndex::new();

        let a = make_artifact(
            "a",
            "We decided to use REST for the API protocol on the backend server architecture.",
            "2026-01-01",
        );
        let b = make_artifact(
            "b",
            "We decided to switch to GraphQL for the API protocol on the backend server architecture.",
            "2026-02-01",
        );
        let c = make_artifact(
            "c",
            "We decided to adopt gRPC for the API protocol on the backend server architecture.",
            "2026-03-01",
        );

        idx.register(&a);
        idx.register(&b);
        idx.register(&c);

        // A is superseded by B, B is superseded by C
        // Resolving from A should give C
        assert_eq!(idx.resolve_chain("a"), "c");
        assert_eq!(idx.resolve_chain("b"), "c");
        assert_eq!(idx.resolve_chain("c"), "c");
    }

    #[test]
    fn test_filter_hides_superseded() {
        let mut idx = SupersessionIndex::new();

        let old = make_artifact(
            "old1",
            "We decided to use REST for the API protocol. Architecture decision for backend server.",
            "2026-01-15",
        );
        let new = make_artifact(
            "new1",
            "We decided to switch to GraphQL for the API protocol. Replacing REST on backend server.",
            "2026-03-10",
        );

        idx.register(&old);
        idx.register(&new);

        let results = vec![
            SearchResult {
                artifact_id: "old1".to_string(),
                content_hash: "hash1".to_string(),
                summary: "REST decision".to_string(),
                significance: 0.8,
                created_date: "2026-01-15".to_string(),
                source_label: "test".to_string(),
                score: 5.0,
                superseded: false,
                superseded_by: None,
            },
            SearchResult {
                artifact_id: "new1".to_string(),
                content_hash: "hash2".to_string(),
                summary: "GraphQL decision".to_string(),
                significance: 0.8,
                created_date: "2026-03-10".to_string(),
                source_label: "test".to_string(),
                score: 4.5,
                superseded: false,
                superseded_by: None,
            },
        ];

        let config = SupersessionConfig { hide_superseded: true };
        let filtered = idx.filter_results(results, &config);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].artifact_id, "new1");
    }

    #[test]
    fn test_no_supersession_different_topics() {
        let mut idx = SupersessionIndex::new();

        let db_decision = make_artifact(
            "db1",
            "We decided to use Postgres for the database storage layer.",
            "2026-01-15",
        );
        let api_decision = make_artifact(
            "api1",
            "We decided to use GraphQL for the API protocol on the frontend client.",
            "2026-03-10",
        );

        idx.register(&db_decision);
        let result = idx.register(&api_decision);

        // Different topics → no supersession
        assert!(result.is_none());
    }

    #[test]
    fn test_no_supersession_older_date() {
        let mut idx = SupersessionIndex::new();

        let newer = make_artifact(
            "newer",
            "We decided to use GraphQL for the API protocol on the backend server.",
            "2026-03-10",
        );
        let older = make_artifact(
            "older",
            "We decided to use REST for the API protocol on the backend server.",
            "2026-01-15",
        );

        idx.register(&newer);
        let result = idx.register(&older);

        // Older artifact can't supersede a newer one
        assert!(result.is_none());
    }
}
