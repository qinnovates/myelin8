use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::Config;
use crate::index::SearchIndex;
use crate::ingest::Artifact;
use crate::recall;
use crate::store::ParquetStore;

/// Run the MCP stdio server. Reads JSON-RPC from stdin, writes responses to stdout.
pub async fn serve(config: Config) -> Result<()> {
    let data_dir = config.data_dir();
    let index_dir = data_dir.join("index");
    let store_dir = data_dir.join("store");
    let hot_dir = data_dir.join("hot");

    // Ensure dirs exist
    std::fs::create_dir_all(&index_dir)?;
    std::fs::create_dir_all(&store_dir)?;
    std::fs::create_dir_all(&hot_dir)?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                write_response(&mut stdout, &err_resp).await?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(&id),
            "notifications/initialized" => {
                // Client acknowledgment, no response needed
                continue;
            }
            "tools/list" => handle_tools_list(&id),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                handle_tools_call(&id, &params, &config, &index_dir, &store_dir, &hot_dir)
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {}", method)
                    }
                })
            }
        };

        write_response(&mut stdout, &response).await?;
    }

    Ok(())
}

async fn write_response(stdout: &mut io::Stdout, response: &Value) -> Result<()> {
    let mut out = serde_json::to_string(response)?;
    out.push('\n');
    stdout.write_all(out.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "myelin8",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "memory_search",
                    "description": "Search indexed memories by query. Returns ranked summaries with metadata.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Search query (full-text search across memory content and summaries)"
                            },
                            "after": {
                                "type": "string",
                                "description": "Only return results after this date (YYYY-MM-DD)"
                            },
                            "before": {
                                "type": "string",
                                "description": "Only return results before this date (YYYY-MM-DD)"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Maximum number of results to return (default: 10)"
                            }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_recall",
                    "description": "Get full content of a specific artifact by ID. Returns content with integrity verification status.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "artifact_id": {
                                "type": "string",
                                "description": "The artifact ID or content hash prefix to recall"
                            }
                        },
                        "required": ["artifact_id"]
                    }
                },
                {
                    "name": "memory_status",
                    "description": "Get system status: artifact counts, index stats, registered sources.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "memory_ingest",
                    "description": "Ingest a note directly into memory. Use when the AI wants to save something for later recall.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The content to store"
                            },
                            "label": {
                                "type": "string",
                                "description": "Label/category for this memory (e.g., 'conversation', 'note', 'decision')"
                            }
                        },
                        "required": ["content", "label"]
                    }
                }
            ]
        }
    })
}

fn handle_tools_call(
    id: &Value,
    params: &Value,
    config: &Config,
    index_dir: &std::path::Path,
    store_dir: &std::path::Path,
    hot_dir: &std::path::Path,
) -> Value {
    let tool_name = params
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = match tool_name {
        "memory_search" => tool_memory_search(&arguments, index_dir),
        "memory_recall" => tool_memory_recall(&arguments, store_dir, hot_dir),
        "memory_status" => tool_memory_status(config, index_dir, store_dir, hot_dir),
        "memory_ingest" => tool_memory_ingest(&arguments, index_dir, hot_dir),
        _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": content
                    }
                ]
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": format!("Error: {}", e)
                    }
                ],
                "isError": true
            }
        }),
    }
}

fn tool_memory_search(args: &Value, index_dir: &std::path::Path) -> Result<String> {
    let query = args
        .get("query")
        .and_then(|q| q.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: query"))?;

    let after = args.get("after").and_then(|a| a.as_str());
    let before = args.get("before").and_then(|b| b.as_str());
    let limit = args
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(10) as usize;

    let index = SearchIndex::open_or_create(index_dir)?;
    let results = index.search(query, after, before, limit)?;

    if results.is_empty() {
        return Ok("No results found.".to_string());
    }

    let output: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "artifact_id": r.artifact_id,
                "summary": r.summary,
                "significance": r.significance,
                "created_date": r.created_date,
                "source_label": r.source_label,
                "content_hash": &r.content_hash[..16.min(r.content_hash.len())],
                "score": r.score
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&output)?)
}

fn tool_memory_recall(
    args: &Value,
    store_dir: &std::path::Path,
    hot_dir: &std::path::Path,
) -> Result<String> {
    let artifact_id = args
        .get("artifact_id")
        .and_then(|a| a.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: artifact_id"))?;

    // Check hot/ first (plaintext JSON)
    let hot_path = hot_dir.join(format!("{}.json", artifact_id));
    if hot_path.exists() {
        let content = std::fs::read_to_string(&hot_path)?;
        let artifact: Value = serde_json::from_str(&content)?;
        return Ok(serde_json::to_string_pretty(&json!({
            "artifact_id": artifact_id,
            "content": artifact.get("content").and_then(|c| c.as_str()).unwrap_or(""),
            "integrity": "HOT — plaintext, not yet compacted",
            "source": "hot"
        }))?);
    }

    // Check Parquet store
    let store = ParquetStore::new(store_dir);
    let result = recall::recall_from_store(&store, artifact_id)?;

    match result {
        Some(recalled) => Ok(serde_json::to_string_pretty(&json!({
            "artifact_id": artifact_id,
            "content": recalled.content,
            "integrity": recalled.integrity_status,
            "content_hash": recalled.content_hash,
            "source": "parquet"
        }))?),
        None => Ok(format!("Artifact '{}' not found.", artifact_id)),
    }
}

fn tool_memory_status(
    config: &Config,
    index_dir: &std::path::Path,
    store_dir: &std::path::Path,
    hot_dir: &std::path::Path,
) -> Result<String> {
    // Hot count
    let hot_count = std::fs::read_dir(hot_dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
                .count()
        })
        .unwrap_or(0);

    // Parquet stats
    let mut parquet_count = 0;
    let mut parquet_bytes = 0u64;
    if let Ok(entries) = std::fs::read_dir(store_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry
                .path()
                .extension()
                .map_or(false, |ext| ext == "parquet")
            {
                parquet_count += 1;
                parquet_bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }

    // Index stats
    let index = SearchIndex::open_or_create(index_dir)?;
    let index_stats = index.stats()?;

    // Sources
    let sources: Vec<Value> = config
        .sources
        .iter()
        .map(|s| {
            json!({
                "label": s.label,
                "path": s.path,
                "pattern": s.pattern
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "hot_artifacts": hot_count,
        "parquet_files": parquet_count,
        "parquet_bytes": parquet_bytes,
        "index_docs": index_stats.num_docs,
        "index_terms": index_stats.num_terms,
        "sources": sources
    }))?)
}

fn tool_memory_ingest(
    args: &Value,
    index_dir: &std::path::Path,
    hot_dir: &std::path::Path,
) -> Result<String> {
    let content = args
        .get("content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

    let label = args
        .get("label")
        .and_then(|l| l.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: label"))?;

    let content_hash = hex::encode(Sha256::digest(content.as_bytes()));
    let artifact_id = format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        &content_hash[..8]
    );

    // Build a summary: first 200 chars, single line
    let summary = content
        .chars()
        .take(200)
        .collect::<String>()
        .replace('\n', " ")
        .trim()
        .to_string();

    let semantic_fields = crate::semantic::extract(content);

    let artifact = Artifact {
        artifact_id: artifact_id.clone(),
        content: content.to_string(),
        content_hash: content_hash.clone(),
        summary: summary.clone(),
        keywords: vec![label.to_string()],
        significance: 0.5,
        source_label: label.to_string(),
        source_path: "mcp-ingest".to_string(),
        created_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        original_size: content.len() as u64,
        supersedes: None,
        semantic: semantic_fields,
    };

    // Index in tantivy
    let mut index = SearchIndex::open_or_create(index_dir)?;
    index.add_artifact(&artifact)?;
    index.commit()?;

    // Write to hot/ as JSON
    let hot_path = hot_dir.join(format!("{}.json", artifact_id));
    let hot_json = serde_json::to_string_pretty(&artifact)?;
    std::fs::write(&hot_path, &hot_json)?;

    Ok(serde_json::to_string_pretty(&json!({
        "artifact_id": artifact_id,
        "content_hash": &content_hash[..16],
        "summary": summary,
        "status": "ingested"
    }))?)
}
