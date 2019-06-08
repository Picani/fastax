use std::collections::{HashMap, HashSet};
use std::fmt;

use ansi_term::Style;

use Node;

/// A taxonomy tree
pub struct Tree {
    nodes: HashMap<i64, Node>,
    children: HashMap<i64, HashSet<i64>>,
    marked: HashSet<i64>
}

impl Tree {
    /// Create a new Tree containing the given nodes. The root is always
    /// the Node with ID 1.
    pub fn new(nodes: &Vec<Node>) -> Tree {
        let mut tree = Tree{
            nodes: HashMap::new(),
            children: HashMap::new(),
            marked: HashSet::new()
        };
        tree.add_nodes(nodes);
        tree
    }

    /// Add the given nodes to the Tree.
    pub fn add_nodes(&mut self, nodes: &Vec<Node>) {
        for node in nodes.iter() {
            if !self.nodes.contains_key(&node.tax_id) {
                self.nodes.insert(node.tax_id, node.clone());
            }

            if node.tax_id != node.parent_tax_id {
                self.children.entry(node.parent_tax_id)
                    .and_modify(|children| {children.insert(node.tax_id);})
                    .or_insert({
                        let mut set = HashSet::new();
                        set.insert(node.tax_id);
                        set
                    });
            }
        }
    }

    /// Mark the nodes with this IDs.
    pub fn mark_nodes(&mut self, taxids: &Vec<i64>) {
        for taxid in taxids.iter() {
            self.marked.insert(*taxid);
        }
    }

    /// Simplify the tree by removing all nodes that have only one child
    /// *and* are not marked.
    pub fn simplify(&mut self) {
        self.simplify_helper(1);
        self.children.retain(|_, v| !v.is_empty());
    }

    fn simplify_helper(&mut self, parent: i64) {
        let new_children = self.remove_single_child(parent);
        // TODO: remove that clone
        self.children.insert(parent, new_children.clone());
        // .unwrap() is safe here because new_children
        // is at least an empty set.
        for child in new_children.iter() {
            self.simplify_helper(*child);
        }
    }


    /// remove_single_child find the new children of a node by removing all
    /// unique child.
    fn remove_single_child(&self, parent: i64) -> HashSet<i64> {
        // nodes are the children of parent
        let mut new_children = HashSet::new();
        if let Some(nodes) = self.children.get(&parent) {
            for node in nodes.iter() {
                let mut node = node;
                loop {
                    if let Some(children) = self.children.get(node) {
                        if children.len() == 1 && !self.marked.contains(node) {
                            node = children.iter().next().unwrap();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                new_children.insert(*node);
            }
        }
        new_children
    }

    /// Return a Newick representation of the tree.
    pub fn to_newick(&self) -> String {
        let mut n = String::from("(");
        self.newick_helper(&mut n, 1);
        n.push_str(");");
        n
    }

    /// Helper function that actually makes the Newick format representation
    /// of the tree. The resulting String is in `n` and the current node is
    /// `taxid`.
    ///
    /// This function is recursive, hence it should be called only once with
    /// the root.
    fn newick_helper(&self, n: &mut String, taxid: i64) {
        // unwrap are safe here because of the way we build the tree
        // and the nodes.
        let node = self.nodes.get(&taxid).unwrap();
        let sciname = &node.names.get("scientific name").unwrap()[0];
        n.push_str(&format!("{}", sciname));

        if let Some(children) = self.children.get(&taxid) {
            n.push_str(",(");
            let mut children = children.iter();
            while let Some(child) = children.next() {
                self.newick_helper(n, *child);
                n.push(',');
            }

            // After iterating through the children, a comma left
            let _ = n.pop();
            n.push(')');
        }
    }

    /// Helper function that actually makes the String-representation of the
    /// tree. The resulting representation is in `s`, the current node is
    /// `taxid`, the `prefix` is used for spacing, and the boolean
    /// `was_first_child` is used to choose which branching character to use.
    ///
    /// This function is recursive, hence it should be called only once with
    /// the root.
    fn print_tree_helper(&self, s: &mut String, taxid: i64, prefix: String, was_first_child: bool) {
        // .unwrap() is safe here because of the way we build the tree.
        let node = self.nodes.get(&taxid).unwrap();
        let sciname = &node.names.get("scientific name").unwrap()[0];

        if let Some(children) = self.children.get(&taxid) {
            if self.marked.contains(&taxid) {
                s.push_str(&format!("{}\u{2500}\u{252C}\u{2500} {}: {}\n",
                                   prefix,
                                   Style::new().bold().paint(&node.rank),
                                   Style::new().bold().paint(sciname)));

            } else {
                s.push_str(&format!("{}\u{2500}\u{252C}\u{2500} {}: {}\n",
                                   prefix, node.rank, sciname));
            }
            let mut prefix = prefix.clone();
            prefix.pop();
            if was_first_child {
                prefix.push('\u{2502}');
            } else {
                prefix.push(' ');
            }

            // We want to keep the last child
            let mut children: Vec<i64> = children.iter().map(|r| *r).collect();
            children.sort();

            loop {
                let child = children.pop();
                let mut new_prefix = prefix.clone();
                match child {
                    Some(child) => {
                        if children.is_empty() {
                            new_prefix.push_str(" \u{2514}");
                            self.print_tree_helper(s, child, new_prefix, false);
                        } else {
                            new_prefix.push_str(" \u{251C}");
                            self.print_tree_helper(s, child, new_prefix, true);
                        }
                    },

                    None => break
                };
            }
        } else {
            if self.marked.contains(&taxid) {
                s.push_str(&format!("{}\u{2500}\u{2500} {}: {}\n",
                                   prefix,
                                   Style::new().bold().paint(&node.rank),
                                   Style::new().bold().paint(sciname)));
            } else {
                s.push_str(&format!("{}\u{2500}\u{2500} {}: {}\n",
                                   prefix, node.rank, sciname));
            }
        }
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();
        self.print_tree_helper(&mut s, 1, String::from(" "), false);
        write!(f, "{}", s)
    }
}
