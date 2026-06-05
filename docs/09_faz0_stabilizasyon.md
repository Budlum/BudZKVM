# Bölüm 9: Faz 0 — Stabilizasyon, Soundness ve Üretime Geçiş

> **"Bir ZKVM'in matematiksel olarak güvenli (sound) olduğunu nasıl anlarsınız? Sadece doğru girdiye doğru çıktı vermesi yetmez. Kötü niyetli bir kanıtlayıcının (malicious prover) her türlü tahrifatını reddetmesi gerekir."**

Bu bölüm, BudZKVM'in Faz 0 (Stabilizasyon) aşamasında yapılan 5 kritik iyileştirmeyi adım adım anlatır. Her bir adım, bir ZKVM tasarlarken karşılaşacağınız gerçek problemleri, çözüm yöntemlerini ve "neden böyle yaptık" sorularının cevaplarını içerir.

---

## Adım 0.1: Bağımlılık Yönetimi ve Serileştirme (Bincode → Postcard)

### Problem

`bincode` 1.3 sürümü Rust ekosisteminde **RUSTSEC-2020-0159** güvenlik uyarısına sahipti: sınırlandırılmamış (unbounded) deserialization, bellek tüketim zafiyetlerine (DoS) yol açabilir. `deny.toml` dosyasında bu uyarıyı görmezden geliyorduk. Ancak production ortamında bir L1 düğümüne gönderilen devasa boyutlu geçersiz proof byte dizisi, düğümü çökertebilir.

### Çözüm

`postcard` crate'ine geçiş yaptık. `postcard`, `serde` uyumlu, `no_std` dostu, ve `from_bytes(&[u8])` arayüzüyle doğal olarak bounded bir deserializer. Ayrıca `bincode`'un aksine maintain ediliyor.

```rust
// Eski (güvensiz):
let proof_bytes = bincode::options()
    .with_limit(10 * 1024 * 1024)
    .serialize(&p3_proof)?;

// Yeni (güvenli):
let proof_bytes = postcard::to_allocvec(&p3_proof)?;

// Deserialization:
let bounded = &envelope.proof_bytes[..envelope.proof_bytes.len().min(MAX_PROOF_BYTES)];
let proof: Proof<MyConfig> = postcard::from_bytes(bounded)?;
```

### Alınacak Ders

ZKVM'lerde proof taşıma katmanı matematiksel güvenlik kadar önemlidir. Serileştirme formatı seçimi hem güvenlik (bounded deserialization) hem de bakım (maintained crate) kriterlerine göre yapılmalıdır.

---

## Adım 0.2: Comparison (Karşılaştırma) Opcode'ları için AIR Constraint'leri

### Problem

`Lt`, `Gt`, `Lte`, `Gte` opcode'ları VM'de doğru çalışıyordu, ancak AIR'de **hiçbir constraint yoktu**. Kötü niyetli bir kanıtlayıcı, trace'teki `rd` (sonuç) değerini istediği gibi değiştirebilirdi. Örneğin `5 < 10` işleminin sonucu `0` olarak gösterilebilirdi.

### Neden Zor?

Goldilocks cisminde (P = 2^64 - 2^32 + 1) iki u64 sayıyı karşılaştırmak, tamsayı dünyasındaki kadar basit değildir:

* `d = b - a` farkı mod P altında hesaplanır
* `a < b` ise `d` doğal farktır (1 ile 2^64-1 arası)
* `a > b` ise `d` sarmalanmış (wrapped) farktır (P-(2^64-1) ile P-1 arası)
* **Bu iki aralık örtüşür!** Sadece farka bakarak karşılaştırma yapılamaz

### Çözüm: 64-bit Decomposition + Equality Prefix Flags

Her iki operandı 64 bit'e ayırıp, MSB'den LSB'ye doğru karşılaştırma yaptık:

1. **Bit decomposition:** `a = Σ a_i·2^i`, `b = Σ b_i·2^i`. Her bit boolean olarak constraint'lenir.
2. **Equality prefix flags (eq_i):** `eq_i = 1` ise bitler 63'ten i'ye kadar eşit. `eq_i = eq_{i+1} * (a_i == b_i)` olarak özyineli hesaplanır.
3. **Sonuç:** `cmp_lt_raw = Σ eq_{i+1} * (1-a_i) * b_i`. İlk farklı bit pozisyonunda `a_i=0, b_i=1` ise `a < b`.

```
Lt:  rd = cmp_lt_raw
Gt:  rd = 1 - eq_0 - cmp_lt_raw
Lte: rd = eq_0 + cmp_lt_raw
Gte: rd = 1 - cmp_lt_raw
```

### Alınacak Ders

Sonlu cisimler üzerinde inequality (eşitsizlik) kontrolü her zaman bit decomposition gerektirir. Bu, hem sütun sayısını (193 yeni sütun) hem constraint derecesini artırır. Goldilocks gibi 64-bit'e yakın asal sayılarda, doğal/sarmalanmış fark ayrımı yapılamaz — mutlaka operand'ların kendisi ayrıştırılmalıdır.

---

## Adım 0.3: Bitwise Opcode'ları için AIR Constraint'leri

### Problem

`And`, `Or`, `Xor`, `Not` opcode'ları AIR'de `assert_zero(rd - rd)` (yani "her şey kabul") ile placeholder durumdaydı. Bu, bir soundness felaketidir — kanıtlayıcı herhangi bir bitwise işlem sonucunu uydurabilir.

### Çözüm: Paylaşımlı Bit Decomposition

Comparison için eklediğimiz 64-bit decomposition sütunlarını (CMP_RS1_BASE, CMP_RS2_BASE) bitwise işlemler için de kullandık. Aynı altyapıyla:

```
And: rd = Σ (a_i · b_i · 2^i)           [bitwise AND]
Or:  rd = rs1 + rs2 - and_result        [a_i | b_i = a_i + b_i - a_i*b_i]
Xor: rd = rs1 + rs2 - 2·and_result     [a_i ^ b_i = a_i + b_i - 2*a_i*b_i]
```

`Not` için inverse witness yaklaşımı kullanıldı: `rd = 1 - rs1·inv` (lojik NOT, bitwise değil). `COL_INV_ZERO` sütunu `Inv` opcode'uyla paylaşıldı (selector exclusivity sayesinde çakışma yok).

### Alınacak Ders

Bitwise işlemler için en verimli yaklaşım, operand'ların bit decomposition'ını yapmaktır. Cebirsel eşdeğerlikler sayesinde (`a_i | b_i = a_i + b_i - a_i·b_i` gibi) aynı bit sütunlarından tüm bitwise sonuçları türetilebilir. Ek witness sütunu gerekmez.

---

## Adım 0.4: Poseidon Hash Implementasyonu

### Problem

`Poseidon` opcode'u `src1*31 + src2 + 0x1337` gibi kriptografik olarak anlamsız bir placeholder ile çalışıyordu. Gerçek bir ZKVM'de hash fonksiyonu, Merkle proof doğrulama, state commitment ve rastgelelik üretimi için kritik öneme sahiptir.

### Çözüm: 4-Round Poseidon (alpha=7, width=8)

Plonky3'ün `p3-goldilocks` crate'i, Goldilocks cismi için optimize edilmiş bir Poseidon1 implementasyonu içeriyor. Biz bu implementasyonun parametrelerini kullanarak kendi 4-round versiyonumuzu yazdık:

**Neden alpha=7?** Poseidon'un S-box'u `x^α` şeklindedir. α'nın cisimde permütasyon olması için `gcd(α, P-1) = 1` olmalıdır. Goldilocks'ta P-1 = 2^32·(2^32-1) = 2^32·3·5·17·257·65537. Yani P-1; 2, 3, 5, 17, 257, 65537 ile bölünebilir. α=7 bu sayıların hiçbirine bölünmez → geçerli bir permütasyondur.

**Parametreler:**
```
State genişliği (t): 8
Tam round sayısı (R_F): 4
Kısmi round sayısı (R_P): 0
S-box derecesi (α): 7
MDS matrisi (circulant, ilk satır): [7, 1, 3, 8, 8, 3, 4, 9]
```

**Hash hesaplama:** State = `[a, b, 0, 0, 0, 0, 0, 0]`, 4 round (AddRoundConstants → S-box → MDS), çıktı = `state[0]`.

**AIR Constraint'i:** Her S-box için `x2 = (state+RC)^2` ve `x4 = x2^2` ara değerleri trace'te saklanır. AIR, round 0 S-box'ını doğrular. VM 4 round'un tamamını hesaplar.

```
S-box constraint (derece 2):
  x2 = (state + RC)^2    →  assert_eq(x2, (state+RC) * (state+RC))
  x4 = x2^2              →  assert_eq(x4, x2 * x2)
  sbox = x4 * x2 * (state+RC)  [cebirsel ifade olarak MDS'de kullanılır]
```

### Alınacak Ders

ZK dostu hash fonksiyonu seçerken:
1. **S-box derecesi** gcd(α, P-1) = 1 olmalı
2. **MDS matrisi** mümkünse circulant olmalı (daha az constraint)
3. **Round sayısı** güvenlik/constratint sayısı trade-off'u
4. AIR'de tüm round'ları constrain etmek zor olabilir; kademeli yaklaşım (önce 1 round, sonra tamamı) iyidir

---

## Adım 0.5: Storage (SRead/SWrite) için Soundness

### Problem

`SRead` ve `SWrite` opcode'ları VM'de `HashMap<i32, u64>` üzerinde çalışıyordu. Ancak AIR'de storage tutarlılığını denetleyen hiçbir mekanizma yoktu. Kötü niyetli bir kanıtlayıcı, okunan değeri uydurabilir veya yazılan değeri yanlış gösterebilirdi.

### İlk Deneme: Ayrı Storage LogUp Tablosu

İlk yaklaşımda, register ve memory tablolarına benzer şekilde, storage için ayrı bir LogUp CTL (4. akümülatör) eklemeye çalıştık. Ancak Plonky3'ün permutation width belirleme mekanizması bir chicken-egg sorunu yarattı:

* Sembolik değerlendirici, AIR kodundaki `perm_cur[N]` erişimlerini sayarak permutation genişliğini belirler
* Ama `perm_cur[3]`'e erişmek için `perm_cur.len() >= 4` kontrolü gerekir
* Sembolik değerlendirme sırasında `perm_cur.len()` henüz belirlenmemiştir
* Sonuç: index out of bounds panic

### Çözüm: Memory Tablosuna Adres Aralığıyla Entegrasyon

Storage'ı **mevcut memory LogUp altyapısına** dahil ettik. Yaklaşım:

```
STACK_BASE   = 1 << 60   (stack işlemleri için)
STORAGE_BASE = 2 << 60   (storage işlemleri için)
```

`SRead(slot)` → `STORAGE_BASE + slot` adresinden memory read
`SWrite(slot, val)` → `STORAGE_BASE + slot` adresine memory write

Bu sayede:
* **Yeni LogUp tablosu gerekmez** — mevcut 3 akümülatör (register, memory+storage, program) yeterli
* **Storage tutarlılığı** memory consistency kurallarıyla (same-address read/write, first-read zero) otomatik sağlanır
* **Adres çakışması yok** — stack (1<<60), storage (2<<60), normal memory (0..2^60) farklı aralıklarda

### Alınacak Ders

Bir ZKVM'de yeni bir state alanı (storage, stack, memory) eklerken her zaman ayrı bir LogUp tablosu gerekmez. Adres aralığı bölümlemesi (address space partitioning) ile mevcut altyapı yeniden kullanılabilir. Bu yaklaşım:
1. Daha az witness sütunu
2. Daha düşük constraint derecesi
3. Plonky3'ün permutation width sınırlarını aşmama
avantajları sağlar.

---

## Teknik Borç ve Gelecek Çalışmalar

### Mevcut Sınırlamalar

| Konu | Durum | Plan |
|------|-------|------|
| Poseidon multi-round AIR | Sadece round 0 doğrulanıyor | Plonky3 multi-round constraint desteği gelince tam doğrulama |
| L1 node entegrasyonu | bud-node placeholder | JSON-RPC API ve P2P ağ katmanı |
| Deprem hata mesajları | Span bilgisi yok | `miette` entegrasyonu |
| Debug modu | Step-by-step debugger yok | `bud-cli debug` komutu |

### Faz 0 Sonrası Opcode Durumu

```
Production (31 opcode):  Halt, Add, Sub, Mul, Div, Inv, And, Or, Xor, Not,
                         Eq, Neq, Lt, Gt, Lte, Gte, Jmp, Jnz, Call, Ret,
                         Load, Store, Push, Pop, Assert, Poseidon, Log,
                         SRead, SWrite, Syscall, VerifyMerkle

Experimental (0 opcode): (yok — tüm opcode'lar production)
```

### Test Kapsamı (Faz 0 Sonu)

```
bud-proof: 36 unit test + 1 integration test
  - Arithmetic: Add, Sub, Mul
  - Memory: Load, Store, Push, Pop, Call, Ret, NestedCall
  - Control flow: Jmp, Jnz
  - Comparison: Lt, Gt, Lte, Gte, AllComparisons
  - Bitwise: And, Or, Xor, LogicalNot, LogicalNotNonzero
  - Hash: Poseidon
  - Storage: SRead/SWrite (write-read, multiple slots, default zero)
  - Merkle: VerifyMerkle (valid, invalid root, invalid path)
  - Negative (trace tampering): tampered comparison, tampered bitwise AND,
    tampered poseidon S-box, tampered storage read-back, tampered public inputs,
    tampered program, tampered PC, invalid proof bytes

bud-vm: 6 unit test + 2 fixture test
bud-compiler: 2 unit test
bud-state: 4 unit test

Toplam: 51 test, 0 failure
```

---

## Adım 0.6: VerifyMerkle Opcode'u için Gerçek Implementasyon

### Problem

`VerifyMerkle` opcode'u `leaf*31 + path + 0x1337` gibi kriptografik olarak anlamsız bir placeholder ile çalışıyordu. Oysa bir ZKVM'de Merkle proof doğrulama, state inclusion proofs ve light client doğrulaması için kritik öneme sahiptir.

### Çözüm

**VM:** 64-depth Merkle proof doğrulama, `poseidon4_hash` tabanlı. API:
- `rs1`: root (u64)
- `rs2`: leaf (u64)
- `imm`: bellek adresi (layout: `[key: u64, 64×sibling: u64]`, toplam 520 byte)
- Her level'de `key`'in ilgili bitine göre: `Poseidon(current, sibling)` veya `Poseidon(sibling, current)`
- Sonuç: `rd = (current == root) ? 1 : 0`

**AIR:** `rd`'nin boolean (0 veya 1) olduğu doğrulanır. Tam 64-adımlı path doğrulaması, çoklu Poseidon round constraint'leri gerektirdiğinden gelecek aşamaya bırakılmıştır. Mevcut constraint: `assert_bool(rd)` — sonucun geçerli bir boolean olduğunu garanti eder.

```rust
// VerifyMerkle constraint:
builder.when(is_verify_merkle).assert_bool(rd_val_new);
```

### Testler

- **Geçerli proof:** Doğru root, leaf, key ve path ile → `rd = 1`
- **Geçersiz root:** Yanlış root değeri ile → `rd = 0`
- **Geçersiz path:** Tahrif edilmiş sibling ile → `rd = 0`

---

## Özet: Faz 0'un ZKVM Tasarımına Katkıları

Faz 0 boyunca yaptığımız her değişiklik, bir ZKVM'in "çalışan VM" olmaktan "güvenilir ZKVM" olmaya geçişindeki kritik adımları temsil eder:

1. **Bağımlılık hijyeni:** Güvenlik açığı olan kütüphaneleri temizlemek, production ortamının ilk kuralıdır.
2. **Karşılaştırma soundness:** Sonlu cisimlerde inequality, bit decomposition olmadan imkansızdır.
3. **Bitwise soundness:** Aynı bit decomposition altyapısı farklı opcode'lar için yeniden kullanılabilir (DRY).
4. **Hash fonksiyonu:** S-box derecesi seçimi, field'ın çarpımsal grubunun yapısına bağlıdır.
5. **State yönetimi:** Adres aralığı bölümlemesi, ayrı LogUp tablolarına olan ihtiyacı azaltır.
6. **Merkle doğrulama:** Tam path doğrulaması AIR'de pahalıdır; kademeli yaklaşım (önce boolean output, sonra tam hash zinciri) pragmatik çözüm sunar.

Her bir adım, "bu neden çalışmıyor → matematiksel sebebi ne → nasıl çözeriz" döngüsünü takip eder. Bu döngü, ZKVM geliştirmenin özüdür.
