//! tpt-oracle — AI Disruption Prediction & MCP Agent Server (Phase 4).
//!
//! Three real capabilities sit on top of the Phase-1 in-memory matcher:
//! 1. **Vector embeddings** — [`embed`] maps an [`InstabilityReport`] to a
//!    feature vector so historical-state matching can be done by cosine
//!    similarity (the same math `tpt-keystone-db`'s Prism vector engine uses).
//! 2. **JSON-LD** — [`to_json_ld`] exposes the live plasma state as linked
//!    data via `appfront-ai-schema`, so an external LLM agent can reason over
//!    it.
//! 3. **MCP server** — [`McpServer`] speaks the Model Context Protocol
//!    (JSON-RPC 2.0 over stdio) and exposes `query_plasma_state` /
//!    `suggest_parameters` tools, letting an agent read state and propose
//!    control adjustments for the next shot.

use appfront_ai_schema;
use serde_json::json;
use tpt_fluxstream::detection::InstabilityReport;
use tpt_ui_components::gauge_widget;

/// A recorded historical shot.
#[derive(Debug, Clone)]
pub struct HistoricalShot {
    pub shot: u64,
    pub report: InstabilityReport,
    pub disrupted: bool,
}

/// Tiny nearest-neighbour matcher over historical reports. Distance is the
/// Euclidean distance in `(dominant_freq, score)` space.
#[derive(Debug, Default)]
pub struct StateMatcher {
    pub history: Vec<HistoricalShot>,
}

impl StateMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, shot: HistoricalShot) {
        self.history.push(shot);
    }

    fn distance(&self, a: &InstabilityReport, b: &InstabilityReport) -> f64 {
        let df = a.dominant_freq_hz - b.dominant_freq_hz;
        let ds = a.instability_score - b.instability_score;
        (df * df + ds * ds).sqrt()
    }

    /// Return up to `k` nearest historical shots to `query`, closest first.
    pub fn nearest(&self, query: &InstabilityReport, k: usize) -> Vec<&HistoricalShot> {
        let mut scored: Vec<(&HistoricalShot, f64)> = self
            .history
            .iter()
            .map(|h| (h, self.distance(query, &h.report)))
            .collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).map(|(h, _)| h).collect()
    }
}

/// Embed an [`InstabilityReport`] as a feature vector for vector similarity
/// search (mirrors Prism's embedding step): log-compressed dominant frequency
/// and magnitude plus the raw instability score.
pub fn embed(r: &InstabilityReport) -> Vec<f64> {
    vec![
        (r.dominant_freq_hz + 1.0).ln(),
        r.instability_score,
        (r.dominant_magnitude + 1.0).ln(),
    ]
}

/// Cosine similarity of two embedding vectors in `[-1, 1]`.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Nearest historical shots by embedding cosine similarity (vector-engine
/// style). Returns `(shot, similarity)` pairs, most similar first.
pub fn nearest_by_embedding<'a>(
    history: &'a [HistoricalShot],
    query: &InstabilityReport,
    k: usize,
) -> Vec<(&'a HistoricalShot, f64)> {
    let q = embed(query);
    let mut scored: Vec<(&HistoricalShot, f64)> = history
        .iter()
        .map(|h| (h, cosine_similarity(&q, &embed(&h.report))))
        .collect();
    // Highest similarity first.
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(k).collect()
}

/// Build a `UITree` describing the current plasma state (used for JSON-LD).
pub fn plasma_state_tree(report: &InstabilityReport) -> appfront_core::UITree<()> {
    appfront_core::UITree::container(|c| {
        c.heading(1, format!("Plasma state — shot #{}", report.shot));
        c.with(gauge_widget(
            "Instability Score",
            &tpt_ui_components::PlasmaGauge::new(0.0, 1.0, 0.0, 0.6),
            report.instability_score,
        ));
        c.text(format!(
            "dominant_freq_hz={:.3} dominant_magnitude={:.3} instability_score={:.3}",
            report.dominant_freq_hz, report.dominant_magnitude, report.instability_score
        ));
    })
}

/// Expose the live plasma state as JSON-LD via `appfront-ai-schema`.
pub fn to_json_ld(report: &InstabilityReport) -> serde_json::Value {
    appfront_ai_schema::to_json_ld(&plasma_state_tree(report))
}

/// Shared oracle state exposed over MCP / vector matching.
#[derive(Debug, Default)]
pub struct OracleState {
    pub matcher: StateMatcher,
    pub latest: Option<InstabilityReport>,
}

impl OracleState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest a fresh instability report (records it and updates `latest`).
    pub fn observe(&mut self, report: InstabilityReport) {
        let disrupted = report.instability_score > 0.6;
        self.matcher.record(HistoricalShot {
            shot: report.shot,
            report: report.clone(),
            disrupted,
        });
        self.latest = Some(report);
    }

    /// Suggest control-parameter adjustments for the next shot given the
    /// latest report. Returns human-readable text plus a structured parameter
    /// delta an agent/controller can apply.
    pub fn suggest(&self) -> Suggestion {
        let Some(r) = &self.latest else {
            return Suggestion {
                alert: false,
                message: "No plasma state observed yet.".into(),
                params: json!({}),
            };
        };
        if r.instability_score > 0.6 {
            Suggestion {
                alert: true,
                message: format!(
                    "High disruption risk (score={:.2}, f={:.1} Hz): reduce plasma current and increase heating to broaden the profile.",
                    r.instability_score, r.dominant_freq_hz
                ),
                params: json!({
                    "plasma_current_ma_delta": -1.5,
                    "heating_power_mw_delta": 2.0,
                    "action": "ramp_down_current"
                }),
            }
        } else if r.instability_score > 0.3 {
            Suggestion {
                alert: false,
                message: format!(
                    "Moderate MHD activity (score={:.2}): hold current, monitor {:.1} Hz mode.",
                    r.instability_score, r.dominant_freq_hz
                ),
                params: json!({ "plasma_current_ma_delta": 0.0, "action": "hold" }),
            }
        } else {
            Suggestion {
                alert: false,
                message: "Stable discharge: no adjustment required.".into(),
                params: json!({ "action": "none" }),
            }
        }
    }
}

/// A parameter suggestion produced by the oracle.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub alert: bool,
    pub message: String,
    pub params: serde_json::Value,
}

/// A minimal Model Context Protocol server (JSON-RPC 2.0 over stdio).
#[derive(Default)]
pub struct McpServer {
    pub state: OracleState,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            state: OracleState::new(),
        }
    }

    /// Handle one JSON-RPC request and produce a response object (or `None`
    /// for notifications that need no reply).
    pub fn handle(&self, req: &serde_json::Value) -> Option<serde_json::Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let rpc = "2.0";

        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "tpt-oracle",
                    "version": "0.1.0"
                }
            }),
            "tools/list" => json!({
                "tools": [
                    {
                        "name": "query_plasma_state",
                        "description": "Return the latest plasma instability report and nearest historical matches.",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "suggest_parameters",
                        "description": "Suggest control-parameter adjustments for the next shot based on the latest state.",
                        "inputSchema": { "type": "object", "properties": {} }
                    }
                ]
            }),
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(json!({}));
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                match self.call_tool(name) {
                    Ok(content) => json!({
                        "content": [{ "type": "text", "text": content }]
                    }),
                    Err(e) => {
                        return Some(json!({
                            "jsonrpc": rpc,
                            "id": id,
                            "error": {
                                "code": -32602,
                                "message": e
                            }
                        }));
                    }
                }
            }
            _ => {
                return Some(json!({
                    "jsonrpc": rpc,
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("method not found: {method}")
                    }
                }));
            }
        };

        Some(json!({ "jsonrpc": rpc, "id": id, "result": result }))
    }

    fn call_tool(&self, name: &str) -> Result<String, String> {
        match name {
            "query_plasma_state" => {
                let Some(r) = &self.state.latest else {
                    return Ok("No plasma state observed yet.".to_string());
                };
                let near = nearest_by_embedding(&self.state.matcher.history, r, 3);
                let matches: Vec<String> = near
                    .iter()
                    .map(|(h, s)| {
                        format!("shot {} (sim={:.3}, disrupted={})", h.shot, s, h.disrupted)
                    })
                    .collect();
                Ok(format!(
                    "shot {}: f={:.2}Hz score={:.3}\nnearest: {}",
                    r.shot,
                    r.dominant_freq_hz,
                    r.instability_score,
                    matches.join("; ")
                ))
            }
            "suggest_parameters" => Ok(self.state.suggest().message),
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_fluxstream::detection::InstabilityReport;

    fn rep(shot: u64, f: f64, mag: f64, score: f64) -> InstabilityReport {
        InstabilityReport {
            shot,
            dominant_freq_hz: f,
            dominant_magnitude: mag,
            instability_score: score,
        }
    }

    #[test]
    fn nearest_finds_closest_disruption() {
        let mut m = StateMatcher::new();
        m.record(HistoricalShot {
            shot: 1,
            report: rep(1, 50.0, 10.0, 0.2),
            disrupted: false,
        });
        m.record(HistoricalShot {
            shot: 2,
            report: rep(2, 51.0, 40.0, 0.9),
            disrupted: true,
        });
        let q = rep(99, 51.2, 38.0, 0.88);
        let near = m.nearest(&q, 1);
        assert_eq!(near[0].shot, 2);
        assert!(near[0].disrupted);
    }

    #[test]
    fn embedding_cosine_is_symmetric_and_bounded() {
        let a = embed(&rep(1, 50.0, 10.0, 0.2));
        let b = embed(&rep(2, 51.0, 40.0, 0.9));
        let sim = cosine_similarity(&a, &b);
        assert!((sim - cosine_similarity(&b, &a)).abs() < 1e-12);
        assert!((-1.0..=1.0).contains(&sim));
    }

    #[test]
    fn nearest_by_embedding_prefers_similar_disruption() {
        let hist = vec![
            HistoricalShot {
                shot: 1,
                report: rep(1, 50.0, 10.0, 0.2),
                disrupted: false,
            },
            HistoricalShot {
                shot: 2,
                report: rep(2, 51.0, 40.0, 0.9),
                disrupted: true,
            },
        ];
        let near = nearest_by_embedding(&hist, &rep(99, 51.2, 38.0, 0.88), 1);
        assert_eq!(near[0].0.shot, 2);
    }

    #[test]
    fn json_ld_contains_shot_and_score() {
        let ld = to_json_ld(&rep(7, 51.0, 30.0, 0.4));
        let s = serde_json::to_string(&ld).unwrap();
        assert!(s.contains("7"));
        assert!(s.contains("0.4"));
    }

    #[test]
    fn mcp_initialize_and_tools_list() {
        let srv = McpServer::new();
        let init = srv.handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize"}));
        assert_eq!(init.unwrap()["result"]["serverInfo"]["name"], "tpt-oracle");

        let list = srv.handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        let tools = &list.unwrap()["result"]["tools"];
        assert_eq!(tools.as_array().unwrap().len(), 2);
    }

    #[test]
    fn mcp_query_and_suggest_tools() {
        let mut srv = McpServer::new();
        srv.state.observe(rep(100, 51.0, 40.0, 0.8));

        let q = srv.handle(&json!({
            "jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"query_plasma_state","arguments":{}}
        }));
        let text = &q.unwrap()["result"]["content"][0]["text"];
        assert!(text.as_str().unwrap().contains("shot 100"));

        let s = srv.handle(&json!({
            "jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":"suggest_parameters","arguments":{}}
        }));
        let text = &s.unwrap()["result"]["content"][0]["text"];
        assert!(text.as_str().unwrap().contains("High disruption risk"));
    }

    #[test]
    fn mcp_unknown_method_errors() {
        let srv = McpServer::new();
        let r = srv.handle(&json!({"jsonrpc":"2.0","id":5,"method":"bogus"}));
        assert_eq!(r.unwrap()["error"]["code"], -32601);
    }
}
