use anyhow::{Result, bail};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// An identity unique within a document. Sibling order is independent of IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(pub u64);

/// Immutable typed content with explicit child ownership.
#[derive(Clone)]
pub struct Node {
    pub(crate) id: NodeId,
    pub(crate) payload: Arc<dyn Any + Send + Sync>,
    pub(crate) children: Vec<NodeId>,
}

impl Node {
    pub fn new<T: Any + Send + Sync>(id: NodeId, payload: T, children: Vec<NodeId>) -> Self {
        Self {
            id,
            payload: Arc::new(payload),
            children,
        }
    }
}

/// A validated forest with ordered roots and no unreachable nodes.
#[derive(Clone)]
pub struct Document {
    pub(crate) nodes: Arc<HashMap<NodeId, Node>>,
    pub(crate) roots: Arc<[NodeId]>,
}

impl Document {
    /// Validate ownership and bound nesting before accepting a snapshot.
    pub fn new(nodes: impl IntoIterator<Item = Node>, roots: Vec<NodeId>) -> Result<Self> {
        let mut store = HashMap::new();
        for node in nodes {
            let id = node.id;
            if store.insert(id, node).is_some() {
                bail!("duplicate node {id:?}");
            }
            if store.len() > 100_000 {
                bail!("document exceeds 100000 nodes");
            }
        }
        let mut visited = HashSet::new();
        let mut pending: Vec<_> = roots.iter().map(|id| (*id, 0)).collect();
        while let Some((id, depth)) = pending.pop() {
            if depth > 256 {
                bail!("document exceeds 256 nesting levels");
            }
            if !visited.insert(id) {
                bail!("node {id:?} has multiple owners or forms a cycle");
            }
            let Some(node) = store.get(&id) else {
                bail!("missing node {id:?}");
            };
            pending.extend(node.children.iter().map(|child| (*child, depth + 1)));
        }
        if visited.len() != store.len() {
            bail!("document contains unreachable nodes");
        }
        Ok(Self {
            nodes: Arc::new(store),
            roots: roots.into(),
        })
    }
}
