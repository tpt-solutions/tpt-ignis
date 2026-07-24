//! tpt-halo — Universal Visualization & Control client (Phase 2+).
//!
//! Renders the tokamak reactor as an `appfront-core` `UITree` (target-agnostic:
//! the same tree drives the native egui/canvas shell, the WASM browser canvas,
//! and the terminal/TUI backend). The geometry is taken from
//! [`tpt_physics_math`]'s [`TokamakEquilibrium`]; live diagnostics come from a
//! [`LiveState`] and are drawn as ASCII poloidal/toroidal projections plus
//! gauge widgets from [`tpt_ui_components`].

use tpt_appfront_core::UITree;
use tpt_physics_math::equilibrium::TokamakEquilibrium;
use tpt_ui_components::{gauge_widget, toroidal_field_gauge, PlasmaGauge};

/// Live reactor readouts fed to the renderer each frame.
#[derive(Debug, Clone)]
pub struct LiveState {
    pub shot: u64,
    /// Plasma current, MA.
    pub ip_ma: f64,
    /// Toroidal field on axis, T.
    pub bt_t: f64,
    /// Volume-averaged plasma beta.
    pub beta: f64,
    /// Dominant instability frequency from the FFT oracle, Hz.
    pub dominant_freq_hz: f64,
    /// Instability score in `[0, 1]`.
    pub instability_score: f64,
    /// Normalised thermal load in `[0, 1]` (overlay from tpt-aether).
    pub thermal_load: f64,
}

impl LiveState {
    pub fn example() -> Self {
        Self {
            shot: 4242,
            ip_ma: 15.0,
            bt_t: 5.3,
            beta: 0.028,
            dominant_freq_hz: 51.0,
            instability_score: 0.22,
            thermal_load: 0.35,
        }
    }
}

/// Rasterise the poloidal (R–Z) cross-section of `eq` into a character grid,
/// drawing the plasma boundary outline. Points inside the boundary are filled
/// with `.`, the outline with `#`, else space.
pub fn poloidal_grid(eq: &TokamakEquilibrium, rows: usize, cols: usize) -> Vec<Vec<char>> {
    let r_lo = eq.r0.value() - eq.a.value() * 1.6;
    let r_hi = eq.r0.value() + eq.a.value() * 1.6;
    let z_ext = eq.kappa * eq.a.value() * 1.6;
    let z_lo = -z_ext;
    let z_hi = z_ext;
    let d_r = (r_hi - r_lo) / (cols as f64).max(1.0);
    let d_z = (z_hi - z_lo) / (rows as f64).max(1.0);

    // Sample the boundary densely and mark grid cells whose centre is within
    // half a cell of a boundary point, or inside the surface.
    let boundary: Vec<(f64, f64)> = eq.boundary(400);
    let mut grid = vec![vec![' '; cols]; rows];
    for (i, row) in grid.iter_mut().enumerate() {
        let z = z_hi - (i as f64 + 0.5) * d_z;
        for (j, cell) in row.iter_mut().enumerate() {
            let r = r_lo + (j as f64 + 0.5) * d_r;
            // Inside test: solve Miller outline for given (R, Z) is awkward, so
            // approximate using the boundary polygon (point-in-polygon).
            let inside = point_in_polygon(r, z, &boundary);
            let near = boundary
                .iter()
                .any(|(br, bz)| (br - r).abs() < d_r * 0.6 && (bz - z).abs() < d_z * 0.6);
            *cell = if near {
                '#'
            } else if inside {
                '.'
            } else {
                ' '
            };
        }
    }
    grid
}

/// Rasterise a toroidal (donut) projection of the device: an annulus of outer
/// radius `R0 + a` and inner radius `R0 - a` in an isotropic grid.
pub fn toroidal_grid(eq: &TokamakEquilibrium, rows: usize, cols: usize) -> Vec<Vec<char>> {
    let r_out = eq.r0.value() + eq.a.value();
    let r_in = (eq.r0.value() - eq.a.value()).max(0.01);
    let mut grid = vec![vec![' '; cols]; rows];
    for (i, row) in grid.iter_mut().enumerate() {
        let y = ((rows as f64) / 2.0) - (i as f64 + 0.5);
        for (j, cell) in row.iter_mut().enumerate() {
            let x = (j as f64 + 0.5) - ((cols as f64) / 2.0);
            let d = (x * x + y * y).sqrt();
            let near_out = (d - r_out).abs() < 1.0;
            let near_in = (d - r_in).abs() < 1.0;
            *cell = if near_out || near_in { '#' } else { ' ' };
        }
    }
    grid
}

fn point_in_polygon(x: f64, y: f64, poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn grid_to_text(grid: &[Vec<char>]) -> String {
    grid.iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the full reactor `UITree` for `eq` + `state`.
pub fn render_ui(eq: &TokamakEquilibrium, state: &LiveState) -> UITree<()> {
    let poloidal = poloidal_grid(eq, 24, 56);
    let toroidal = toroidal_grid(eq, 24, 56);

    let (axis_r, axis_z) = eq.magnetic_axis();
    let q95 = eq.safety_factor(0.95, 1.0);

    UITree::container(|c| {
        c.heading(1, "TPT Halo — Reactor View")
            .ai_action("read_overview")
            .ai_description("Live tokamak reactor overview");

        c.heading(2, format!("Shot #{} — live diagnostics", state.shot));
        c.container(|r| {
            r.with(gauge_widget(
                "Toroidal Field (T)",
                &toroidal_field_gauge(),
                state.bt_t,
            ));
            r.with(gauge_widget(
                "Plasma Current (MA)",
                &PlasmaGauge::new(0.0, 20.0, 10.0, 18.0),
                state.ip_ma,
            ));
            r.with(gauge_widget(
                "Instability Score",
                &PlasmaGauge::new(0.0, 1.0, 0.0, 0.6),
                state.instability_score,
            ));
            r.with(gauge_widget(
                "Thermal Load",
                &PlasmaGauge::new(0.0, 1.0, 0.0, 0.8),
                state.thermal_load,
            ));
        });

        c.heading(
            3,
            format!(
                "Equilibrium: R0={:.2}m a={:.2}m kappa={:.2} q95={:.2} axis=(R={:.2}, Z={:.2})",
                eq.r0.value(),
                eq.a.value(),
                eq.kappa,
                q95,
                axis_r,
                axis_z,
            ),
        );

        c.heading(3, "Poloidal cross-section (R–Z)");
        c.text(grid_to_text(&poloidal))
            .ai_action("read_poloidal")
            .ai_description("Poloidal cross-section of the plasma boundary");

        c.heading(3, "Toroidal projection");
        c.text(grid_to_text(&toroidal))
            .ai_action("read_toroidal")
            .ai_description("Toroidal projection of the reactor");
    })
}

/// Render the reactor view to a self-contained HTML snippet (offline check).
pub fn render_html(eq: &TokamakEquilibrium, state: &LiveState) -> String {
    let mut ui = render_ui(eq, state);
    ui.assign_ids();
    let agent_state = tpt_appfront_core::query_state(&ui);
    let report = tpt_appfront_core::render(&ui, &agent_state);
    tpt_appfront_core::to_html(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq() -> TokamakEquilibrium {
        TokamakEquilibrium::shaped(6.2, 2.0, 5.3, 15.0, 1.7, 0.33, 0.2)
    }

    #[test]
    fn poloidal_grid_has_boundary_and_interior() {
        let g = poloidal_grid(&eq(), 24, 56);
        let flat: String = g.iter().flatten().collect();
        assert!(flat.contains('#'), "boundary outline drawn");
        assert!(flat.contains('.'), "plasma interior filled");
    }

    #[test]
    fn toroidal_grid_is_a_ring() {
        let g = toroidal_grid(&eq(), 24, 56);
        let flat: String = g.iter().flatten().collect();
        // A ring must have outline cells but an empty hole in the centre.
        assert!(flat.contains('#'));
        assert!(!g[g.len() / 2].iter().all(|c| *c == '#'));
    }

    #[test]
    fn render_ui_contains_diagnostics_and_views() {
        let ui = render_ui(&eq(), &LiveState::example());
        let html = render_html(&eq(), &LiveState::example());
        // Live shot + readouts present.
        assert!(html.contains("Shot #4242"));
        assert!(html.contains("Toroidal Field"));
        assert!(html.contains("Poloidal cross-section"));
        assert!(html.contains("Toroidal projection"));
        // The tree structure is intact.
        let _ = ui;
    }

    #[test]
    fn point_in_polygon_classifies_centre() {
        let poly = vec![(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
        assert!(point_in_polygon(0.0, 0.0, &poly));
        assert!(!point_in_polygon(5.0, 5.0, &poly));
    }

    #[test]
    fn integration_overlays_aether_thermal_load() {
        // Phase 3 integration: tpt-aether computes the geospatial thermal load
        // (with a cooling-pump failure cascade), and tpt-halo overlays the
        // resulting scalar onto its live reactor view.
        use tpt_aether::ReactorGraph;
        let g = ReactorGraph::mock_reactor();
        let load_nominal = g.thermal_load((0.0, 0.0, 0.0), None);
        let load_failed = g.thermal_load((0.0, 0.0, 0.0), Some("pump_a"));

        let mean = |m: &std::collections::HashMap<String, f64>| {
            m.values().sum::<f64>() / m.len().max(1) as f64
        };

        let mut state = LiveState::example();
        state.thermal_load = mean(&load_failed);
        let html_failed = render_html(&eq(), &state);

        state.thermal_load = mean(&load_nominal);
        let html_nominal = render_html(&eq(), &state);

        assert!(html_failed.contains("Thermal Load"));
        // A failed pump pushes the mean thermal load above the nominal case.
        assert!(mean(&load_failed) > mean(&load_nominal));
        assert!(html_failed.len() > html_nominal.len() - 1);
    }
}
