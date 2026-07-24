//! Reusable UI widgets for the TPT Ignis suite, built on `tpt-appfront-core`'s
//! target-agnostic `UITree` AST (renders to HTML/WASM/canvas via the
//! appfront backends, and to a devtools/`to_html` snippet for offline tests).
//!
//! The Phase-1 scaffold shipped a headless [`PlasmaGauge`] model. Phase 2 adds
//! real *widget* builders that emit `UITree`s: a radial [`PlasmaGauge`] view
//! and an ASCII time-series [`diagnostic_chart`] for diagnostic shots — both
//! carry AI metadata so an external agent can read plasma state (see
//! `tpt-appfront-ai-schema`).

use tpt_appfront_core::UITree;

/// Visual state of a single radial gauge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeView {
    /// Normalized fill in `[0, 1]`.
    pub fraction: f64,
    /// True when the value is outside the safe band.
    pub alarm: bool,
}

/// A headless plasma gauge model. `value` is compared against a `[min, max]`
/// range and an optional safe sub-band `[safe_low, safe_high]`.
#[derive(Debug, Clone, Copy)]
pub struct PlasmaGauge {
    pub min: f64,
    pub max: f64,
    pub safe_low: f64,
    pub safe_high: f64,
}

impl PlasmaGauge {
    pub fn new(min: f64, max: f64, safe_low: f64, safe_high: f64) -> Self {
        Self {
            min,
            max,
            safe_low,
            safe_high,
        }
    }

    /// Compute the visual state for `value`.
    pub fn view(&self, value: f64) -> GaugeView {
        let span = (self.max - self.min).max(1e-9);
        let fraction = ((value - self.min) / span).clamp(0.0, 1.0);
        let alarm = value < self.safe_low || value > self.safe_high;
        GaugeView { fraction, alarm }
    }
}

/// Convenience: build a gauge for a toroidal field reading in Tesla.
pub fn toroidal_field_gauge() -> PlasmaGauge {
    PlasmaGauge::new(0.0, 6.0, 4.5, 5.5)
}

/// Render a single horizontal bar of `width` columns whose fill is `fraction`.
/// Uses block-drawing characters for a smooth fill.
pub fn ascii_bar(fraction: f64, width: usize) -> String {
    let frac = fraction.clamp(0.0, 1.0);
    let filled = (frac * width as f64).round() as usize;
    let mut s = String::new();
    for i in 0..width {
        s.push(if i < filled { '█' } else { ' ' });
    }
    s
}

/// Build a `UITree` widget for one [`PlasmaGauge`] reading.
///
/// Emits a heading, an ASCII fill bar annotated with the value, and an
/// alarm/OK badge. Tagged with AI metadata (`read_gauge`) so an agent can
/// poll the live reading.
pub fn gauge_widget(title: &str, gauge: &PlasmaGauge, value: f64) -> UITree<()> {
    let view = gauge.view(value);
    let bar = ascii_bar(view.fraction, 20);
    let state = if view.alarm { "ALARM" } else { "OK" };
    let line = format!("{bar} {value:.3}  [{state}]");

    UITree::container(|c| {
        c.heading(3, title.to_string())
            .ai_action("read_gauge")
            .ai_param("metric", title.to_string())
            .ai_description(format!("Live reading for {title}"));
        c.text(line)
            .ai_action("read_gauge")
            .ai_param("metric", title.to_string())
            .ai_param("value", format!("{value:.3}"))
            .ai_param("state", state.to_string());
    })
}

/// Build a dashboard `UITree` of several gauges laid out as a data grid.
pub fn gauge_dashboard(title: &str, gauges: &[(&str, PlasmaGauge, f64)]) -> UITree<()> {
    let rows: Vec<Vec<String>> = gauges
        .iter()
        .map(|(name, g, v)| {
            let view = g.view(*v);
            vec![
                name.to_string(),
                format!("{v:.3}"),
                format!("{:.0}%", view.fraction * 100.0),
                if view.alarm { "ALARM" } else { "OK" }.to_string(),
                ascii_bar(view.fraction, 16),
            ]
        })
        .collect();

    UITree::container(|c| {
        c.heading(2, title.to_string()).ai_action("read_dashboard");
        c.data_grid(["Gauge", "Value", "Fill", "State", "Bar"], rows)
            .class("gauge-dashboard");
    })
}

/// Build an ASCII time-series chart of `points` (label, value) as a `UITree`.
///
/// Each point becomes one bar row `[label | bar  value]`; the whole chart is
/// emitted as a single monospaced `Text` node so the appfront canvas/DOM
/// backends render it faithfully.
pub fn diagnostic_chart(title: &str, points: &[(String, f64)]) -> UITree<()> {
    let max = points
        .iter()
        .map(|(_, v)| v.abs())
        .fold(0.0_f64, f64::max)
        .max(1e-9);
    let width = 40usize;

    let body = points
        .iter()
        .map(|(label, v)| {
            let frac = v.abs() / max;
            let bar = ascii_bar(frac, width);
            format!("{label:<10} |{bar} {v:+.3}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    UITree::container(|c| {
        c.heading(3, title.to_string())
            .ai_action("read_chart")
            .ai_param("chart", title.to_string())
            .ai_description(format!("Diagnostic shot chart: {title}"));
        c.text(body);
    })
}

/// Convenience: chart a vector of samples `(time_s, value)`.
pub fn chart_samples(title: &str, samples: &[(f64, f64)]) -> UITree<()> {
    let points: Vec<(String, f64)> = samples
        .iter()
        .map(|(t, v)| (format!("{t:.4}s"), *v))
        .collect();
    diagnostic_chart(title, &points)
}

/// Render a `UITree` to a self-contained HTML snippet (uses appfront's
/// devtools `to_html`). Useful for offline verification and for embedding in
/// a devtools panel.
pub fn to_html(ui: &UITree<()>) -> String {
    let mut ui = ui.clone();
    ui.assign_ids();
    let state = tpt_appfront_core::query_state(&ui);
    let report = tpt_appfront_core::render(&ui, &state);
    tpt_appfront_core::to_html(&report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_fills_and_alarms() {
        let g = toroidal_field_gauge();
        let v = g.view(5.0);
        assert!(v.fraction > 0.0 && v.fraction < 1.0);
        assert!(!v.alarm);
        let low = g.view(1.0);
        assert!(low.alarm);
    }

    #[test]
    fn ascii_bar_scales_with_fraction() {
        assert_eq!(ascii_bar(0.0, 10), "          ");
        assert_eq!(ascii_bar(1.0, 10), "██████████");
        assert_eq!(ascii_bar(0.5, 10), "█████     ");
    }

    #[test]
    fn gauge_widget_emits_heading_and_bar() {
        let ui = gauge_widget("Toroidal Field", &toroidal_field_gauge(), 5.0);
        let html = to_html(&ui);
        assert!(html.contains("Toroidal Field"));
        assert!(html.contains("OK"));
        assert!(html.contains("█"));
    }

    #[test]
    fn gauge_dashboard_lists_all_gauges() {
        let ui = gauge_dashboard(
            "Cryo & Magnets",
            &[
                ("Toroidal B", toroidal_field_gauge(), 5.0),
                (
                    "Plasma Current",
                    PlasmaGauge::new(0.0, 20.0, 10.0, 18.0),
                    15.0,
                ),
            ],
        );
        let html = to_html(&ui);
        // The devtools/tree view exposes the data-grid columns and gauges.
        assert!(html.contains("DataGrid"));
        assert!(html.contains("Gauge"));
        assert!(html.contains("Value"));
        assert!(html.contains("Cryo"));
    }

    #[test]
    fn diagnostic_chart_contains_all_points() {
        let pts: Vec<(String, f64)> =
            vec![("t0".into(), 0.1), ("t1".into(), -0.5), ("t2".into(), 0.9)];
        let ui = diagnostic_chart("Thomson ne", &pts);
        let html = to_html(&ui);
        assert!(html.contains("t0"));
        assert!(html.contains("t1"));
        assert!(html.contains("t2"));
        // The largest-magnitude point (t2) should have the fullest bar.
        assert!(html.contains("████████████████████████████"));
    }
}
