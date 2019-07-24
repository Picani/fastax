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
use std::io;
use std::path::PathBuf;

use structopt::StructOpt;

static NCBI_FTP_HOST: &str = "ftp.ncbi.nih.gov:21";
static NCBI_FTP_PATH: &str = "/pub/taxonomy";

mod db;
mod tree;


/// Explore the NCBI Taxonomy database from a local copy.
#[derive(StructOpt)]
pub struct Opt {
    #[structopt(subcommand)]
    cmd: Command,

    /// Be verbose
    #[structopt(short = "v", long = "verbose")]
    verbosity: bool,

    /// Be extremely verbose
    #[structopt(short = "d", long = "debug")]
    debug: bool,
}

#[derive(StructOpt)]
enum Command {
    /// Lookup for NCBI Taxonomy ID(s) or scientific name(s) and show the
    /// results; no search is performed, only exact matches are returned
    #[structopt(name = "show")]
    Show {
        /// The NCBI Taxonomy ID(s) or scientific name(s)
        terms: Vec<String>,

        /// Output the results as CSV
        #[structopt(short = "c", long = "csv")]
        csv: bool,
    },

    /// Output the lineage of the node(s) (i.e. all nodes in
    /// the path to the root)
    #[structopt(name = "lineage")]
    Lineage {
        /// The NCBI Taxonomy ID(s) or scientific name(s)
        terms: Vec<String>,

        /// Keep only the nodes that have a named rank
        #[structopt(short = "r", long = "ranks")]
        ranks: bool,

        /// Output the results as CSV; the rows might have different number
        /// of columns; each cell is of the form rank:scientific name:taxid
        #[structopt(short = "c", long = "csv")]
        csv: bool,
    },

    /// (Re)populate the local taxonomy database by downloading the
    /// latest release from the NCBI servers
    #[structopt(name = "populate")]
    Populate {
        /// Use that email when connecting to NCBI servers
        #[structopt(short = "e", long = "email", default_value="plop@example.com")]
        email: String
    },

    /// Make a tree from the root to all given IDs
    #[structopt(name = "tree")]
    Tree {
        /// The NCBI Taxonomy IDs or scientific name(s)
        terms: Vec<String>,

        /// Show all internal nodes
        #[structopt(short = "i", long = "internal")]
        internal: bool,

        /// Print the tree in Newick format
        #[structopt(short = "n", long = "newick")]
        newick: bool,
    }
}

pub fn run(opt: Opt) -> Result<(), Box<dyn Error>> {
    if opt.debug {
        loggerv::Logger::new()
            .max_level(log::Level::Debug)
            .level(true)
            .init()?;
        // simple_logger::init_with_level(log::Level::Debug)?;
   } else if opt.verbosity {
        loggerv::Logger::new()
            .max_level(log::Level::Info)
            .level(true)
            .init()?;
        // simple_logger::init_with_level(log::Level::Info)?;
    } else {
        loggerv::init_quiet()?;
        // simple_logger::init_with_level(log::Level::Warn)?;
    }

    let xdg_dirs = xdg::BaseDirectories::with_prefix("fastax")?;
    let datadir = xdg_dirs.get_data_home();
    let _ = xdg_dirs.create_data_directory(&datadir)?;

    match opt.cmd {
        Command::Populate{email} => {
            info!("Downloading data from {}...", NCBI_FTP_HOST);
            let _ = db::download_taxdump(&datadir, email)?;
            info!("Checking download integrity...");
            let _ = db::check_integrity(&datadir)?;
            info!("Everything's OK!");
            info!("Extracting dumps...");
            let _ = db::extract_dump(&datadir)?;
            info!("Initialization of the database.");
            let _ = db::init_db(&datadir)?;
            info!("Loading dumps into local database. This may take some time.");
            let _ = db::insert_divisions(&datadir)?;
            let _ = db::insert_genetic_codes(&datadir)?;
            let _ = db::insert_names(&datadir)?;
            let _ = db::insert_nodes(&datadir)?;
            info!("Removing temporary files.");
            let _ = db::remove_temp_files(&datadir)?;
            info!("C'est fini !");
        },

        Command::Show{terms, csv} => {
            let ids = term_to_taxids(&datadir, terms)?;
            let nodes = db::get_nodes(&datadir, ids)?;
            if csv {
                let mut wtr = csv::Writer::from_writer(io::stdout());

                wtr.write_record(&["taxid", "scientific_name",
                                   "rank", "division", "genetic_code",
                                   "mitochondrial_genetic_code"])?;
                for node in nodes.iter() {
                    wtr.serialize((
                        node.tax_id,
                        &node.names.get("scientific name").unwrap()[0],
                        &node.rank,
                        &node.division,
                        &node.genetic_code,
                        &node.mito_genetic_code))?;
                }
                wtr.flush()?;

            } else {
                for node in nodes.iter() {
                    println!("{}", node);
                }
            }
        },

        Command::Lineage{terms, ranks, csv} => {
            let ids = term_to_taxids(&datadir, terms)?;
            let lineages = ids.iter()
                    .map(|id| db::get_lineage(&datadir, *id));

            if csv {
                let mut wtr = csv::WriterBuilder::new()
                    .flexible(true)
                    .from_writer(io::stdout());

                for lineage in lineages {
                    let nodes = lineage?;
                    let row = nodes.iter()
                        .filter(|node| !ranks || node.rank != "no rank")
                        .map(|node| format!("{}:{}:{}",
                                            &node.rank,
                                            &node.names.get("scientific name").unwrap()[0],
                                            node.tax_id))
                        .collect::<Vec<String>>();
                    wtr.serialize(row)?;
                }
                wtr.flush()?;
            } else {
                for lineage in lineages {
                    let nodes = lineage?
                        .iter()
                        .filter(|node| !ranks || node.rank != "no rank")
                        .map(|node| format!("{}: {} (taxid: {})",
                                            &node.rank,
                                            &node.names.get("scientific name").unwrap()[0],
                                            node.tax_id))
                        .collect::<Vec<String>>();

                    for (i, node) in nodes.iter().enumerate() {
                        if i == 0 { println!("root"); }
                        else if i == nodes.len() - 1 {
                            println!("{}\u{2514}\u{2500}\u{2500} {}",
                                     std::iter::repeat(" ").take(i+1).collect::<String>(),
                                     node);
                        } else {
                            println!("{}\u{2514}\u{252C}\u{2500} {}",
                                     std::iter::repeat(" ").take(i+1).collect::<String>(),
                                     node);
                        }
                    }
                }
            }
        },

        Command::Tree{terms, internal, newick} => {
            let ids = term_to_taxids(&datadir, terms)?;
            let mut lineages = ids.iter()
                .map(|id| db::get_lineage(&datadir, *id))
                .collect::<Result<Vec<_>, _>>()?;
            lineages.sort_by(|a, b| b.len().cmp(&a.len()));

            let mut tree = tree::Tree::new(&lineages.pop().unwrap());
            for lineage in lineages.iter() {
                tree.add_nodes(lineage);
            }
            tree.mark_nodes(&ids);

            if !internal {
                tree.simplify();
            }

            if newick {
                println!("{}", tree.to_newick());
            } else {
                println!("{}", tree);
            }
        }
    }

    Ok(())
}

//=============================================================================
// Database models

#[derive(Debug, Clone)]
pub struct Node {
    tax_id: i64,
    parent_tax_id: i64,
    rank: String,
    division: String,
    genetic_code: String,
    mito_genetic_code: Option<String>,
    comments: Option<String>,
    names: HashMap<String, Vec<String>> // many synonym or common names
}

impl Node {
    pub fn new() -> Node {
        Node{
            tax_id: 0,
            parent_tax_id: 0,
            rank: String::new(),
            division: String::new(),
            genetic_code: String::new(),
            mito_genetic_code: None, // Not all organisms have mitochondria
            comments: None, // Only a small fraction of nodes have comments
            names: HashMap::new()
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
fn clean_term(term: &String) -> String {
    term.trim().replace("_", " ")
}


/// Return a list of Taxonomy IDs from the given terms. Each term can be
/// an ID already or a scientific name. In the second case, the corresponding
/// ID is fetched from the database. The input order is kept.
/// Return either a vector of taxids or an error (for example, one scientific
/// name cannot be found).
fn term_to_taxids(datadir: &PathBuf, terms: Vec<String>) -> Result<Vec<i64>, Box<dyn Error>> {
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
