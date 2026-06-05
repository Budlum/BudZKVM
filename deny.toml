[graph]
targets = []

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "BSD-2-Clause",
    "CC0-1.0",
    "ISC",
    "OpenSSL",
    "Zlib",
]
deny = [
    "GPL-1.0",
    "GPL-2.0",
    "GPL-3.0",
    "AGPL-3.0",
]
copyleft = "deny"
allow-osi-fsf-free = "either"
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
deny = []
skip = []
skip-tree = []

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
unsound = "warn"
notice = "warn"
ignore = []
