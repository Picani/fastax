use std::error::Error;
use std::path::PathBuf;
use std::fs::{File, read_to_string};
use std::io;

use suppaftp::{FtpStream, FtpError};
use md5::Context;
use rusqlite::Connection;

use crate::Node;
use crate::NCBI_FTP_HOST;
use crate::NCBI_FTP_PATH;
use tempfile::{TempDir, Builder};

/// The local taxonony database
pub struct DB {
    conn: Connection
}

impl DB {
    /// Open a database.
    pub fn new(dbpath: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let conn = Connection::open(dbpath)?;
        debug!("Database opened.");
        Ok(DB { conn })
    }

    //-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-
    // Database initialization and population

    /// Populate the local taxonony database using that dump.
    ///
    /// *dump* is expected to be the path to an accessible copy of the
    /// `taxdmp.zip` file, as the one available on the NCBI FTP servers.
    pub fn populate(&self, dump: &PathBuf) -> Result<(), Box<dyn Error>> {
        info!("Initialization of the database.");
        self.init_db()?;

        info!("Extracting dumps...");
        let dumpdir = extract_dump(dump)?;

        info!("Loading dumps into local database. This may take some time.");
        self.insert_divisions(&dumpdir.path().join("division.dmp"))?;
        self.insert_genetic_codes(&dumpdir.path().join("gencode.dmp"))?;
        self.insert_names(&dumpdir.path().join("names.dmp"))?;
        self.insert_nodes(&dumpdir.path().join("nodes.dmp"))?;

        info!("C'est fini !");
        Ok(())
    }

    /// Initialize a the database by running the CREATE TABLE statements.
    fn init_db(&self) -> Result<(), Box<dyn Error>> {
        static CREATE_TABLES_STMT: &str = "
DROP TABLE IF EXISTS divisions;
DROP TABLE IF EXISTS geneticCodes;
DROP TABLE IF EXISTS nodes;
DROP TABLE IF EXISTS names;

CREATE TABLE IF NOT EXISTS divisions (
    id INTEGER NOT NULL PRIMARY KEY,
    division TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS geneticCodes (
    id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS nodes (
    tax_id INTEGER NOT NULL PRIMARY KEY,
    parent_tax_id INTEGER,
    rank TEXT NOT NULL,
    division_id INTEGER NOT NULL,
    genetic_code_id INTEGER NOT NULL,
    mito_genetic_code_id INTEGER NOT NULL,
    comment TEXT,

    FOREIGN KEY(division_id) REFERENCES divisions(id)
    FOREIGN KEY(genetic_code_id) REFERENCES geneticCodes(code_id)
    FOREIGN KEY(mito_genetic_code_id) REFERENCES geneticCodes(code_id)
);

CREATE TABLE IF NOT EXISTS names (
    id         INTEGER NOT NULL PRIMARY KEY,
    tax_id     INTEGER NOT NULL,
    name       TEXT NOT NULL,
    name_class TEXT NOT NULL
);";

        self.conn.execute_batch(CREATE_TABLES_STMT)?;
        debug!("Tables created.");
        Ok(())
    }

    /// Read the names.dmp file and insert the records into the database. When
    /// it's done, create the indexes on names and name classes.
    fn insert_names(&self, namesdump: &PathBuf) -> Result<(), Box<dyn Error>> {
        debug!("Inserting names...");

        let file = File::open(namesdump)?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b'|')
            .from_reader(file);

        let mut stmts: Vec<String> = vec![String::from("BEGIN;")];

        for (i, result) in rdr.records().enumerate() {
            if i > 1 && i%10_000 == 0 {
                stmts.push(String::from("COMMIT;"));
                let stmt = &stmts.join("\n");
                self.conn.execute_batch(stmt)?;
                debug!("Inserted {} records so far.", i);
                stmts.clear();
                stmts.push(String::from("BEGIN;"));
            }

            let record = result?;

            let taxid: i64 = record[0].trim().parse()?;
            let name: String = record[1].parse()?;
            let name_class: String = record[3].parse()?;

            stmts.push(format!("INSERT INTO names(tax_id, name, name_class)
                            VALUES ({}, '{}', '{}');",
                               taxid.to_string(),
                               name.trim().replace("'", "''"),
                               name_class.trim().replace("'", "''")));
        }

        // There could left records in stmts
        stmts.push(String::from("COMMIT;"));
        let stmt = &stmts.join("\n");
        self.conn.execute_batch(stmt)?;
        debug!("Done inserting names.");

        debug!("Creating names indexes.");
        self.conn.execute("CREATE INDEX idx_names_tax_id ON names(tax_id);", [])?;
        self.conn.execute("CREATE INDEX idx_names_name ON names(name);", [])?;

        Ok(())
    }

    /// Read the division.dmp file and insert the records into the database.
    fn insert_divisions(&self, divdump: &PathBuf) -> Result<(), Box<dyn Error>> {
        debug!("Inserting divisions...");

        let file = File::open(divdump)?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b'|')
            .from_reader(file);

        let mut stmts: Vec<String> = vec![String::from("BEGIN;")];

        for result in rdr.records() {
            let record = result?;
            let id: i64 = record[0].trim().parse()?;
            let name: String = record[2].trim().parse()?;
            stmts.push(format!("INSERT INTO divisions VALUES ({}, '{}');",
                               id,
                               name.replace("'", "''")));
        }

        stmts.push(String::from("COMMIT;"));
        let stmt = &stmts.join("\n");
        self.conn.execute_batch(stmt)?;
        debug!("Done inserting divisions.");

        Ok(())
    }

    /// Read the gencode.dmp file and insert the records into the database.
    fn insert_genetic_codes(&self, gencodedump: &PathBuf) -> Result<(), Box<dyn Error>> {
        debug!("Inserting genetic codes...");

        let file = File::open(gencodedump)?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b'|')
            .from_reader(file);

        let mut stmts: Vec<String> = vec![String::from("BEGIN;")];
        for result in rdr.records() {
            let record = result?;
            let id: i64 = record[0].trim().parse()?;
            let name: String = record[2].trim().parse()?;
            stmts.push(format!("INSERT INTO geneticCodes VALUES ({}, '{}');",
                               id,
                               name.replace("'", "''")));
        }

        stmts.push(String::from("COMMIT;"));
        let stmt = &stmts.join("\n");
        self.conn.execute_batch(stmt)?;
        debug!("Done inserting genetic codes.");

        Ok(())
    }

    /// Read the nodes.dmp file and insert the records into the database. When
    /// it's done, create the index on `parent_tax_id`.
    fn insert_nodes(&self, nodesdump: &PathBuf) -> Result<(), Box<dyn Error>> {
        debug!("Inserting nodes...");

        let file = File::open(nodesdump)?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b'|')
            .from_reader(file);

        let mut stmts: Vec<String> = vec![
            String::from("BEGIN;"),
            // Special case: the root
            String::from("INSERT INTO nodes VALUES (1, 1, 'no rank', 8, 0, 0, '');")
        ];

        let mut records = rdr.records().enumerate();
        records.next(); // We burn the root row
        for (i, result) in records {
            if i > 0 && i%10_000 == 0 {
                stmts.push(String::from("COMMIT;"));
                let stmt = &stmts.join("\n");
                self.conn.execute_batch(stmt)?;
                debug!("Inserted {} records so far.", i);
                stmts.clear();
                stmts.push(String::from("BEGIN;"));
            }

            let record = result?;

            let taxid: i64 = record[0].trim().parse()?;
            let parent_taxid: i64 = record[1].trim().parse()?;
            let rank: String = record[2].trim().parse()?;
            let division_id: i64 = record[4].trim().parse()?;
            let genetic_code_id: i64 = record[6].trim().parse()?;
            let mito_genetic_code_id: i64 = record[8].trim().parse()?;
            let comments: String = record[12].trim().parse()?;

            stmts.push(format!(
                "INSERT INTO nodes VALUES ({}, {}, '{}', {}, {}, {}, '{}');",
                taxid.to_string(),
                parent_taxid.to_string(),
                rank,
                division_id.to_string(),
                genetic_code_id.to_string(),
                mito_genetic_code_id.to_string(),
                comments
            ));
        }

        // There could left records in stmts
        stmts.push(String::from("COMMIT;"));
        let stmt = &stmts.join("\n");
        self.conn.execute_batch(stmt)?;
        debug!("Done inserting nodes.");

        debug!("Creating nodes indexes.");
        self.conn.execute("CREATE INDEX idx_nodes_parent_id ON nodes(parent_tax_id);", [])?;

        Ok(())
    }


    //-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-
    // Database querying

    /// Get the Taxonomy IDs corresponding to this scientific names. The used
    /// name class are "scientific name", "synonym" and "genbank synonym".
    /// Either return all the IDs or an error.
    pub fn get_taxids(&self, names: Vec<String>) -> Result<Vec<i64>, Box<dyn Error>> {
        let mut taxids = vec![];

        let mut stmt = self.conn.prepare("
    SELECT tax_id FROM names
    WHERE name_class IN ('scientific name', 'synonym', 'genbank synonym')
    AND name=?")?;

        for name in names.iter() {
            let mut rows = stmt.query(&[name])?;
            let row = rows.next()?;
            if let Some(row) = row {
                // With the right database, get_unwrap should be safe.
                taxids.push(row.get_unwrap(0));
            } else {
                return Err(From::from(format!("No such scientific name: {}", name)));
            }
        }

        Ok(taxids)
    }

    /// Get the Nodes corresponding to the IDs. The Nodes are ordered in the same
    /// way as the IDs. If an ID is invalid, an error is returned.
    pub fn get_nodes(&self, ids: Vec<i64>) -> Result<Vec<Node>, Box<dyn Error>> {
        let mut nodes = vec![];

        let mut stmt = self.conn.prepare("
    SELECT
      nodes.tax_id,
      nodes.parent_tax_id,
      nodes.rank,
      divisions.division,
      code.name as code,
      mito.name as mito,
      names.name_class,
      names.name,
      nodes.comment
    from nodes
      inner join divisions on nodes.division_id = divisions.id
      inner join names on nodes.tax_id = names.tax_id
      inner join geneticCodes code on nodes.genetic_code_id = code.id
      inner join geneticCodes mito on nodes.mito_genetic_code_id = mito.id
    where nodes.tax_id=?")?;

        for id in ids.iter() {
            let mut rows = stmt.query(&[id])?;

            let mut node: Node = Default::default();

            let row = rows.next()?;
            if let Some(row) = row {
                // With the right database, get_unwrap should be safe.
                node.tax_id = row.get_unwrap(0);
                node.parent_tax_id = row.get_unwrap(1);
                node.rank = row.get_unwrap(2);
                node.division = row.get_unwrap(3);
                node.genetic_code = row.get_unwrap(4);

                let mito_code: String = row.get_unwrap(5);
                if mito_code != "Unspecified" {
                    node.mito_genetic_code = row.get_unwrap(5);
                }

                let comments: String = row.get_unwrap(8);
                if !comments.is_empty() {
                    node.comments = Some(comments);
                }

                node.names.entry(row.get_unwrap(6))
                    .or_insert_with(|| vec![row.get_unwrap(7)]);
            } else {
                return Err(From::from(format!("No such ID: {}", id)));
            }

            loop {
                let row = rows.next()?;

                if let Some(row) = row {
                    // With the right database, get_unwrap should be safe.
                    node.names.entry(row.get_unwrap(6))
                        .and_modify(|n| n.push(row.get_unwrap(7)))
                        .or_insert_with(|| vec![row.get_unwrap(7)]);

                } else {
                    break;
                }
            }

            nodes.push(node);
        }

        Ok(nodes)
    }

    /// Get the Node corresponding to this unique ID, then all Nodes in the path
    /// to the root (the special node with taxonomy ID 1). The Nodes are ordered,
    /// with the root last.
    pub fn get_lineage(&self, id: i64) -> Result<Vec<Node>, Box<dyn Error>> {
        let mut id = id;
        let mut ids = vec![id];
        let mut stmt = self.conn.prepare("SELECT parent_tax_id FROM nodes WHERE tax_id=?")?;
        loop {
            let parent_id = stmt.query_row([id], |row| {row.get(0)})?;
            ids.push(parent_id);
            id = parent_id;

            if id == 1 {
                break;
            }
        }

        let mut lineage = self.get_nodes(ids)?;
        lineage.reverse();
        Ok(lineage)
    }

    /// Get the children of the Node corresponding to this unique ID. If
    /// `species_only` is true, then stop when the children are species, else
    /// continue until the children are tips.
    /// Note that the ID given as argument is included in the results. Thus, the
    /// resulting vector contains at least one element.
    pub fn get_children(&self, id: i64, species_only: bool) -> Result<Vec<Node>, Box<dyn Error>> {
        let mut ids: Vec<i64> = vec![];
        let mut temp_ids = vec![id];

        let mut stmt = self.conn.prepare("SELECT tax_id, rank FROM nodes WHERE parent_tax_id=?")?;

        while let Some(id) = temp_ids.pop() {
            ids.push(id);

            let mut rows = stmt.query([id])?;
            loop {
                let row = rows.next()?;
                if let Some(row) = row {
                    // With the right database, get_unwrap should be safe.
                    let rank: String = row.get_unwrap(1);

                    if species_only && rank == "species" {
                        ids.push(row.get_unwrap(0));
                    } else {
                        temp_ids.push(row.get_unwrap(0))
                    }

                } else {
                    break;
                }
            }
        }

        let nodes = self.get_nodes(ids)?;
        Ok(nodes)
    }

}


//-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-
// Utility functions

/// Download the latest release of `taxdmp.zip` and `taxdmp.zip.md5`
/// from the NCBI FTP servers.
pub fn download_taxdump(datadir: &PathBuf, email: String) -> Result<(), Box<dyn Error>> {
    debug!("Contacting {}...", NCBI_FTP_HOST);
    let mut conn = FtpStream::connect(NCBI_FTP_HOST)?;
    conn.login("ftp", &email)?;
    debug!("Connected and logged.");

    conn.cwd(NCBI_FTP_PATH)?;

    debug!("Retrieving MD5 sum file...");
    conn.retr("taxdmp.zip.md5", move |stream| {
        let path = datadir.join("taxdmp.zip.md5");
        let mut file = match File::create(path) {
            Err(e) => return Err(FtpError::ConnectionError(e)),
            Ok(f) => f
        };
        io::copy(stream, &mut file)
            .map(|_| ())
            .map_err(FtpError::ConnectionError)
    })?;

    debug!("Retrieving dumps file...");
    conn.retr("taxdmp.zip", |stream| {
        let path = datadir.join("taxdmp.zip");
        let mut file = match File::create(path) {
            Err(e) => return Err(FtpError::ConnectionError(e)),
            Ok(f) => f
        };
        io::copy(stream, &mut file).map_err(FtpError::ConnectionError)
    })?;

    conn.quit()?;
    debug!("We're done. Ending connection.");
    Ok(())
}

/// Check the integrity of `taxdmp.zip` using `taxdmp.zip.md5`.
pub fn check_integrity(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let path = datadir.join("taxdmp.zip");
    let mut file = File::open(path)?;
    let mut hasher = Context::new();
    debug!("Computing MD5 sum...");
    io::copy(&mut file, &mut hasher)?;
    let digest = format!("{:x}", hasher.compute());

    let path = datadir.join("taxdmp.zip.md5");
    let mut ref_digest = read_to_string(path)?;
    ref_digest.truncate(32);

    if digest != ref_digest {
        warn!("Expected sum is: {}", ref_digest);
        warn!("Computed sum is: {}", digest);
        panic!("Fail to check integrity.");
    } else {
        Ok(())
    }
}

/// Extract all files from taxdmp.zip in a temporary directory and return it.
fn extract_dump(dump: &PathBuf) -> Result<TempDir, Box<dyn Error>> {
    let file = File::open(dump)?;
    let tmp_dir = Builder::new().prefix("fastax").tempdir()?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = tmp_dir.path().join(file.mangled_name());

        let mut outfile = File::create(&outpath)?;
        io::copy(&mut file, &mut outfile)?;
        debug!("Extracted {}", outpath.as_path().display());
    }
    Ok(tmp_dir)
}
