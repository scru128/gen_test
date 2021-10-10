# SCRU128 Generator Tester

A command-line SCRU128 tester that tests if a generator generates monotonically
ordered IDs, fills randomness bits with random numbers, resets the per-second
randomness field every second, and so on.

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

Copyright 2021 LiosK

Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the
License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied. See the License for the
specific language governing permissions and limitations under the License.
