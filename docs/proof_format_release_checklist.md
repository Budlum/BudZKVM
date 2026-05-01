# Proof Format Release Checklist

Use this checklist whenever serialized proof bytes, verifier expectations, transcript ordering, or
public proof API shapes change.

## Scope the Change

- Identify every affected type in `bud-proof/src/bud_stark/proof.rs`.
- Identify transcript changes in prover/verifier code.
- Identify commitment, opening, public input, preprocessed trace, and auxiliary trace shape changes.
- Decide whether old proof bytes should be rejected or remain supported.

## Compatibility

- Add or update proof serialization round-trip tests.
- Add malformed proof tests for invalid byte payloads and wrong proof shapes.
- If backward compatibility is required, add a migration or version-dispatch path.
- If backward compatibility is not required, document the breaking change.

## Versioning

- Add or update the proof-format version before treating bytes as stable.
- Include the Plonky3 version and backend assumptions in release notes.
- Record whether the change affects CLI, L1 integration, or node-facing APIs.

## Release Gates

Run:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo check
nix develop --command cargo test
nix develop --command cargo test -p bud-proof
nix develop --command python3 scripts/check_docs_links.py
```

## Release Notes

Release notes should include:

- Proof-format version before and after the change.
- Whether existing proof bytes still verify.
- Any new public values or transcript observations.
- Any verifier failure mode that changed.
- Required downstream updates for CLI, state, node, or L1 integration.
