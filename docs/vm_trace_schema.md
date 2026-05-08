# BudVM Trace Schema

Bu doküman `bud-vm` tarafından üretilen `Step` kayıtlarının ne anlama geldiğini sabitler. Phase 2'nin amacı VM davranışını prover için güvenilir bir kaynak haline getirmektir. Bu yüzden trace sadece debug log değildir; AIR tarafındaki tablo satırlarının ham girdisidir.

## Temel Kural

`Vm::step(program)` bir instruction gerçekten fetch edip execute ederse tam olarak bir `Step` üretir.

Şu durumlarda yeni trace satırı üretilmez:

* VM zaten `halted == true` durumundaysa.
* `pc >= program.len()` ise.

Bu ikinci durum program counter'ın program dışına çıkması için tanımlı davranıştır: VM deterministik biçimde halt eder, fakat sahte bir instruction satırı eklemez.

## Step Alanları

| Alan | Anlamı |
| --- | --- |
| `pc` | Instruction fetch edilmeden önceki program counter. |
| `next_pc` | Instruction execute edildikten sonra beklenen sonraki program counter. |
| `instruction` | Decode edilmiş `bud_isa::Instruction`. |
| `src1_idx` | `rs1` register index'i. |
| `src2_idx` | `rs2` register index'i. |
| `dst_idx` | `rd` register index'i. |
| `src1_val` | Instruction execute edilmeden önce okunan `rs1` değeri. |
| `src2_val` | Instruction execute edilmeden önce okunan `rs2` değeri. |
| `dst_val` | Instruction'ın hesapladığı sonuç değeri. Yazma yapmayan opcode'larda semantik olarak `0` veya kontrol değeri olabilir. |
| `registers` | Instruction execute edildikten sonraki 32 register'lık snapshot. |

## Program Counter Semantiği

Normal ALU, memory, storage, syscall ve log instruction'ları için:

```text
next_pc = pc + 1
```

`Halt` için:

```text
next_pc = pc
halted = true
```

`Jmp`, `Jnz`, `Call` ve `Ret` kendi kontrol-flow kurallarını uygular:

* `Jmp`: `pc + imm`
* `Jnz`: `rs1 != 0` ise `pc + imm`, aksi halde `pc + 1`
* `Call`: dönüş adresini stack'e koyar, sonra `pc + imm`
* `Ret`: dönüş adresini stack'ten alır

Eğer branch veya jump program dışına çıkarsa bir sonraki `step` VM'i halt eder ve ek trace satırı üretmez.

## Gas Semantiği

Her fetch edilen instruction execute edilmeden önce gas tüketir. `Halt` maliyeti `0` kabul edilir.

Mevcut maliyet grupları:

| Opcode grubu | Gas |
| --- | ---: |
| `Halt` | 0 |
| Basit ALU ve branch opcode'ları | 1 |
| `Call`, `Ret`, `Push`, `Pop` | 2 |
| `Load`, `Store`, `SRead`, `SWrite` | 3 |
| `Syscall` | 5 |
| `Poseidon`, `VerifyMerkle` | 10 |

`gas_used > gas_limit` olursa VM `Out of gas` hatasıyla durur. Bu davranış sonsuz döngüleri deterministik biçimde kesmek için vardır.

## Memory Semantiği

`Load` iki modda çalışır:

* `rs1 == 0`: `imm` immediate değer olarak yüklenir.
* `rs1 != 0`: `register[rs1] + imm` adresinden 8 byte little-endian word okunur.

Memory erişimi geçersizse `Load` sonucu `0` olur.

Geçersiz memory erişimleri:

* Negatif adres.
* `usize` içine sığmayan adres.
* `addr + 8` taşması.
* `addr + 8 > memory.len()`.

`Store` aynı adres kurallarını kullanır. Adres geçersizse no-op olur ve memory değişmez.

## Register Semantiği

Instruction içindeki `rd`, `rs1` ve `rs2` alanları ISA decode aşamasında 5 bit ile maskelenir. Bu yüzden normal register okumaları ve yazmaları `0..32` aralığındadır.

`VerifyMerkle` path register'ını `imm` üzerinden seçer. Bu immediate negatifse veya `0..32` dışında kalıyorsa path değeri `0` kabul edilir. Böylece kötü bytecode VM'i index panic ile nondeterministik biçimde düşürmez.

## Arithmetic Semantiği

BudVM aritmetiği `u64` üzerinde wrapping davranışı kullanır:

* `Add`: `wrapping_add`
* `Sub`: `wrapping_sub`
* `Mul`: `wrapping_mul`
* `Poseidon` placeholder hesabı da wrapping işlemler kullanır

Bu karar prover tarafı için önemlidir. AIR ve testler Rust debug/release overflow farkına bağlı kalmamalıdır.

## Halt Sonrası Davranış

`Halt` execute edildikten sonra:

* `halted = true`
* `pc` aynı kalır
* `next_pc = pc`
* Sonraki `step` çağrıları trace'e yeni satır eklemez
* Register ve memory değişmez

Bu davranış `COL_IS_HALT` kısıtları güçlendirildiğinde prover tarafındaki terminasyon modelinin temelini oluşturur.

## Prover Bağlantısı

`bud-proof/src/plonky3_prover.rs`, `Vec<Step>` değerlerini `RowMajorMatrix<Goldilocks>` formatına çevirir. Bu yüzden `Step` alanları şu prover sütunlarının ana kaynağıdır:

* `pc` -> `COL_PC`
* `next_pc` -> `COL_NEXT_PC`
* `instruction.opcode` -> `COL_OPCODE` ve selector sütunları
* `instruction.rd`, `instruction.rs1`, `instruction.rs2` -> register index sütunları
* `src1_val`, `src2_val`, `dst_val` -> operand ve sonuç sütunları
* `registers` snapshot'ı -> register event üretiminde başlangıç ve geçiş bağlamı
* `memory_addr`, `memory_val`, `is_memory_write` -> memory event üretiminin ana kaynağı

## Prover Trace Sütunları (Main Trace)

Prover tarafında oluşturulan ana matris (Main Trace) şu anda **55 sütun** genişliğindedir:

| Aralık | Grup | Tanım |
| --- | --- | --- |
| 0 - 10 | Temel | PC, Next PC, Opcode, Register Index/Değerleri, Immediate |
| 11 - 22 | CPU Selectors | `IS_ADD`, `IS_SUB`, `IS_LOAD`, `IS_HALT`, `JNZ_COND` vb. |
| 23 - 28 | Register Table | `REG_CLK`, `REG_IDX`, `REG_VAL`, `REG_IS_WRITE` vb. |
| 29 - 48 | Genişletilmiş Selectors | `IS_DIV`, `IS_AND`, `IS_STORE`, `IS_CALL`, `POSEIDON` vb. |
| 49 - 54 | Memory Table | `MEM_CLK`, `MEM_ADDR`, `MEM_VAL`, `MEM_IS_WRITE`, `MEM_ACTIVE`, `MEM_SAME` |

Hafıza (Memory) tablosu, CPU'nun yaptığı tüm `Load` ve `Store` işlemlerini toplar, bunları adres (`MEM_ADDR`) ve zaman (`MEM_CLK`) sırasına göre dizer. Bu sayede AIR kısıtları, aynı adrese yapılan okumanın bir önceki yazılan değerle aynı olduğunu (`MEM_SAME` kısıtıyla) kolayca denetleyebilir.

## Yardımcı İz (Auxiliary Trace) Şeması

BudZKVM, Cross-Table Lookup (CTL) işlemlerini doğrulamak için LogUp Fractional Sums yöntemini kullanır. Bu amaçla ana matrise ek olarak iki sütunlu bir yardımcı iz matrisi üretilir:

| Sütun | Adı | Tanım |
| --- | --- | --- |
| 0 | `S_REG` | Register tutarlılığı için LogUp kesirli toplam toplayıcısı. |
| 1 | `S_MEM` | Hafıza (Memory) tutarlılığı için LogUp kesirli toplam toplayıcısı. |

Bu sütunlar Fiat-Shamir transcript'inden gelen $\alpha, \beta$ (tuple paketleme) ve $\gamma$ (kesirli payda) değerlerine bağlıdır. Her satırda $S_{i+1} = S_i + \sum \frac{w_j}{\gamma - C_j}$ kuralı işletilir. Program sonunda her iki sütunun da `0` olması zorunludur.

Trace schema değişirse hem VM testleri hem prover testleri birlikte güncellenmelidir.

## Fixture Testleri

`bud-vm/tests/trace_fixtures.rs` dosyası trace şemasını örnek programlar üzerinden sabitler. Bu testler üç ana akışı kapsar:

* Aritmetik trace: `Load`, `Add`, `Sub`, `Mul`, `Halt`.
* Kontrol akışı trace'i: `Jnz`, `Jmp` ve program dışına çıkınca deterministik halt.
* Memory/storage/event trace'i: `Store`, memory `Load`, `SWrite`, `SRead`, `Log`.

Bu fixture'lar özellikle refactor sırasında değerlidir. VM kodu değiştiğinde testler sadece final register sonucuna değil, ara `Step` satırlarının `pc`, `next_pc`, operand değerleri ve register snapshot'larına da bakar.
