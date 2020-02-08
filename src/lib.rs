extern crate ansi_term;
extern crate csv;
extern crate ftp;
#[macro_use]
extern crate log;
extern crate loggerv;
extern crate md5;
extern crate rusqlite;
// extern crate simple_logger;
extern crate structopt;
extern crate xdg;
extern crate zip;

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;


static NCBI_FTP_HOST: &str = "ftp.ncbi.nih.gov:21";
static NCBI_FTP_PATH: &str = "/pub/taxonomy";

mod db;
pub mod tree;


/// Populate the local taxonomy DB at `datadir` while sending `email`
/// to the NCBI FTP servers.
pub fn populate_db(datadir: &PathBuf, email: String) -> Result<(), Box<dyn Error>> {
    info!("Downloading data from {}...", NCBI_FTP_HOST);
    db::download_taxdump(&datadir, email)?;
    info!("Checking download integrity...");
    db::check_integrity(&datadir)?;
    info!("Everything's OK!");
    info!("Extracting dumps...");
    db::extract_dump(&datadir)?;
    info!("Initialization of the database.");
    db::init_db(&datadir)?;
    info!("Loading dumps into local database. This may take some time.");
    db::insert_divisions(&datadir)?;
    db::insert_genetic_codes(&datadir)?;
    db::insert_names(&datadir)?;
    db::insert_nodes(&datadir)?;
    info!("Removing temporary files.");
    db::remove_temp_files(&datadir)?;
    info!("C'est fini !");
    Ok(())
}

/// Fetch from the database the node that corresponds to the given `term`
/// and return it. If the term does not correspond to a Node, an error
/// is returned.
pub fn get_node(datadir: &PathBuf, term: String) -> Result<Node, Box<dyn Error>> {
    let ids = term_to_taxids(&datadir, &[term])?;
    let node = db::get_nodes(&datadir, ids)?;
    Ok(node[0].clone())
}

/// Fetch from the database the nodes that correspond to the given `terms`
/// and return them. If any of the term does not correspond to a Node, an
/// error is returned.
pub fn get_nodes(datadir: &PathBuf, terms: &[String]) -> Result<Vec<Node>, Box<dyn Error>> {
    let ids = term_to_taxids(&datadir, terms)?;
    db::get_nodes(&datadir, ids)
}

/// Make the lineage for each of the given `nodes`.
pub fn make_lineages(datadir: &PathBuf, nodes: &[Node]) -> Result<Vec<Vec<Node>>, Box<dyn Error>> {
    // From https://stackoverflow.com/a/26370894
    let lineages: Result<Vec<_>, _> = nodes.iter()
        .map(|node| db::get_lineage(&datadir, node.tax_id))
        .collect();
    lineages
}

/// Make the tree with the Root as root and the given `nodes` as leaves.
/// Any given node that is not a leaf (because another given node is in
/// its sub-tree) is kept is the returned tree.
pub fn make_tree(datadir: &PathBuf, nodes: &[Node]) -> Result<tree::Tree, Box<dyn Error>> {
    let mut lineages = make_lineages(&datadir, nodes)?;
    lineages.sort_by(|a, b| b.len().cmp(&a.len()));

    // The root taxid is 1
    let mut tree = tree::Tree::new(1, &lineages.pop().unwrap());
    for lineage in lineages.iter() {
        tree.add_nodes(lineage);
    }
    let ids: Vec<_> = nodes.iter().map(|node| node.tax_id).collect();
    tree.mark_nodes(&ids);
    Ok(tree)
}

/// Make the sub-tree with the given `root` as root.
/// If `species` is true, then doesn't include in the resulting tree
/// the nodes that are below nodes ranked as species (such as subspecies).
pub fn make_subtree(datadir: &PathBuf, root: Node, species: bool) -> Result<tree::Tree, Box<dyn Error>> {
    let nodes = db::get_children(&datadir, root.tax_id, species)?;
    Ok(tree::Tree::new(root.tax_id, &nodes))
}

/// Get the Last Common Ancestor (LCA) of `node1` and `node2`.
pub fn get_lca(datadir: &PathBuf, node1: &Node, node2: &Node) -> Result<Node, Box<dyn Error>> {
    let node1 = node1.clone();
    let node2 = node2.clone();
    let mut tree = make_tree(datadir, &[node1, node2])?;
    tree.simplify();

    // Two cases here: the LCA is the root, or the LCA is the root's child.
    let lca_id =
        if tree.children.get(&1).unwrap().len() == 2 {
            &1
        } else {
            tree.children.get(&1).unwrap().iter().next().unwrap()
        };
    let lca = tree.nodes.get(lca_id).unwrap();
    Ok(lca.clone())
}

//=============================================================================
// Database models

#[derive(Debug, Clone, Default)]
pub struct Node {
    pub tax_id: i64,
    parent_tax_id: i64,
    pub rank: String,
    pub division: String,
    pub genetic_code: String,
    pub mito_genetic_code: Option<String>,
    pub comments: Option<String>,
    pub names: HashMap<String, Vec<String>>, // many synonym or common names
    pub format_string: Option<String>,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(format_string) = &self.format_string {
            // Format the Node according to its format string.
            return write!(f, "{}", format_string
                          .replace("%taxid", &self.tax_id.to_string())
                          .replace("%name", &self.names.get("scientific name").unwrap()[0])
                          .replace("%rank", &self.rank));
        }

        let mut lines = String::new();

        let sciname = &self.names.get("scientific name").unwrap()[0];
        let l1 = format!("{} - {}\n", sciname, self.rank);
        let l2 = std::iter::repeat("-").take(l1.len()-1).collect::<String>();
        lines.push_str(&l1);
        lines.push_str(&l2);
        lines.push_str(&format!("\nNCBI Taxonomy ID: {}\n", self.tax_id));

        if self.names.contains_key("synonym") {
            lines.push_str("Same as:\n");
            for synonym in self.names.get("synonym").unwrap() {
                lines.push_str(&format!("* {}\n", synonym));
            }
        }

        if self.names.contains_key("genbank common name") {
            let genbank = &self.names.get("genbank common name").unwrap()[0];
            lines.push_str(&format!("Commonly named {}.\n", genbank));
        }

        if self.names.contains_key("common name") {
            lines.push_str("Also known as:\n");
            for name in self.names.get("common name").unwrap() {
                lines.push_str(&format!("* {}\n", name));
            }
        }

        if self.names.contains_key("authority") {
            lines.push_str("First description:\n");
            for authority in self.names.get("authority").unwrap() {
                lines.push_str(&format!("* {}\n", authority));
            }
        }

        lines.push_str(&format!("Part of the {}.\n", self.division));
        lines.push_str(&format!("Uses the {} genetic code.\n", self.genetic_code));

        if let Some(ref mito) = self.mito_genetic_code {
            lines.push_str(&format!("Its mitochondria use the {} genetic code.\n", mito));
        }

        if let Some(ref comments) = self.comments {
            lines.push_str(&format!("\nComments: {}", comments));
        }

        write!(f, "{}", lines)
    }
}

//=============================================================================
// Utils functions

/// Trim a string and replace all underscore by space. Return a new String.
fn clean_term(term: &str) -> String {
    term.trim().replace("_", " ")
}


/// Return a list of Taxonomy IDs from the given terms. Each term can be
/// an ID already or a scientific name. In the second case, the corresponding
/// ID is fetched from the database. The input order is kept.
/// Return either a vector of taxids or an error (for example, one scientific
/// name cannot be found).
fn term_to_taxids(datadir: &PathBuf, terms: &[String]) -> Result<Vec<i64>, Box<dyn Error>> {
    // We want to keep the input order. This makes the code slightly
    // more complicated.
    let mut ids: Vec<i64> = vec![];
    let terms: Vec<String> = terms.iter()
        .map(|term| clean_term(term))
        .collect();
    let mut names: Vec<String> = vec![];
    let mut indices: Vec<usize> = vec![];

    for (i, term) in terms.iter().enumerate() {
        match term.parse::<i64>() {
            Ok(id) => ids.push(id),
            Err(_) => {
                names.push(term.to_string());
                indices.push(i);
                ids.push(-1)
            }
        };
    }

    let name_ids = db::get_taxids(&datadir, names)?;
    for (idx, taxid) in indices.iter().zip(name_ids.iter()) {
        ids[*idx] = *taxid;
    }

    Ok(ids)
}
