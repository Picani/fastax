Fastax
======

![crates.io badge](https://img.shields.io/crates/v/fastax?color=green)


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
code is managed using [Cargo][3] and published on [crates.io][4]. If Cargo
is already installed, just open a terminal and type:

```
$ cargo install fastax
```

Et voilà !

Alternatively, you can compile it from sources:

```
$ git clone https://github.com/Picani/fastax.git
$ cd fastax
$ cargo build --release
```

The executable file is `target/release/fastax`. Just move it somewhere on
your `PATH`.

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

For each command, you need to query at least one node. The term used to get
a node can be either its unique NCBI Taxonomy ID (so called taxid), its
binomial scientific name or its binomial scientific name with the two part
separated by an underscore (the character `_`). This last option is useful
for scripting.

Note also that for some species, multiple binomial scientific names are in
use. Fastax looks for each of them.

### The `show` command

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

or:

```
$ fastax show "Homo sapiens"
Homo sapiens - species
----------------------
NCBI Taxonomy ID: 9606
Commonly named human.
Also known as:
* man
First description:
* Homo sapiens Linnaeus, 1758
Part of the Primates.
Uses the Standard genetic code.
Its mitochondria use the Vertebrate Mitochondrial genetic code.
```

or also:

```
$ fastax show Tyrannosaurus_rex
Tyrannosaurus rex - species
---------------------------
NCBI Taxonomy ID: 436495
Part of the Vertebrates.
Uses the Standard genetic code.
Its mitochondria use the Vertebrate Mitochondrial genetic code.
```


### The `lineage` command

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

The same lineage in CSV:

```
$ fastax lineage Saccharomyces_cerevisiae
no rank:root:1,no rank:cellular organisms:131567,superkingdom:Eukaryota:2759,no rank:Opisthokonta:33154,kingdom:Fungi:4751,subkingdom:Dikarya:451864,phylum:Ascomycota:4890,no rank:saccharomyceta:716545,subphylum:Saccharomycotina:147537,class:Saccharomycetes:4891,order:Saccharomycetales:4892,family:Saccharomycetaceae:4893,genus:Saccharomyces:4930,species:Saccharomyces cerevisiae:4932
```


### The `tree` command

You can get a phylogenetic tree:

```
$ fastax tree "Escherichia coli" 4932 Drosophila_melanogaster 9606 "Mus musculus"
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

With `-f/--format`, you can also change the default node formatting:

```
$ fastax tree -f "%taxid (%name)" "Escherichia coli" 4932 Drosophila_melanogaster 9606 "Mus musculus"
 ─┬─ 1 (root)
  └─┬─ 131567 (cellular organisms)
    ├─┬─ 33154 (Opisthokonta)
    │ ├─┬─ 33213 (Bilateria)
    │ │ ├─┬─ 314146 (Euarchontoglires)
    │ │ │ ├── 10090 (Mus musculus)
    │ │ │ └── 9606 (Homo sapiens)
    │ │ └── 7227 (Drosophila melanogaster)
    │ └── 4932 (Saccharomyces cerevisiae)
    └── 562 (Escherichia coli)
```

The available tags are

* `%name` which is replaced by the scientific name,
* `%rank` which is replaced by the rank,
* `%taxid` which is replaced by the NCBI Taxonomy ID.

By default, the nodes with only one child are hidden. You can show them with
the `-i/--internal` option:

```
$ fastax tree -i Mus_musculus Rattus_norvegicus
 ─┬─ no rank: root
  └─┬─ no rank: cellular organisms
    └─┬─ superkingdom: Eukaryota
      └─┬─ no rank: Opisthokonta
        └─┬─ kingdom: Metazoa
          └─┬─ no rank: Eumetazoa
            └─┬─ no rank: Bilateria
              └─┬─ no rank: Deuterostomia
                └─┬─ phylum: Chordata
                  └─┬─ subphylum: Craniata
                    └─┬─ no rank: Vertebrata
                      └─┬─ no rank: Gnathostomata
                        └─┬─ no rank: Teleostomi
                          └─┬─ no rank: Euteleostomi
                            └─┬─ superclass: Sarcopterygii
                              └─┬─ no rank: Dipnotetrapodomorpha
                                └─┬─ no rank: Tetrapoda
                                  └─┬─ no rank: Amniota
                                    └─┬─ class: Mammalia
                                      └─┬─ no rank: Theria
                                        └─┬─ no rank: Eutheria
                                          └─┬─ no rank: Boreoeutheria
                                            └─┬─ superorder: Euarchontoglires
                                              └─┬─ no rank: Glires
                                                └─┬─ order: Rodentia
                                                  └─┬─ suborder: Myomorpha
                                                    └─┬─ no rank: Muroidea
                                                      └─┬─ family: Muridae
                                                        └─┬─ subfamily: Murinae
                                                          ├─┬─ genus: Rattus
                                                          │ └── species: Rattus norvegicus
                                                          └─┬─ genus: Mus
                                                            └─┬─ subgenus: Mus
                                                              └── species: Mus musculus
```


### The `subtree` command

You can get the phylogenetic tree of the children of a node:

```
$ fastax subtree Homininae
 ─┬─ subfamily: Homininae
  ├─┬─ genus: Homo
  │ ├── species: Homo heidelbergensis
  │ └─┬─ species: Homo sapiens
  │   ├── subspecies: Homo sapiens subsp. 'Denisova'
  │   └── subspecies: Homo sapiens neanderthalensis
  ├─┬─ genus: Pan
  │ ├─┬─ species: Pan troglodytes
  │ │ ├── subspecies: Pan troglodytes verus x troglodytes
  │ │ ├── subspecies: Pan troglodytes ellioti
  │ │ ├── subspecies: Pan troglodytes vellerosus
  │ │ ├── subspecies: Pan troglodytes verus
  │ │ ├── subspecies: Pan troglodytes troglodytes
  │ │ └── subspecies: Pan troglodytes schweinfurthii
  │ └── species: Pan paniscus
  └─┬─ genus: Gorilla
    ├─┬─ species: Gorilla beringei
    │ ├── subspecies: Gorilla beringei beringei
    │ └── subspecies: Gorilla beringei graueri
    └─┬─ species: Gorilla gorilla
      ├── subspecies: Gorilla gorilla diehli
      ├── subspecies: Gorilla gorilla uellensis
      └── subspecies: Gorilla gorilla gorilla
```

If you only want the species:

```
$ fastax subtree -s Homininae
 ─┬─ subfamily: Homininae
  ├─┬─ genus: Homo
  │ ├── species: Homo heidelbergensis
  │ └── species: Homo sapiens
  ├─┬─ genus: Pan
  │ ├── species: Pan troglodytes
  │ └── species: Pan paniscus
  └─┬─ genus: Gorilla
    ├── species: Gorilla beringei
    └── species: Gorilla gorilla
```

The same tree in newick:

```
$ fastax subtree -sn Homininae
(Homininae,(Homo,(Homo sapiens,Homo heidelbergensis),Gorilla,(Gorilla beringei,Gorilla gorilla),Pan,(Pan paniscus,Pan troglodytes)));
```

As with the `tree` command, you can format the node with the `-f/--format`
option, and show the internal nodes with the `-i/--internal` option. See
above for more information.

License
-------

Copyright © 2019 Sylvain PULICANI picani@laposte.net

This work is free. You can redistribute it and/or modify it under the terms
of the MIT license. See the `LICENSE` file for more details.


[1]: http://evolution.genetics.washington.edu/phylip/newicktree.html
[2]: https://www.rust-lang.org
[3]: https://crates.io
[4]: https://crates.io/crates/fastax
