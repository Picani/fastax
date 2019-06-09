Fastax
======

Fastax is a command-line tool that makes phylogenetic trees and lineages
from the NCBI Taxonomy database. It uses a local copy of the database,
which makes it really fast.

By default, all results are pretty-printed. In addition, it can output trees
as [Newick][1] and lineages as CSV.

It can also be used to get information about some taxa like there alternative
scientific names or the genetic code they use.

Installation
------------

Fastax is written in [Rust][2], which makes it safe, fast and portable. The
code is managed using [Cargo][3], then all you should have to do is install it
(see the [Cargo documentation][4]) and run the following:

```
$ git clone https://github.com/Picani/fastax.git
$ cd fastax
$ cargo build --release
```

Et voilà ! The executable file is `target/release/fastax`. Just move it
somewhere on your `PATH`.

Populate the local database
---------------------------

First, you need to get the local copy of the NCBI Taxonomy database.

    $ fastax populate -ve plop@example.com
    
`populate` will download the latest database dumps, extract them, and load
them in a local SQLite database. `-v` asks fastax to tell what it's doing.
`-e` asks to connect to the NCBI with that email address. Note that giving
your email is optional but preferred.

The database is located in a `fastax` folder inside your local data folder,
which should be `$HOME/.local/share`.

Usage
-----

You can get general information about a node:

```
$ fastax show 4932
Saccharomyces cerevisiae - species
----------------------------------
NCBI Taxonomy ID: 4932
Same as:
* Saccharomyces capensis
* Saccharomyces italicus
* Saccharomyces oviformis
* Saccharomyces uvarum var. melibiosus
Commonly named baker's yeast.
Also known as:
* S. cerevisiae
* brewer's yeast
Part of the Plants and Fungi.
Uses the Standard genetic code.
Its mitochondria use the Yeast Mitochondrial genetic code.
```

You can get the lineage of a node:

```
$ fastax lineage 4932
root
  └┬─ no rank: cellular organisms (taxid: 131567)
   └┬─ superkingdom: Eukaryota (taxid: 2759)
    └┬─ no rank: Opisthokonta (taxid: 33154)
     └┬─ kingdom: Fungi (taxid: 4751)
      └┬─ subkingdom: Dikarya (taxid: 451864)
       └┬─ phylum: Ascomycota (taxid: 4890)
        └┬─ no rank: saccharomyceta (taxid: 716545)
         └┬─ subphylum: Saccharomycotina (taxid: 147537)
          └┬─ class: Saccharomycetes (taxid: 4891)
           └┬─ order: Saccharomycetales (taxid: 4892)
            └┬─ family: Saccharomycetaceae (taxid: 4893)
             └┬─ genus: Saccharomyces (taxid: 4930)
              └── species: Saccharomyces cerevisiae (taxid: 4932)
```

You can get a phylogenetic tree:

```
$ fastax tree 562 4932 7227 9606 10090
 ─┬─ no rank: root
  └─┬─ no rank: cellular organisms
    ├─┬─ no rank: Opisthokonta
    │ ├─┬─ no rank: Bilateria
    │ │ ├─┬─ superorder: Euarchontoglires
    │ │ │ ├── species: Mus musculus
    │ │ │ └── species: Homo sapiens
    │ │ └── species: Drosophila melanogaster
    │ └── species: Saccharomyces cerevisiae
    └── species: Escherichia coli
```

The same tree in Newick:

```
$ fastax tree -n 562 4932 7227 9606 10090
(root,(cellular organisms,(Escherichia coli,Opisthokonta,(Saccharomyces cerevisiae,Bilateria,(Drosophila melanogaster,Euarchontoglires,(Homo sapiens,Mus musculus))))));
```


[1]: http://evolution.genetics.washington.edu/phylip/newicktree.html
[2]: https://www.rust-lang.org
[3]: https://crates.io
[4]: https://doc.rust-lang.org/cargo/getting-started/installation.html
