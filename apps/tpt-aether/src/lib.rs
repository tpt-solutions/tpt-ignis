//! tpt-aether — Digital Twin & Reactor Topology Graph (Phase 3).
//!
//! Models the physical and logical topology of the reactor facility as a
//! directed cooling-dependency graph with geospatial (3D) component positions,
//! then maps live neutron/thermal flux onto that geometry. The in-memory graph
//! is the offline-testable core; [`ReactorGraph::persist`] and
//! [`ReactorGraph::dependents_via_db`] mirror the model into `tpt-keystone-db`'s
//! Plexus (graph) and Meridian (geospatial) engines via the Postgres wire
//! protocol, exercised by `#[ignore]`d integration tests against a live node.

use std::collections::{HashMap, HashSet};

use tpt_sdk::keystone::{KeystoneClient, Value};

/// A physical component in the reactor, with a cooling-dependency list and a
/// geospatial position `(x, y, z)` in metres (vessel frame; origin = magnetic
/// axis).
#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub kind: String,
    /// Ids of components this one depends on for cooling (directed edges).
    pub cools_from: Vec<String>,
    pub position: (f64, f64, f64),
}

impl Component {
    pub fn new(id: &str, kind: &str, position: (f64, f64, f64)) -> Self {
        Self {
            id: id.to_string(),
            kind: kind.to_string(),
            cools_from: Vec::new(),
            position,
        }
    }

    pub fn cools_from(mut self, deps: &[&str]) -> Self {
        self.cools_from = deps.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// An in-memory reactor topology graph (directed cooling-dependency edges).
#[derive(Debug, Default)]
pub struct ReactorGraph {
    pub nodes: HashMap<String, Component>,
    pub edges: Vec<(String, String)>,
}

impl ReactorGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a component and register its cooling edges.
    pub fn add(&mut self, c: Component) {
        for dep in &c.cools_from {
            self.edges.push((dep.clone(), c.id.clone()));
        }
        self.nodes.insert(c.id.clone(), c);
    }

    /// Breadth-first upstream reachability: every component that ultimately
    /// depends on `start` for cooling. Answers "if `start` fails, who
    /// overheats?".
    pub fn dependents_of(&self, start: &str) -> HashSet<String> {
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for (a, b) in &self.edges {
            adj.entry(a.as_str()).or_default().push(b.as_str());
        }
        let mut seen = HashSet::new();
        let mut queue = vec![start];
        while let Some(node) = queue.pop() {
            if let Some(nexts) = adj.get(node) {
                for n in nexts {
                    if seen.insert((*n).to_string()) {
                        queue.push(n);
                    }
                }
            }
        }
        seen
    }

    /// Map 14.1 MeV neutron flux from a plasma source at `source` onto every
    /// component, using an inverse-square law `phi = P / (4*pi*r^2)`. Returns
    /// per-component flux (arbitrary units, normalised so the nearest surface
    /// is `1.0`). This is the geospatial neutron/thermal-load view that
    /// tpt-halo overlays onto the 3D model.
    pub fn neutron_flux(&self, source: (f64, f64, f64), power: f64) -> HashMap<String, f64> {
        let mut out = HashMap::new();
        let mut max = 0.0_f64;
        for (id, c) in &self.nodes {
            let (x, y, z) = c.position;
            let r2 = (x - source.0).powi(2) + (y - source.1).powi(2) + (z - source.2).powi(2);
            let flux = power / (4.0 * std::f64::consts::PI * r2.max(1e-6));
            max = max.max(flux);
            out.insert(id.clone(), flux);
        }
        if max > 0.0 {
            for v in out.values_mut() {
                *v /= max;
            }
        }
        out
    }

    /// Normalised thermal load per component (used by tpt-halo's overlay):
    /// combines neutron flux with whether the component sits in a cooling
    /// failure cascade rooted at `failed`.
    pub fn thermal_load(
        &self,
        source: (f64, f64, f64),
        failed: Option<&str>,
    ) -> HashMap<String, f64> {
        let flux = self.neutron_flux(source, 1.0);
        let cascade = failed.map(|f| self.dependents_of(f));
        flux.into_iter()
            .map(|(id, f)| {
                let heat = if let Some(set) = &cascade {
                    if set.contains(&id) {
                        // Starved of cooling: flux builds up (x3).
                        f * 3.0
                    } else {
                        f
                    }
                } else {
                    f
                };
                (id, heat.min(1.0))
            })
            .collect()
    }

    /// Build a MAST-U/ITER-class mock reactor: two coolant pumps feeding HTS
    /// magnet segments and a divertor, all wrapped by the vacuum vessel and a
    /// cryoplant. Positions are illustrative (metres, vessel frame).
    pub fn mock_reactor() -> Self {
        let mut g = ReactorGraph::new();
        g.add(Component::new("cryoplant", "cryoplant", (0.0, 6.0, 0.0)));
        g.add(Component::new("pump_a", "coolant_pump", (3.0, 4.0, 0.0)).cools_from(&["cryoplant"]));
        g.add(
            Component::new("pump_b", "coolant_pump", (-3.0, 4.0, 0.0)).cools_from(&["cryoplant"]),
        );
        g.add(Component::new("hts_seg_1", "hts_magnet", (2.0, 1.0, 0.0)).cools_from(&["pump_a"]));
        g.add(Component::new("hts_seg_2", "hts_magnet", (-2.0, 1.0, 0.0)).cools_from(&["pump_b"]));
        g.add(
            Component::new("hts_seg_3", "hts_magnet", (0.0, -2.0, 0.0))
                .cools_from(&["pump_a", "pump_b"]),
        );
        g.add(Component::new("divertor", "divertor", (0.0, -1.0, 0.0)).cools_from(&["hts_seg_3"]));
        g.add(Component::new("vessel", "vacuum_vessel", (0.0, 0.0, 0.0)));
        g
    }

    // ---- `tpt-keystone-db` persistence (graph + geospatial engines) ----

    /// DDL mirroring this graph into `tpt-keystone-db`: a Plexus-style edge
    /// table for cooling dependencies and a Meridian-style point table for
    /// geospatial component positions.
    pub fn ddl() -> Vec<&'static str> {
        vec![
            "CREATE TABLE reactor_component (
                id   text PRIMARY KEY,
                kind text,
                x    double precision,
                y    double precision,
                z    double precision
            )",
            "CREATE TABLE reactor_cooling_edge (
                from_id text,
                to_id   text
            )",
        ]
    }

    /// Persist the current graph into a live `tpt-keystone-db` node.
    pub async fn persist(
        &self,
        client: &mut KeystoneClient,
    ) -> Result<u64, tpt_sdk::keystone::KeystoneError> {
        for stmt in Self::ddl() {
            client.query(stmt).await?;
        }
        let component_rows: Vec<Vec<Value>> = self
            .nodes
            .values()
            .map(|c| {
                vec![
                    Value::Text(c.id.clone()),
                    Value::Text(c.kind.clone()),
                    Value::Float(c.position.0),
                    Value::Float(c.position.1),
                    Value::Float(c.position.2),
                ]
            })
            .collect();
        let edge_rows: Vec<Vec<Value>> = self
            .edges
            .iter()
            .map(|(a, b)| vec![Value::Text(a.clone()), Value::Text(b.clone())])
            .collect();

        let n1 = client
            .copy_in(
                "reactor_component",
                &["id", "kind", "x", "y", "z"],
                &component_rows,
            )
            .await?;
        let n2 = client
            .copy_in("reactor_cooling_edge", &["from_id", "to_id"], &edge_rows)
            .await?;
        Ok(n1 + n2)
    }

    /// Answer "if `start` fails, who overheats?" via a recursive CTE on the
    /// persisted edge table (mirrors [`ReactorGraph::dependents_of`]).
    pub async fn dependents_via_db(
        &self,
        client: &mut KeystoneClient,
        start: &str,
    ) -> Result<HashSet<String>, tpt_sdk::keystone::KeystoneError> {
        let sql = format!(
            "WITH RECURSIVE dep(to_id) AS (
                SELECT to_id FROM reactor_cooling_edge WHERE from_id = '{start}'
                UNION ALL
                SELECT e.to_id FROM reactor_cooling_edge e
                JOIN dep d ON e.from_id = d.to_id
            ) SELECT to_id FROM dep"
        );
        let res = client.query(&sql).await?;
        Ok(res
            .rows
            .iter()
            .filter_map(|r| r.get_str(0).map(|s| s.to_string()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependents_trace_cooling_failure() {
        let mut g = ReactorGraph::new();
        g.add(Component::new("pump_a", "pump", (0.0, 0.0, 0.0)));
        g.add(
            Component::new("magnet_seg_1", "hts_magnet", (1.0, 0.0, 0.0)).cools_from(&["pump_a"]),
        );
        g.add(
            Component::new("magnet_seg_2", "hts_magnet", (1.0, 1.0, 0.0))
                .cools_from(&["magnet_seg_1"]),
        );
        let deps = g.dependents_of("pump_a");
        assert!(deps.contains("magnet_seg_1"));
        assert!(deps.contains("magnet_seg_2"));
    }

    #[test]
    fn mock_reactor_has_expected_topology() {
        let g = ReactorGraph::mock_reactor();
        assert_eq!(g.nodes.len(), 8);
        // Cryoplant feeds both pumps; both pumps feed hts_seg_3.
        let from_cryo = g.dependents_of("cryoplant");
        assert!(from_cryo.contains("pump_a"));
        assert!(from_cryo.contains("pump_b"));
        assert!(from_cryo.contains("hts_seg_3"));
        assert!(from_cryo.contains("divertor"));
    }

    #[test]
    fn neutron_flux_is_inverse_square_and_normalised() {
        let g = ReactorGraph::mock_reactor();
        let flux = g.neutron_flux((0.0, 0.0, 0.0), 1.0);
        // The vessel at the origin is the nearest surface -> normalised to 1.0.
        assert!((flux["vessel"] - 1.0).abs() < 1e-9);
        // A distant pump sees strictly less flux.
        assert!(flux["pump_a"] < flux["vessel"]);
    }

    #[test]
    fn thermal_load_amplifies_cooling_cascade() {
        let g = ReactorGraph::mock_reactor();
        let baseline = g.thermal_load((0.0, 0.0, 0.0), None);
        let failed = g.thermal_load((0.0, 0.0, 0.0), Some("pump_a"));
        // hts_seg_1 depends on the failed pump -> load amplified.
        assert!(failed["hts_seg_1"] > baseline["hts_seg_1"]);
        // Nothing exceeds 1.0 (clamped).
        assert!(failed.values().all(|&v| v <= 1.0));
    }
}
