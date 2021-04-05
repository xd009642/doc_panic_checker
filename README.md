# doc-panic-checker

[![Build Status](https://github.com/xd009642/doc_panic_checker/workflows/Build/badge.svg)](https://github.com/xd009642/doc_panic_checker/actions)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Coverage Status](https://coveralls.io/repos/github/xd009642/doc_panic_checker/badge.svg?branch=main)](https://coveralls.io/github/xd009642/doc_panic_checker?branch=main)

This is a quick tool I wrote to find public methods or functions which could
panic and where that panic hasn't been documented. 

## Usage

```
$ doc_panic_checker --help
doc_panic_checker 0.1.0

USAGE:
    doc_panic_checker [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --color <color>                         [default: auto]
        --exclude-files <excluded-files>...    
        --manifest-path <manifest-path>        
```

Running `doc_panic_checker` on itself gives this output, where we can clearly
see the source files and the functions or methods that could panic and aren't
documented. It also shows a line range for the affected region which is rather
imprecise due to laziness in walking through the AST but could be made more
precise.

```
  INFO Analysing project in /home/daniel/personal/doc_panic_checker
  WARN Potentially undocumented panics in src/ast_walker.rs
	AstWalker::process 63:71
  WARN Potentially undocumented panics in src/main.rs
	get_analysis 32:40
	setup_logging 64:91
```

## License

This project is currently licensed under the terms of both the MIT license and
the Apache License (Version 2.0). See LICENSE-MIT and LICENSE-APACHE for more 
details.

