//! Target-agnostic UI widget tree and devtools renderer.
//!
//! Stub implementation kept within `tpt-ignis` until the canonical crate is
//! published in `tpt-appfront`. Provides the `UITree<T>` builder API, AI
//! metadata annotations, and an HTML devtools renderer used for offline tests.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Meta {
    ai_action: Option<String>,
    ai_params: Vec<(String, String)>,
    ai_description: Option<String>,
    class: Option<String>,
    id: Option<String>,
}

#[derive(Clone)]
enum NodeInner {
    Heading {
        level: u32,
        text: String,
    },
    Text {
        content: String,
    },
    DataGrid {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Container {
        children: Vec<TreeNode>,
    },
}

#[derive(Clone)]
struct TreeNode {
    inner: NodeInner,
    meta: Rc<RefCell<Meta>>,
}

/// A target-agnostic UI widget tree.
///
/// `T` is a phantom type parameter for app-state metadata (all widgets in this
/// workspace use `T = ()`).
#[derive(Clone)]
pub struct UITree<T> {
    nodes: Vec<TreeNode>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> UITree<T> {
    /// Build a tree from a closure that populates a [`NodeBuilder`].
    pub fn container(f: impl FnOnce(&mut NodeBuilder)) -> Self {
        let mut b = NodeBuilder { nodes: Vec::new() };
        f(&mut b);
        Self {
            nodes: b.nodes,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Stamp every node in the tree with a unique id string.
    pub fn assign_ids(&mut self) {
        assign_ids_rec(&mut self.nodes, &mut 0);
    }

    /// Collect all human-readable text in depth-first order.
    pub fn text_content(&self) -> String {
        collect_text(&self.nodes)
    }
}

fn assign_ids_rec(nodes: &mut [TreeNode], counter: &mut usize) {
    for node in nodes.iter_mut() {
        node.meta.borrow_mut().id = Some(format!("n{}", *counter));
        *counter += 1;
        if let NodeInner::Container { children } = &mut node.inner {
            assign_ids_rec(children, counter);
        }
    }
}

fn collect_text(nodes: &[TreeNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        match &node.inner {
            NodeInner::Heading { text, .. } => {
                out.push_str(text);
                out.push('\n');
            }
            NodeInner::Text { content } => {
                out.push_str(content);
                out.push('\n');
            }
            NodeInner::DataGrid { headers, rows } => {
                for h in headers {
                    out.push_str(h);
                    out.push(' ');
                }
                out.push('\n');
                for row in rows {
                    for cell in row {
                        out.push_str(cell);
                        out.push(' ');
                    }
                    out.push('\n');
                }
            }
            NodeInner::Container { children } => {
                out.push_str(&collect_text(children));
            }
        }
    }
    out
}

fn render_nodes(nodes: &[TreeNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        let meta = node.meta.borrow();
        let id_attr = meta
            .id
            .as_deref()
            .map(|id| format!(" id=\"{id}\""))
            .unwrap_or_default();
        let class_attr = meta
            .class
            .as_deref()
            .map(|c| format!(" class=\"{c}\""))
            .unwrap_or_default();
        match &node.inner {
            NodeInner::Heading { level, text } => {
                out.push_str(&format!("<h{level}{id_attr}>{}</h{level}>", esc(text)));
            }
            NodeInner::Text { content } => {
                out.push_str(&format!("<pre{id_attr}{class_attr}>{}</pre>", esc(content)));
            }
            NodeInner::DataGrid { headers, rows } => {
                out.push_str(&format!(
                    "<table{id_attr}{class_attr} data-node-type=\"DataGrid\"><thead><tr>"
                ));
                for h in headers {
                    out.push_str(&format!("<th>{}</th>", esc(h)));
                }
                out.push_str("</tr></thead><tbody>");
                for row in rows {
                    out.push_str("<tr>");
                    for cell in row {
                        out.push_str(&format!("<td>{}</td>", esc(cell)));
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</tbody></table>");
            }
            NodeInner::Container { children } => {
                out.push_str(&format!("<div{id_attr}{class_attr}>"));
                out.push_str(&render_nodes(children));
                out.push_str("</div>");
            }
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Accumulates nodes inside a [`UITree::container`] closure.
pub struct NodeBuilder {
    nodes: Vec<TreeNode>,
}

impl NodeBuilder {
    pub fn heading(&mut self, level: u32, text: impl ToString) -> NodeRef {
        let meta = Rc::new(RefCell::new(Meta::default()));
        self.nodes.push(TreeNode {
            inner: NodeInner::Heading {
                level,
                text: text.to_string(),
            },
            meta: Rc::clone(&meta),
        });
        NodeRef(meta)
    }

    pub fn text(&mut self, content: impl ToString) -> NodeRef {
        let meta = Rc::new(RefCell::new(Meta::default()));
        self.nodes.push(TreeNode {
            inner: NodeInner::Text {
                content: content.to_string(),
            },
            meta: Rc::clone(&meta),
        });
        NodeRef(meta)
    }

    pub fn data_grid<S: ToString>(
        &mut self,
        headers: impl IntoIterator<Item = S>,
        rows: Vec<Vec<String>>,
    ) -> NodeRef {
        let meta = Rc::new(RefCell::new(Meta::default()));
        let headers = headers.into_iter().map(|h| h.to_string()).collect();
        self.nodes.push(TreeNode {
            inner: NodeInner::DataGrid { headers, rows },
            meta: Rc::clone(&meta),
        });
        NodeRef(meta)
    }

    pub fn container(&mut self, f: impl FnOnce(&mut NodeBuilder)) -> NodeRef {
        let meta = Rc::new(RefCell::new(Meta::default()));
        let mut inner = NodeBuilder { nodes: Vec::new() };
        f(&mut inner);
        self.nodes.push(TreeNode {
            inner: NodeInner::Container {
                children: inner.nodes,
            },
            meta: Rc::clone(&meta),
        });
        NodeRef(meta)
    }

    /// Splice the top-level nodes of `tree` as a container child.
    pub fn with<U>(&mut self, tree: UITree<U>) -> NodeRef {
        let meta = Rc::new(RefCell::new(Meta::default()));
        self.nodes.push(TreeNode {
            inner: NodeInner::Container {
                children: tree.nodes,
            },
            meta: Rc::clone(&meta),
        });
        NodeRef(meta)
    }
}

/// A handle to a freshly-added node; supports AI metadata annotation chaining.
pub struct NodeRef(Rc<RefCell<Meta>>);

impl NodeRef {
    pub fn ai_action(&mut self, action: &str) -> &mut Self {
        self.0.borrow_mut().ai_action = Some(action.to_string());
        self
    }

    pub fn ai_param(&mut self, key: &str, val: impl ToString) -> &mut Self {
        self.0
            .borrow_mut()
            .ai_params
            .push((key.to_string(), val.to_string()));
        self
    }

    pub fn ai_description(&mut self, desc: impl ToString) -> &mut Self {
        self.0.borrow_mut().ai_description = Some(desc.to_string());
        self
    }

    pub fn class(&mut self, c: &str) -> &mut Self {
        self.0.borrow_mut().class = Some(c.to_string());
        self
    }
}

pub mod agent {
    use super::UITree;

    pub struct State {
        pub(super) _content: String,
    }

    pub fn query_state<T>(tree: &UITree<T>) -> State {
        State {
            _content: tree.text_content(),
        }
    }
}

pub mod devtools {
    use super::{agent, render_nodes, UITree};

    pub struct Report {
        html: String,
    }

    pub fn render<T>(tree: &UITree<T>, _state: &agent::State) -> Report {
        Report {
            html: render_nodes(&tree.nodes),
        }
    }

    pub fn to_html(report: &Report) -> String {
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{}</body></html>",
            report.html
        )
    }
}
