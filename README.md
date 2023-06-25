# SCRU128 Generator Tester

[![GitHub tag](https://img.shields.io/github/v/tag/scru128/gen_test)](https://github.com/scru128/gen_test)
[![License](https://img.shields.io/github/license/scru128/gen_test)](https://github.com/scru128/gen_test/blob/main/LICENSE)

A command-line SCRU128 tester that tests if a generator generates monotonically
ordered IDs, sets up-to-date timestamps, fills randomness bits with random
numbers, resets the per-second randomness field every second, and so on.

## Usage

```bash
any-command-that-prints-identifiers-infinitely | scru128-test
```

## Installation

[Install Rust](https://www.rust-lang.org/tools/install) and build from source:

```bash
cargo install --git https://github.com/scru128/gen_test.git
```

## License

Copyright 2021-2023 LiosK

Licensed under the Apache License, Version 2.0.

## See also

- [SCRU128 Specification](https://github.com/scru128/spec)
