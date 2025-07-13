# cargo-countlines

**Quickly and easily count lines of code in any file or directory**

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

cargo-countlines is a simple command-line tool that allows you to count lines of code in
any file or by recursively traversing any directory.

- Count SLOC in all major programming languages
- Count code, comments, and blank lines separately
- Define your own languages by providing a JSON file
- Choose between single-threaded, async, or parallel counting for maximum performance
- Exclude any directories or files using unix glob syntax
- Optional progress information while counting
- Option to follow symbolic links
- Option to restrict the maximum recursion depth
- Option to produce machine-readable output

### Examples

Count SLOC in this crate, excluding the `target/` directory.
```
$ cargo countlines --exclude "target" .

╭───────┬───────┬──────┬─────────┬───────┬─────────╮
│       │ files │ code │ comment │ blank │ invalid │
├───────┼───────┼──────┼─────────┼───────┼─────────┤
│ Rust  │     5 │  736 │      20 │   128 │       0 │
│ JSON  │     2 │  163 │       0 │     0 │       0 │
│ Toml  │     1 │   33 │       0 │     3 │       0 │
├───────┼───────┼──────┼─────────┼───────┼─────────┤
│ Total │     8 │  932 │      20 │   131 │       0 │
╰───────┴───────┴──────┴─────────┴───────┴─────────╯
0 files errored
results in 3.889917ms
```

Count SLOC single-threaded, following symbolic links, recursing to a depth of 3,
excluding python and ruby files in an arbitrary directory and produce machine-readable output.
```
$ cargo countlines -m sync --follow-links -e "*.py" -e "*.rb" -d 3 --machine-readable /home/me/my_dir

 HTML         10  13,777     40  1,258  0
 Toml        126   3,380  1,856    702  0
 Perl         24   2,119  1,168    347  0
 Shell        12     977    232    154  0
 Java          5     676    393    122  0
 JSON         11     635      0      6  0
 JavaScript    9     464    191     87  0
 Haskell      14     429     38    124  0
 Rust          9     418     97     76  0
 C            12     366     12    102  0
 Go            7     305     40     66  0
 XML          11     296      0      0  0
 PHP           1     136     29     32  0
 CSS           1      93      0     16  0
 D             2      54      0     10  0
 C++           2      48      0      8  0
```
