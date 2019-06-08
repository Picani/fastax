#[macro_use]
extern crate log;
extern crate structopt;
extern crate fastax;

use std::process;
use structopt::StructOpt;


fn main() {
    let opt = fastax::Opt::from_args();

    if let Err(e) = fastax::run(opt) {
        error!("{}", e);
    }
    process::exit(exitcode::OK);
}
