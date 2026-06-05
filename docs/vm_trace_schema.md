# BudVM Trace Schema v2

Bu doküman `bud-vm` tarafından üretilen `Step` kayıtlarının AIR trace matrisine nasıl aktarıldığını ve trace sütunlarının yapısını sabitler. Prover tarafının AIR constraint'leri tam olarak bu şemaya göre yazılmıştır.

## Temel Kural

`Vm::step(program)` bir instruction gerçekten fetch edip execute ederse tam olarak bir `Step` üretir. Şu durumlarda yeni trace satırı üretilmez:

* VM zaten `halted == true` durumundaysa.
* `pc >= program.len()` ise.

## Step Alanları

| Alan | Anlamı |
| --- | --- |
| `pc` | Instruction fetch edilmeden önceki program counter |
| `next_pc` | Instruction execute edildikten sonra beklenen sonraki program counter |
| `instruction` | Decode edilmiş `bud_isa::Instruction` |
| `src1_idx` | `rs1` register index'i |
| `src2_idx` | `rs2` register index'i |
| `dst_idx` | `rd` register index'i |
| `src1_val` | Instruction execute edilmeden önce okunan `rs1` değeri |
| `src2_val` | Instruction execute edilmeden önce okunan `rs2` değeri |
| `dst_val` | Instruction'ın hesapladığı sonuç değeri |
| `registers` | Instruction execute edildikten sonraki 32 register'lık snapshot |
| `memory_addr` | Bellek erişim adresi (Load/Store için) |
| `memory_val` | Bellekten okunan veya yazılan değer |
| `is_memory_write` | Yazma işlemi mi? |
| `stack_pointer` | Stack işaretçisinin güncel değeri |

## Prover Trace Sütunları (Ana Matris — Main Trace)

Prover tarafında oluşturulan ana matris **354 sütun** genişliğindedir. Sütunlar şu gruplara ayrılır:

### Temel Sütunlar (0-10)
| İndeks | Sütun | Açıklama |
|--------|-------|----------|
| 0 | `CLK` | Satır sayacı (clock) |
| 1 | `PC` | Program counter |
| 2 | `OPCODE` | İşlem kodu (0x00-0x1E) |
| 3 | `RD_IDX` | Hedef register index'i |
| 4 | `RS1_IDX` | Birinci kaynak register index'i |
| 5 | `RS2_IDX` | İkinci kaynak register index'i |
| 6 | `RS1_VAL` | Birinci kaynak register değeri |
| 7 | `RS2_VAL` | İkinci kaynak register değeri |
| 8 | `RD_VAL_NEW` | Hesaplanan sonuç değeri |
| 9 | `NEXT_PC` | Beklenen sonraki PC |
| 10 | `IMM` | Immediate değer (i32 olarak) |

### CPU Selector Sütunları (11-22)
| İndeks | Sütun | Açıklama |
|--------|-------|----------|
| 11 | `IS_ADD` | Add opcode selector'ü |
| 12 | `IS_SUB` | Sub selector'ü |
| 13 | `IS_MUL` | Mul selector'ü |
| 14 | `IS_EQ` | Eq selector'ü |
| 15 | `IS_LT` | Lt selector'ü |
| 16 | `IS_JMP` | Jmp selector'ü |
| 17 | `IS_JNZ` | Jnz selector'ü |
| 18 | `IS_LOAD` | Load selector'ü |
| 19 | `IS_HALT` | Halt selector'ü |
| 20 | `IS_ASSERT` | Assert selector'ü |
| 21 | `IS_LOG` | Log selector'ü |
| 22 | `JNZ_COND` | Jnz koşul değeri (1 = atla, 0 = geç) |

### Register Table Sütunları (23-28)
| İndeks | Sütun | Açıklama |
|--------|-------|----------|
| 23 | `REG_CLK` | Register event clock'u |
| 24 | `REG_IDX` | Register index'i |
| 25 | `REG_VAL` | Register değeri |
| 26 | `REG_IS_WRITE` | Yazma event'i mi? |
| 27 | `REG_ACTIVE` | Event aktif mi? |
| 28 | `REG_SAME` | Sonraki event aynı register'a mı? |

### Genişletilmiş Selector Sütunları (29-48)
| İndeks | Sütun | Açıklama |
|--------|-------|----------|
| 29 | `IS_DIV` | Div selector'ü |
| 30 | `IS_INV` | Inv selector'ü |
| 31 | `IS_AND` | And selector'ü |
| 32 | `IS_OR` | Or selector'ü |
| 33 | `IS_XOR` | Xor selector'ü |
| 34 | `IS_NOT` | Not selector'ü |
| 35 | `IS_NEQ` | Neq selector'ü |
| 36 | `IS_GT` | Gt selector'ü |
| 37 | `IS_LTE` | Lte selector'ü |
| 38 | `IS_GTE` | Gte selector'ü |
| 39 | `IS_STORE` | Store selector'ü |
| 40 | `IS_PUSH` | Push selector'ü |
| 41 | `IS_POP` | Pop selector'ü |
| 42 | `IS_CALL` | Call selector'ü |
| 43 | `IS_RET` | Ret selector'ü |
| 44 | `IS_SREAD` | SRead selector'ü |
| 45 | `IS_SWRITE` | SWrite selector'ü |
| 46 | `IS_POSEIDON` | Poseidon selector'ü |
| 47 | `IS_SYSCALL` | Syscall selector'ü |
| 48 | `IS_VERIFY_MERKLE` | VerifyMerkle selector'ü |

### Memory Table Sütunları (49-54)
| İndeks | Sütun | Açıklama |
|--------|-------|----------|
| 49 | `MEM_CLK` | Memory event clock'u |
| 50 | `MEM_ADDR` | Bellek adresi |
| 51 | `MEM_VAL` | Bellek değeri |
| 52 | `MEM_IS_WRITE` | Yazma event'i mi? |
| 53 | `MEM_ACTIVE` | Event aktif mi? |
| 54 | `MEM_SAME` | Sonraki event aynı adrese mi? |

Memory tablosu: `Load`, `Store`, `Push`, `Pop`, `Call`, `Ret` işlemlerine ek olarak **`SRead` ve `SWrite` işlemlerini de kapsar.** Storage işlemleri `STORAGE_BASE = 2 << 60` adres ön eki ile memory adres alanına yerleştirilir. Bu sayede storage tutarlılığı, ayrı bir LogUp tablosuna gerek kalmadan mevcut memory LogUp altyapısı üzerinden doğrulanır.

### Soundness ve Public Input Sütunları (55-64)
| İndeks | Sütun | Açıklama |
|--------|-------|----------|
| 55 | `STACK_PTR` | Stack işaretçisi |
| 56 | `REG_SUB_CLK` | Register alt-clock (LogUp sıralama) |
| 57 | `GAS_USED` | Kümülatif gas tüketimi |
| 58 | `DIV_INV` | Div inverse witness |
| 59 | `DIV_ZERO` | Div sıfır bayrağı |
| 60 | `INV_ZERO` | Inv/Not sıfır bayrağı (paylaşımlı) |
| 61 | `EQ_DIFF_INV` | Eq/Neq fark inverse witness |
| 62 | `JNZ_COND_INV` | Jnz koşul inverse witness |
| 63 | `RAW_INST` | Ham instruction (encode edilmiş u64) |
| 64 | `CPU_ACTIVE` | CPU aktiflik bayrağı (padding izolasyonu) |

### Comparison Witness Sütunları (65-257)
| Aralık | Grup | Açıklama |
|--------|------|----------|
| 65-128 | `CMP_RS1_BASE` | rs1'in 64-bit decomposition'ı (Lt/Gt/Lte/Gte ve And/Or/Xor için ortak) |
| 129-192 | `CMP_RS2_BASE` | rs2'nin 64-bit decomposition'ı |
| 193-256 | `CMP_EQ_BASE` | Equality prefix flags (eq_0..eq_63, yalnızca comparison için) |
| 257 | `CMP_LT_RAW` | Ham less-than sonucu (bit decomposition'dan hesaplanan) |

**Comparison constraint'leri:** 64-bit decomposition ile her bitin boolean olduğu kontrol edilir. Equality prefix flags (eq_i), MSB'den LSB'ye doğru özyineli olarak hesaplanır: `eq_i = eq_{i+1} * (a_i == b_i)`. Sonuç: `Lt: rd = cmp_lt_raw`, `Gt: rd = 1 - eq_0 - cmp_lt_raw`, `Lte: rd = eq_0 + cmp_lt_raw`, `Gte: rd = 1 - cmp_lt_raw`.

**Bitwise constraint'leri:** And/Or/Xor aynı bit decomposition sütunlarını kullanır. `And: rd = Σ(a_i*b_i*2^i)`, `Or: rd = rs1 + rs2 - and_result`, `Xor: rd = rs1 + rs2 - 2*and_result`. `Not` ise inverse witness ile: `rd = 1 - rs1*inv` (`COL_INV_ZERO` paylaşımlı).

### Poseidon Witness Sütunları (258-353)
| Aralık | Grup | Açıklama |
|--------|------|----------|
| 258-289 | `POSEIDON_STATE_BASE` | 4 round × 8 state elementi (round giriş durumu) |
| 290-321 | `POSEIDON_X2_BASE` | S-box ara değerler: x² |
| 322-353 | `POSEIDON_X4_BASE` | S-box ara değerler: x⁴ |

**Poseidon4 parametreleri:** alpha=7, width=8, 4 tam round. MDS circulant matris `[7,1,3,8,8,3,4,9]`. Round sabitleri Plonky3 Poseidon1 Goldilocks'tan alınmıştır. AIR yalnızca round 0 S-box constraint'ini doğrular; VM 4 round'un tamamını hesaplar.

## Yardımcı İz (Auxiliary Trace) Şeması

BudZKVM, Cross-Table Lookup (CTL) işlemlerini doğrulamak için LogUp Fractional Sums yöntemini kullanır. Yardımcı iz **3 sütun** genişliğindedir:

| Sütun | Adı | Tanım |
| --- | --- | --- |
| 0 | `S_REG` | Register tutarlılığı LogUp akümülatörü |
| 1 | `S_MEM` | Memory + Storage tutarlılığı LogUp akümülatörü |
| 2 | `S_PROG` | Program CTL LogUp akümülatörü |

> **Not:** Storage (`SRead`, `SWrite`) işlemleri ayrı bir LogUp tablosu gerektirmez. `STORAGE_BASE = 2 << 60` adres ön eki ile memory tablosuna dahil edilir. Bu sayede storage tutarlılığı mevcut memory LogUp (sütun 1) üzerinden doğrulanır.

Bu sütunlar Fiat-Shamir transcript'inden gelen α, β (tuple paketleme) ve γ (kesirli payda) değerlerine bağlıdır. Her satırda S_{i+1} = S_i + Σ w_j/(γ - C_j) kuralı işletilir. Program sonunda her sütunun 0 olması zorunludur.

## Aritmetik Semantiği

BudVM aritmetiği Goldilocks asal cismi (P = 2^64 - 2^32 + 1) üzerinde çalışır:

* `Add`, `Sub`, `Mul`: wrapping u64 aritmetiği
* `Div`: Goldilocks field-native modüler bölme: `rd = rs1 * rs2^{-1} mod P`. Payda sıfırsa sonuç 0.
* `Inv`: Modüler ters: `rd = rs1^{-1} mod P`. Girdi sıfırsa sonuç 0.
* `Poseidon4`: 4-round Poseidon hash (alpha=7, width=8 Goldilocks). Girdi: `(rs1, rs2)`, state: `[a, b, 0, 0, 0, 0, 0, 0]`, çıktı: `state[0]`.

## Gas Semantiği

| Opcode grubu | Gas |
| --- | ---: |
| `Halt` | 0 |
| Basit ALU, branch, karşılaştırma | 1 |
| `Call`, `Ret`, `Push`, `Pop` | 2 |
| `Load`, `Store`, `SRead`, `SWrite` | 3 |
| `Syscall` | 5 |
| `Poseidon`, `VerifyMerkle` | 10 |

## Fixture Testleri

`bud-vm/tests/trace_fixtures.rs` trace şemasını örnek programlar üzerinden sabitler:

* Aritmetik trace: `Load`, `Add`, `Sub`, `Mul`, `Halt`
* Kontrol akışı trace'i: `Jnz`, `Jmp` ve program dışına çıkınca deterministik halt
* Memory/storage/event trace'i: `Store`, memory `Load`, `SWrite`, `SRead`, `Log`

Trace schema değişirse hem VM testleri hem prover testleri birlikte güncellenmelidir.
