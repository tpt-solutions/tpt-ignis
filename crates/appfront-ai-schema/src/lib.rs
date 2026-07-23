//! JSON-LD serialisation for `appfront-core` UI trees.
//!
//! Stub implementation kept within `tpt-ignis` until the canonical crate is
//! published in `tpt-appfront`.

use appfront_core::UITree;
use serde_json::{json, Value};

/// Serialise `tree` as a JSON-LD `Dataset` object.
///
/// The `description` field contains the tree's full text content so that
/// external agents can locate headings and metric values by substring search.
pub fn to_json_ld<T>(tree: &UITree<T>) -> Value {
    json!({
        "@context": "https://schema.org/",
        "@type": "Dataset",
        "description": tree.text_content()
    })
}
