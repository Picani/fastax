#[macro_use]
extern crate log;
extern crate structopt;
extern crate fastax;

use std::error::Error;
use std::io;
use std::process;

use structopt::StructOpt;


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
    /// Warning: by default, it doesn't show all internal nodes, which may
    /// not be what you want! In that case, use -i/--internal.
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

        /// Format the nodes with this formatting string (%rank is replaced
        /// the rank, %name by the scientific name and %taxid by the NCBI
        /// taxonomy ID)
        #[structopt(short = "f", long = "format")]
        format: Option<String>,
    },

    /// Make a tree with the given ID as root.
    /// Warning: by default, it doesn't show all internal nodes, which may
    /// not be what you want! In that case, use -i/--internal.
    #[structopt(name = "subtree")]
    SubTree {
        /// The NCBI Taxonomy ID or scientific name
        term: String,

        /// Stop at species instead of tips (can be subspecies)
        #[structopt(short = "s", long = "species")]
        species: bool,

        /// Show all internal nodes
        #[structopt(short = "i", long = "internal")]
        internal: bool,

        /// Print the tree in Newick format
        #[structopt(short = "n", long = "newick")]
        newick: bool,

        /// Format the nodes with this formatting string (%rank is replaced
        /// the rank, %name by the scientific name and %taxid by the NCBI
        /// taxonomy ID)
        #[structopt(short = "f", long = "format")]
        format: Option<String>,
    },
}

/// Pretty-print the `nodes`. If `csv` is true, print the node as CSV.
fn show(nodes: Vec<fastax::Node>, csv: bool) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

/// Pretty-print the given `lineages`.
/// If `ranks` is true, then keep only the Nodes that have a named rank.
/// If `csv` is true, print the lineage as CSV.
fn show_lineages(lineages: Vec<Vec<fastax::Node>>, ranks: bool, csv: bool) -> Result<(), Box<dyn Error>> {
    if csv {
        let mut wtr = csv::WriterBuilder::new()
            .flexible(true)
            .from_writer(io::stdout());

        for lineage in lineages {
            let nodes = lineage;
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
            let nodes = lineage.iter()
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
    Ok(())
}

/// Pretty-print the tree with the Nodes corresponding to the given `terms`.
/// If `internal` is true, print also the intenal nodes (*i.e.* the nodes
/// that have only one child).
/// If `newick` is true, print the tree in Newick format.
/// If `format` is given, use it as the format string for all nodes.
fn show_tree(mut tree: fastax::tree::Tree, internal: bool, newick: bool, format: Option<String>) -> Result<(), Box<dyn Error>> {
    if let Some(format_string) = format {
        tree.set_format_string(format_string);
    } else if newick {
        // The default formatting for tree is not really useful
        // for newick trees
        tree.set_format_string(String::from("%name"));
    }

    if !internal {
        tree.simplify();
    }

    if newick {
        println!("{}", tree.to_newick());
    } else {
        println!("{}", tree);
    }
    Ok(())
}

/// Run fastax!!!
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
    xdg_dirs.create_data_directory(&datadir)?;

    match opt.cmd {
        Command::Populate{email} => {
            fastax::populate_db(&datadir, email)?;
        },

        Command::Show{terms, csv} => {
            let nodes = fastax::get_nodes(&datadir, &terms)?;
            show(nodes, csv)?;
        },

        Command::Lineage{terms, ranks, csv} => {
            let lineages = fastax::make_lineages(&datadir, &terms)?;
            show_lineages(lineages, ranks, csv)?;
        },

        Command::Tree{terms, internal, newick, format} => {
            let tree = fastax::make_tree(&datadir, &terms)?;
            show_tree(tree, internal, newick, format)?;
        },

        Command::SubTree{term, species, internal, newick, format} => {
            let tree = fastax::make_subtree(&datadir, &term, species)?;
            show_tree(tree, internal, newick, format)?;
        }
    }

    Ok(())
}

/// Main entry point
fn main() {
    let opt = Opt::from_args();

    if let Err(e) = run(opt) {
        if e.description().contains("no such table") {
            error!("The database is probably not initialized.\nTry running: 'fastax populate'");
        } else {
            error!("{}", e);
        }
    }
    process::exit(exitcode::OK);
}
