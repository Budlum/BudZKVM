# Adding an Opcode

Bu rehber, BudZKVM'e yeni bir opcode eklerken takip edilmesi gereken adımları checklist formatında sunar. Amaç, ISA, VM, trace, AIR ve proof stack sözleşmesini bozmadan, deterministik ve sound bir şekilde opcode eklemektir.

## Mevcut Durum

BudZKVM'de **31 opcode** tanımlıdır. Bunlardan **30'u production** (AIR constraint'leri tamamlanmış), **1'i experimental** (`VerifyMerkle`) durumdadır. Production opcode'lar `default` feature ile derlendiğinde kullanılabilir; experimental opcode'lar `cfg(feature = "experimental")` gerektirir.

## 1. ISA Yüzeyini Tanımla

`bud-isa/src/lib.rs` dosyasını güncelle:

- `Opcode` enum'una yeni varyant ekle.
- Kararlı bir discriminant (0x1F'den başlayarak) ata.
- `Instruction::decode_any()` metodunda raw byte → yeni varyant eşlemesini ekle.
- Eğer opcode experimental olacaksa `Opcode::is_experimental()` listesine ekle.
- Encoding/decoding testi yaz.

**Kural:** Bytecode artifact'ları bu discriminant'a bağımlı olabileceği için discriminant'ı kararlı tut. Değer experimental ise bu durumu `docs/02_isa_ve_bytecode.md` dosyasında belgele.

## 2. VM Semantiğini Uygula

`bud-vm/src/lib.rs` dosyasını güncelle:

- `Vm::step()` metodunda yeni opcode için bir kol (arm) ekle.
- Register okumalarını, register yazmalarını, `dst_val` ve `next_pc`'i tanımla.
- Opcode'un memory, storage, stack, gas ve halt davranışıyla nasıl etkileştiğine karar ver.
- `Vm::gas_cost()` metodunda gas maliyetini belirle.
- Normal davranış ve edge case'ler için VM testleri ekle.

VM trace'i, AIR'in bu adımı doğrulayabilmesi için yeterli bilgi içermelidir. Eğer AIR'in yeni bir witness değerine ihtiyacı varsa, bunu `Step` struct'ına ve trace matrix'e bilinçli olarak ekle.

## 3. Derleyici veya CLI'dan Yayınla

Eğer opcode kullanıcıya dönükse, derleyici pipeline'ını güncelle:

- `bud-compiler/src/ast.rs` ve `parser.rs`: AST/payload değişiklikleri.
- `bud-compiler/src/sema.rs`: Semantik doğrulama.
- `bud-compiler/src/codegen.rs`: Bytecode üretimi.
- Gerekirse CLI örnekleri veya fixture'lar.

Opcode'lar dil tarafından sunulmadan önce VM'de var olabilir, ancak dokümanlar opcode'un internal, experimental veya stable olduğunu belirtmelidir.

## 4. Trace Sütunları veya Selector'lar Ekle

`bud-proof/src/plonky3_air.rs` ve `bud-proof/src/plonky3_prover.rs` dosyalarını güncelle:

- Yalnızca mevcut selector'lar opcode'u temsil edemiyorsa yeni selector sütunu ekle.
- `trace_matrix()` fonksiyonunda selector'ü doldur.
- Yeni witness sütunlarını doldur.
- Trace padding ve halt satırlarını tutarlı tut.
- Opcode yeni okuma/yazma getiriyorsa register, memory veya lookup event'lerini güncelle.

**Mevcut trace genişliği: 354 sütun.** Yeni sütunları şu gruplardan sonra ekle:
- 0-64: Temel + Selector + Register + Memory + Soundness
- 65-257: Comparison + Bitwise witness
- 258-353: Poseidon witness

Her yeni sütun, kararlı hale gelmeden önce trace schema dokümanında (`docs/vm_trace_schema.md`) net bir anlama sahip olmalıdır.

## 5. AIR Constraint'lerini Ekle

`BudAir::eval()` içinde:

- Opcode'a özgü denklemleri opcode selector'ü ile gate'le (`builder.when(is_my_opcode)`).
- `next_pc` davranışını constrain et.
- Hedef değerleri ve yan etkileri constrain et.
- Değer küçük veya binary olacaksa boolean/range constraint ekle.
- Opcode ortak tabloları okuyor veya yazıyorsa permutation/lookup constraint'lerini güncelle.

**Kritik kural:** Constraint sadece dürüst trace'i kabul etmemeli, tahrif edilmiş trace'i de **reddetmelidir**. Her constraint için negatif test yaz.

## 6. Test Ekle

Minimum:

- `bud-isa`: Opcode için encoding/decoding coverage.
- `bud-vm`: Execution coverage (normal + edge case).
- `bud-proof`: Pozitif prover testi (prove + verify).
- AIR tahrif edilmiş witness'ı reddetmesi gerekiyorsa negatif prover/verifier testi.
- BudL opcode'u üretiyorsa compiler snapshot veya entegrasyon testi.

Test pattern'i (bud-proof):
```rust
#[test]
fn proves_my_new_opcode() {
    let program = vec![
        inst(Opcode::MyNewOpcode, 1, 2, 3, 0),
        inst(Opcode::Halt, 0, 0, 0, 0),
    ];
    prove_and_verify(program, |vm| {
        vm.registers[2] = input_a;
        vm.registers[3] = input_b;
    });
}
```

## 7. Dokümantasyonu Güncelle

Güncellenmesi gereken dosyalar:

- `docs/02_isa_ve_bytecode.md` — Opcode formatı, discriminant ve stabilite durumu.
- `docs/vm_trace_schema.md` — Yeni trace sütunları eklendiyse.
- `docs/03_virtual_machine.md` — VM semantiği değiştiyse.
- `docs/09_faz0_stabilizasyon.md` — Yeni production opcode'u eklendiyse.
- `README.md` — Roadmap durumu.

Değişikliği göndermeden önce `docs/development.md`'deki yerel CI eşdeğerini çalıştır:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo check --workspace --all-targets
nix develop --command cargo clippy --workspace --all-targets -- -D warnings
nix develop --command cargo test --workspace
```
