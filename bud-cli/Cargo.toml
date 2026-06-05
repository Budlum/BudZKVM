[package]
name = "bud-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
bud-isa = { path = "../bud-isa" }
bud-vm = { path = "../bud-vm" }
bud-compiler = { path = "../bud-compiler" }
bud-proof = { path = "../bud-proof" }
bud-state = { path = "../bud-state" }
clap = { version = "4.0", features = ["derive"] }
hex = "0.4"
serde_json = "1.0"
tiny-keccak = { version = "2.0", features = ["keccak"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[features]
experimental = ["bud-isa/experimental", "bud-vm/experimental", "bud-compiler/experimental"]

