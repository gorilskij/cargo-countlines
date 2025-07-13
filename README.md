# cargo-countlines

**Quickly and easily count lines of code in any file or directory**

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

cargo-countlines is a simple command-line tool that allows you to count lines of code in
any file or by recursively traversing any directory.

- Count SLOC in all major programming languages
- Count code, comments, and blank lines separately
- Define your own languages by providing a JSON file
- Choose between single-threaded, async, or parallel counting for maximum performance
- Optional progress information while counting
- Option to follow symbolic links
- Option to restrict the maximum recursion depth
- Option to produce machine-readable output

### Examples

Count SLOC in this crate, excluding the `target/` directory.
```
cargo countlines --exclude "target" .

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
