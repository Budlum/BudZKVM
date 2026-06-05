# Bölüm 5: STARK, AIR ve Plonky3 (bud-proof)

Sıra geldi sihrin gerçeğe dönüştüğü yere. Elimizde VM'den aldığımız geniş ve detaylı bir "Execution Trace" (Çalıştırma İzi) var. Amacımız, bu matrisi alıp ZK-STARK kullanarak kriptografik olarak kanıtlamak. Bunun için Polygon'un geliştirdiği, sektör standardı haline gelmeye başlayan **Plonky3** kütüphanesini kullanıyoruz. Projemizdeki `bud-proof` modülü tamamen bu işe adanmıştır.

## Plonky3 Neden Önemli?

Eskiden (örneğin Winterfell kullanırken) kısıt dereceleri (constraint degrees), domain boyutları ve blowup faktörlerini manuel olarak çok hassas şekilde ayarlamak gerekiyordu. Plonky3, STARK kanıtlarının matematiğini daha modüler ve esnek bir mimariye oturtur. Özellikle **Goldilocks** cismi (field) gibi donanım dostu küçük asal sayıları yerel olarak çok iyi destekler. Bu sayede kanıt üretme süresi ciddi şekilde kısalır.

## AIR (Algebraic Intermediate Representation)

Bir ZKVM'in kalbi AIR'dır. AIR, Execution Trace'in doğruluğunu kontrol eden **matematiksel kurallar bütünüdür**. 
* Geleneksel programlamada doğruluğu `if (A + B == C)` ile kontrol ederiz.
* AIR dünyasında ise bu denklemi sıfıra eşitlemek zorundayız: `(A + B) - C = 0`

Eğer tüm satırlar için tüm denklemlerinizin sonucu sıfır çıkıyorsa, STARK kanıtı başarılı olur. Tek bir satırda, tek bir kısıtlama sıfırdan farklı bir sonuç verirse (örneğin VM yanlış bir matematik hesabı yapmışsa), sistem "Constraint failed" hatası verir ve kanıt üretilemez.

### Geçiş Kısıtlamaları (Transition Constraints)

`plonky3_air.rs` dosyasını incelerseniz, `BudAir` implementasyonunda `eval` fonksiyonunu görürsünüz. Bu fonksiyon, trace üzerinde "şu anki satır (`cur`)" ve "bir sonraki satır (`nxt`)" arasında kontroller yapar.

Örneğin PC (Program Counter) kuralını yazalım:
*"Eğer program bitmediyse, bir sonraki satırın PC'si, şu anki satırın next_pc'sine eşit olmalıdır."*

```rust
builder.when_transition().assert_zero(
    is_cpu.clone() * (nxt_pc.clone() - next_pc.clone())
);
```
Bu denklemde, eğer CPU aktifse ve `nxt_pc` ile `next_pc` farklıysa, sonuç sıfır olmaz ve kanıt patlar.

### Selector Sütunlarının Gücü

Daha önce Opcode'ların (0x01 = Add vb.) trace'e eklendiğini söylemiştik. Ancak polinom matematiğinde `if (opcode == 0x01)` yazamazsınız. Bunun yerine BudZKVM trace'ine **Selector Sütunları** eklenmiştir: `COL_IS_ADD`, `COL_IS_SUB`, `COL_IS_JMP` vb.

Eğer işlem bir Toplama (ADD) ise, trace oluşturulurken `COL_IS_ADD` sütununa `1` yazılır, diğerlerine `0` yazılır. AIR içindeki kuralımız şöyle şekillenir:

```rust
builder.when(cur[COL_IS_ADD].clone())
    .assert_eq(rd_val_new.clone(), rs1_val.clone() + rs2_val.clone());
```
Bu sayede her bir matematiksel denklem, sadece kendi opcode'u aktif olduğunda çalışır. BudZKVM'de **32 selector sütunu** vardır — her opcode için bir tane.

### Trace Matrisi Yapısı

Güncel BudZKVM ana trace matrisi **354 sütun** genişliğindedir. Sütun grupları:

| Aralık | Grup | Açıklama |
|--------|------|----------|
| 0-10 | Temel | PC, Next PC, Opcode, Register Index/Değerleri, Immediate |
| 11-22 | CPU Selectors | 12 adet selector (ADD, SUB, JMP, JNZ, HALT vb.) |
| 23-28 | Register Table | Register event sıralaması (LogUp için) |
| 29-48 | Genişletilmiş Selectors | 20 adet ek selector (DIV, AND, STORE, CALL, POSEIDON vb.) |
| 49-54 | Memory Table | Memory event sıralaması (Load, Store, Push, Pop, SRead, SWrite) |
| 55-64 | Soundness | Gas, inverse witness'lar, CPU aktiflik bayrağı |
| 65-257 | Comparison/Bitwise | 64-bit decomposition + equality prefix flags |
| 258-353 | Poseidon | 4-round state + S-box intermediate değerleri |

## Register Tablosu Kısıtlamaları

Önceki bölümde bahsettiğimiz "Register Consistency" (Tutarlılık) kontrolünü Plonky3'te nasıl yazdığımıza bakalım:

*"Eğer bir sonraki satırda aynı register'da kalıyorsak (`r_same = 1`) VE bu bir okuma işlemiyse (`nr_write = 0`), register'ın içindeki değer DEĞİŞMEMELİDİR."*

Bunu polinom diliyle şu şekilde ifade ederiz:
```rust
builder.when_transition().assert_zero(
    r_active.clone() * nr_active.clone() * r_same.clone() * 
    (one.clone() - nr_write) * (nr_val - r_val)
);
```
İşte bir ZKVM'in hafıza bütünlüğünü koruyan, hacklenmesini ve dışarıdan veri sızdırılmasını engelleyen güvenlik duvarı tam olarak bu matematiksel formüllerdir.

## LogUp CTL: 3 Tablolu Cross-Table Lookup

BudZKVM, register, memory ve program tabloları arasındaki tutarlılığı **LogUp Fractional Sums** yöntemiyle doğrular. 3 adet Fiat-Shamir challenge'ı (α, β, γ) kullanılır:

1. **Register LogUp** (accumulator 0): CPU'nun `rs1`, `rs2` okumaları ve `rd` yazması, register event tablosuyla eşleştirilir. `R0` donanımsal olarak sıfıra sabitlenmiştir.

2. **Memory LogUp** (accumulator 1): CPU'nun `Load`, `Store`, `Push`, `Pop`, `Call`, `Ret` işlemlerine ek olarak **`SRead`, `SWrite` işlemlerini de kapsar.** Storage işlemleri `STORAGE_BASE = 2 << 60` adres ön eki ile memory adres alanına yerleştirilir. Stack işlemleri `STACK_BASE = 1 << 60` adresini kullanır. Bu sayede ayrı bir storage LogUp tablosuna gerek kalmaz.

3. **Program CTL** (accumulator 2): CPU'nun `(pc, instruction)` çiftleri, preprocessed program tablosuyla eşleştirilir. Yalnızca `CPU_ACTIVE = 1` olan satırlar LogUp'a katılır.

## Comparison ve Bitwise Constraint Stratejisi

### Comparison (Lt, Gt, Lte, Gte)

Goldilocks cisminde (P = 2^64 - 2^32 + 1) iki u64 sayıyı karşılaştırmak için **64-bit decomposition + equality prefix flags** kullanılır. Her iki operand 64 bit'e ayrılır, MSB'den LSB'ye doğru karşılaştırma yapılır:

```
Lt:  rd = cmp_lt_raw
Gt:  rd = 1 - eq_0 - cmp_lt_raw
Lte: rd = eq_0 + cmp_lt_raw
Gte: rd = 1 - cmp_lt_raw
```

Bu yaklaşım 193 ek sütun gerektirir (64+64 bit + 64 eq flags + 1 result).

### Bitwise (And, Or, Xor, Not)

Comparison için eklenen bit decomposition sütunları, bitwise işlemler için **yeniden kullanılır.** Cebirsel eşdeğerliklerle:

```
And: rd = Σ(a_i · b_i · 2^i)
Or:  rd = rs1 + rs2 - and_result
Xor: rd = rs1 + rs2 - 2·and_result
Not: rd = 1 - rs1·inv  (inverse witness, lojik NOT)
```

## Poseidon Hash (alpha=7, 4 round)

VM, 4-round Poseidon hash (alpha=7, width=8, Goldilocks field) hesaplar. AIR, round 0 S-box'ını doğrular:

```rust
// x2 = (state + RC)^2
builder.assert_eq(x2, (state + RC) * (state + RC));
// x4 = x2^2
builder.assert_eq(x4, x2 * x2);
```

S-box intermediate değerleri trace'te 96 sütun kaplar (4 round × 8 element × 3 değer). Tam multi-round doğrulama Plonky3 constraint limitleri nedeniyle gelecek aşamaya bırakılmıştır.

## BudZKVM'de Güncel Prover Akışı

1. VM programı çalıştırır ve her cycle için bir trace satırı üretir.
2. Adapter bu satırları **354 sütunlu** `Goldilocks` main trace matrisine çevirir (bit decomposition, S-box intermediate'leri, storage event'leri dahil).
3. Main trace commit edilir ve transcript'e yazılır.
4. Fiat-Shamir randomness üretilir.
5. Bu randomness ile **3 sütunlu** auxiliary trace üretilir (Register, Memory+Storage, Program CTL LogUp).
6. AIR kısıtları main ve auxiliary pencereleri birlikte okuyarak değerlendirilir.
7. Proof `postcard` ile serialize edilerek (bounded, DoS korumalı) CLI, test veya L1 entegrasyon katmanına taşınır.

Güncel Plonky3 yolunda auxiliary trace, **LogUp Fractional Sums (Kesirli Toplamlar)** yöntemini kullanır. Fiat-Shamir transcript'inden üç adet random challenge (α, β, γ) üretilir. γ değeri paydadaki kesirli toplamları oluşturmak için kullanılır.

Bir sonraki bölümde, derleyici ve CLI katmanını inceleyeceğiz.
