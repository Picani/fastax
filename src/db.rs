use std::error::Error;
use std::path::PathBuf;
use std::fs::{File, read_to_string, remove_file};
use std::io;

use ftp::{FtpStream, FtpError};
use md5::Context;
use rusqlite::{Connection, NO_PARAMS};

use Node;
use NCBI_FTP_HOST;
use NCBI_FTP_PATH;

/// Open the taxonomy database in this directory.
fn open_db(dir: &PathBuf) -> Result<Connection, Box<dyn Error>> {
    let dbpath = dir.join("taxonomy.db");
    let conn = Connection::open(dbpath)?;
    debug!("Database opened.");

    Ok(conn)
}

/// Get the Nodes corresponding to the IDs. The Nodes are ordered in the same
/// way as the IDs. If an ID is invalid, an error is returned.
pub fn get_nodes(dir: &PathBuf, ids: Vec<i64>) -> Result<Vec<Node>, Box<dyn Error>> {
    let mut nodes = vec![];
    let conn = open_db(dir)?;

    let mut stmt = conn.prepare("
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

        let mut node;
        // Here, row.get has no reason to return an error
        // so row.get_unwrap should be safe
        if let Some(row) = rows.next() {
            let row = row?;
            node = Node::new();
            node.tax_id = row.get(0);
            node.parent_tax_id = row.get(1);
            node.rank = row.get(2);
            node.division = row.get(3);
            node.genetic_code = row.get(4);

            let mito_code: String = row.get(5);
            if mito_code != "Unspecified" {
                node.mito_genetic_code = row.get(5);
            }

            let comments: String = row.get(8);
            if !comments.is_empty() {
                node.comments = Some(comments);
            }

            node.names.entry(row.get(6))
                .or_insert(vec![row.get(7)]);
        } else {
            return Err(From::from(format!("No such ID: {}", id)));
        }

        while let Some(row) = rows.next() {
            let row = row?;
            node.names.entry(row.get(6))
                .and_modify(|n| n.push(row.get(7)))
                .or_insert(vec![row.get(7)]);
        }

        nodes.push(node);
    }

    Ok(nodes)
}

/// Get the Node corresponding to this unique ID, then all Nodes in the path
/// to the root (the special node with taxonomy ID 1). The Nodes are ordered,
/// with the root last.
pub fn get_lineage(dir: &PathBuf, id: i64) -> Result<Vec<Node>, Box<dyn Error>> {
    let conn = open_db(dir)?;

    let mut id = id;
    let mut ids = vec![id];
    let mut stmt = conn.prepare("SELECT parent_tax_id FROM nodes WHERE tax_id=?")?;
    loop {
        let parent_id = stmt.query_row(&[id], |row| {row.get(0)})?;
        ids.push(parent_id);
        id = parent_id;

        if id == 1 {
            break;
        }
    }

    let mut lineage = get_nodes(dir, ids)?;
    lineage.reverse();
    Ok(lineage)
}


//-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-
// Database downloading


/// Download the latest release of `taxdmp.zip` and `taxdmp.zip.md5`
/// from the NCBI FTP servers.
pub fn download_taxdump(datadir: &PathBuf, email: String) -> Result<(), Box<dyn Error>> {
    debug!("Contacting {}...", NCBI_FTP_HOST);
    let mut conn = FtpStream::connect(NCBI_FTP_HOST)?;
    let _ = conn.login("ftp", &email)?;
    debug!("Connected and logged.");

    let _ = conn.cwd(NCBI_FTP_PATH)?;

    debug!("Retrieving MD5 sum file...");
    let path = datadir.join("taxdmp.zip.md5");
    let mut file = File::create(path)?;
    let mut cursor = conn.simple_retr("taxdmp.zip.md5")?;
    io::copy(&mut cursor, &mut file)?;

    debug!("Retrieving dumps file...");
    let _ = conn.retr("taxdmp.zip", |stream| {
        let path = datadir.join("taxdmp.zip");
        let mut file = match File::create(path) {
            Err(e) => return Err(FtpError::ConnectionError(e)),
            Ok(f) => f
        };
        io::copy(stream, &mut file).map_err(|e| FtpError::ConnectionError(e))
    })?;

    let _ = conn.quit()?;
    debug!("We're done. Ending connection.");
    Ok(())
}

/// Check the integrity of `taxdmp.zip` using `taxdmp.zip.md5`.
pub fn check_integrity(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let path = datadir.join("taxdmp.zip");
    let mut file = File::open(path)?;
    let mut hasher = Context::new();
    debug!("Computing MD5 sum...");
    let _ = io::copy(&mut file, &mut hasher)?;
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

/// Extract all files from taxdmp.zip in the same directory.
pub fn extract_dump(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let path = datadir.join("taxdmp.zip");
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = datadir.join(file.sanitized_name());

        debug!("Extracted {}", outpath.as_path().display());
        let mut outfile = File::create(&outpath)?;
        io::copy(&mut file, &mut outfile)?;
    }
    Ok(())
}

/// Remove the downloaded and extracted files.
pub fn remove_temp_files(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    remove_file(datadir.join("taxdmp.zip"))?;
    remove_file(datadir.join("taxdmp.zip.md5"))?;
    remove_file(datadir.join("citations.dmp"))?;
    remove_file(datadir.join("delnodes.dmp"))?;
    remove_file(datadir.join("division.dmp"))?;
    remove_file(datadir.join("gc.prt"))?;
    remove_file(datadir.join("gencode.dmp"))?;
    remove_file(datadir.join("merged.dmp"))?;
    remove_file(datadir.join("names.dmp"))?;
    remove_file(datadir.join("nodes.dmp"))?;
    remove_file(datadir.join("readme.txt"))?;
    Ok(())
}

//-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-_-
// Database initialization and population

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

/// Initialize a the database by running the CREATE TABLE statements.
pub fn init_db(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let path = datadir.join("taxonomy.db");
    let conn = Connection::open(path)?;
    debug!("Database opened.");
    conn.execute_batch(CREATE_TABLES_STMT)?;
    debug!("Tables created.");
    Ok(())
}

/// Read the names.dmp file and insert the records into the database. When
/// it's done, create the indexes on names and name classes.
pub fn insert_names(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    debug!("Inserting names...");
    let dbpath = datadir.join("taxonomy.db");
    let conn = Connection::open(dbpath)?;
    debug!("Database opened.");

    let dumppath = datadir.join("names.dmp");
    let file = File::open(dumppath)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'|')
        .from_reader(file);

    let mut stmts: Vec<String> = vec![String::from("BEGIN;")];

    debug!("Beginning to read records from names.dmp.");
    for (i, result) in rdr.records().enumerate() {
        if i > 1 && i%10000 == 0 {
            stmts.push(String::from("COMMIT;"));
            let stmt = &stmts.join("\n");
            conn.execute_batch(stmt)?;
            debug!("Read {} records so far.", i);
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
    conn.execute_batch(stmt)?;
    debug!("Done inserting names.");

    debug!("Creating names indexes.");
    conn.execute("CREATE INDEX idx_names_tax_id ON names(tax_id);", NO_PARAMS)?;
    conn.execute("CREATE INDEX idx_names_class ON names(name_class);", NO_PARAMS)?;

    Ok(())
}

/// Read the division.dmp file and insert the records into the database.
pub fn insert_divisions(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    debug!("Inserting divisions...");
    let dbpath = datadir.join("taxonomy.db");
    let conn = Connection::open(dbpath)?;
    debug!("Database opened.");

    let dumppath = datadir.join("division.dmp");
    let file = File::open(dumppath)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'|')
        .from_reader(file);

    let mut stmts: Vec<String> = vec![String::from("BEGIN;")];

    debug!("Beginning to read records from divisions.dmp.");
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
    conn.execute_batch(stmt)?;
    debug!("Done inserting divisions.");

    Ok(())
}

/// Read the gencode.dmp file and insert the records into the database.
pub fn insert_genetic_codes(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    debug!("Inserting genetic codes...");
    let dbpath = datadir.join("taxonomy.db");
    let conn = Connection::open(dbpath)?;
    debug!("Database opened.");

    let dumppath = datadir.join("gencode.dmp");
    let file = File::open(dumppath)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'|')
        .from_reader(file);

    let mut stmts: Vec<String> = vec![String::from("BEGIN;")];

    debug!("Beginning to read records from gencode.dmp.");
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
    conn.execute_batch(stmt)?;
    debug!("Done inserting genetic codes.");

    Ok(())
}

/// Read the nodes.dmp file and insert the records into the database. When
/// it's done, create the index on `parent_tax_id`.
pub fn insert_nodes(datadir: &PathBuf) -> Result<(), Box<dyn Error>> {
    debug!("Inserting nodes...");
    let dbpath = datadir.join("taxonomy.db");
    let conn = Connection::open(dbpath)?;
    debug!("Database opened.");

    let dumppath = datadir.join("nodes.dmp");
    let file = File::open(dumppath)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'|')
        .from_reader(file);

    let mut stmts: Vec<String> = vec![
        String::from("BEGIN;"),
        // Special case: the root
        String::from("INSERT INTO nodes VALUES (1, 1, 'no rank', 8, 0, 0, '');")
    ];

    debug!("Beginning to read records from nodes.dmp.");
    let mut records = rdr.records().enumerate();
    records.next(); // We burn the root row
    for (i, result) in records {
        if i > 0 && i%10000 == 0 {
            stmts.push(String::from("COMMIT;"));
            let stmt = &stmts.join("\n");
            conn.execute_batch(stmt)?;
            debug!("Read {} records so far.", i);
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
    conn.execute_batch(stmt)?;
    debug!("Done inserting nodes.");

    debug!("Creating nodes indexes.");
    conn.execute("CREATE INDEX idx_nodes_parent_id ON nodes(parent_tax_id);", NO_PARAMS)?;

    Ok(())
}
