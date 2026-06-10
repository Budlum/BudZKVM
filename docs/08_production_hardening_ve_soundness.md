# Bölüm 8: Üretime Hazırlık, Soundness ve Güvenlik Sertleştirmesi (Production Hardening & Soundness)

Sanal bir makinenin (VM) yerel test ortamında doğru bytecode çalıştırıp yeşil testler vermesi, onun kriptografik olarak güvenli (sound) bir ZKVM olduğu anlamına gelmez. Klasik yazılım dünyasında "doğru girdi -> doğru çıktı" testi yeterliyken, ZKVM dünyasında çok daha büyük bir tehdit modeliyle karşı karşıyayız: **Kötü Niyetli Kanıtlayıcı (Malicious Prover)**.

Kötü niyetli bir kanıtlayıcı, sanal makineyi yerel olarak hiç çalıştırmadan, çalıştırma izi (execution trace) matrisindeki sayıları tahrif ederek veya AIR kısıtlamalarındaki (algebraic constraints) boşlukları suistimal ederek geçersiz bir eyaleti (state) veya sahte bir işlemi verifier'a (doğrulayıcı) kanıtlayabilir.

> [!IMPORTANT]
> **Soundness (Doğruluk Güvencesi):** Dürüst olmayan hiçbir kanıtlayıcının, yanlış bir iddia (örneğin sıfırdan token yaratma, başkasının bakiyesini çalma veya geçersiz bytecode yürütme) için verifier'dan onay alacak bir kanıt üretememesi garantisidir.

Bu bölümde, BudZKVM projesini üretime (production-ready) ve güvenlik denetimine (audit-ready) hazırlarken uyguladığımız sertleştirme ve soundness adımlarını, matematiksel formüllerle ve kod yapılarıyla detaylıca inceleyeceğiz.

---

## 1. Sanal Makine ve ISA Güvenliği (Determinizm & Semantik)

Bir ZKVM'in matematiksel kısıtları, sanal makinenin deterministik semantikleri üzerine kurulur. Eğer VM'de sessizce yutulan hatalar veya tanımsız davranışlar (undefined behaviors) varsa, AIR kuralları bu durumları yakalayamaz.

### Vm::run Sessiz Hatalarının Önlenmesi
İlk tasarımlarda `Vm::run` fonksiyonu yürütme sırasında oluşan hataları (örneğin Out of Gas, geçersiz opcode veya yığın taşması) sessizce yutuyor veya panikliyordu. Üretim ortamında bir akıllı sözleşme (contract) çalıştırılırken hata oluşursa, eyaletin deterministik bir şekilde geriye çekilmesi (rollback) ve bu hatanın kriptografik kanıta (proof receipt) yansıması gerekir.

Bunu sağlamak için VM çekirdeğini şu şekilde güncelledik:
* `Vm::run` artık geriye `Result<ExecutionReceipt, VmError>` döner.
* `ExecutionReceipt` yapısı, yürütmenin başarısını (`success`), harcanan gas miktarını (`gas_used`), final PC değerini (`final_pc`), oluşan olayları (`events`) ve eyalet yazma özetini (`state_writes_digest`) net bir şekilde taşır.
* Hata oluştuğunda (örneğin `VmError::OutOfGas` veya `VmError::AssertionFailed`), eyalet yazmaları geri alınır, ancak harcanan gas ve başarısızlık durumu fişe (receipt) yazılarak determinizm korunur.

### IsaProfile (Production vs Experimental Opcode Politikası)
Sanal makinelere geliştirme aşamasında yeni özellikler (örneğin deneysel kriptografik öncüller veya storage opcodeları) eklenir. Ancak bu deneysel komutlar henüz olgunlaşmamış veya AIR kısıtları tam yazılmamış olabilir. Kötü niyetli bir kanıtlayıcı, AIR kısıtı olmayan deneysel bir komutu kullanarak verifier'ı aldatabilir.

BudZKVM'de bu zafiyeti engellemek için **`IsaProfile`** yapısını kurduk:
```rust
pub enum IsaProfile {
    Production,
    Experimental,
    Testing,
}
```
* `Instruction::decode` metodu, aktif profile bağlı olarak komut çözer. Eğer profile `Production` ise ve bytecode içinde deneysel bir komut (örneğin `SWrite` veya `SRead` deneysel moddayken) bulunursa, VM çözme (decoding) aşamasında hata verir.
* Derleyici (`bud-compiler`) kod üretirken hedef profili denetler. Üretim profili aktifken kaynak kodda deneysel bir ifade veya komut varsa derleme statik olarak engellenir.

### Goldilocks Field-Native Modüler Bölme
Klasik bilgisayarlarda bölme işlemi tamsayı bölmesidir (`src1 / src2 = bölüm` ve kalanı atar). Ancak sonlu cisimler (finite fields) üzerinde çalışan ZK-STARK sistemlerinde tamsayı bölmesini AIR kısıtlaması olarak yazmak son derece maliyetlidir (bit decomposition ve range check gerekir).

BudZKVM, yerel olarak Goldilocks cismi ($p = 2^{64} - 2^{32} + 1$) üzerinde çalıştığı için bölme komutunu (`Div`) field-native modüler bölme haline getirdik:
$$\text{dst} = \text{src1} \cdot \text{src2}^{-1} \pmod p$$
Eğer $\text{src2} = 0$ ise, bu işlem matematiksel olarak tanımsızdır. VM yürütme aşamasında $\text{src2} = 0$ durumunda `VmError::DivisionByZero` hatası üreterek durur. AIR tarafında ise bu durum modüler ters eleman kısıtıyla zorunlu kılınır (bkz. Aritmetik Ters Eleman Tanıklığı).

---

## 2. AIR Soundness Eksikliklerinin Kapatılması (Matematiksel Güvenlik)

Matematiksel soundness, trace matrisinin her bir satırı ve sütunu arasındaki ilişkilerin hiçbir açık kapı bırakmayacak şekilde AIR (Algebraic Intermediate Representation) denklemleriyle kilitlenmesini gerektirir.

### R0 Register Soundness Açığı ve Çözümü
BudZKVM'de `R0` yazmacı (register) donanımsal olarak her zaman sıfır (`0`) değerine sabitlenmiştir. Ancak ilk trace nesil kodlarında stack işlemleri (`Push`, `Call`, `Ret`) arka planda çalışırken `dst_idx = 0` değerini alıyor ve bu satırlarda `dst_val` (ve dolayısıyla `COL_RD_VAL_NEW`) hücresine sıfır dışı değerler yazılabiliyordu. 

Kötü niyetli bir kanıtlayıcı, bu zafiyeti kullanarak stack adımlarında `R0` hücresine sahte ara değerler enjekte edebilir ve LogUp CTL (Cross-Table Lookup) veriyolunu bozarak registers tablosu ile CPU tablosunu tutarsız hale getirebilirdi.

Bu soundness açığı iki aşamalı olarak kapatıldı:
1. **VM Trace Düzeyi:** `trace_matrix` oluşturulurken, eğer bir adımda hedef yazma yazmacı `0` ise (`dst_idx == 0`), trace hücresindeki `COL_RD_VAL_NEW` değeri kesin olarak `0` değerine zorlanır:
   ```rust
   let dst_val = if dst_idx == 0 { 0 } else { step.dst_val };
   ```
2. **AIR Düzeyi:** Register LogUp CTL (Cross-Table Lookup) ve AIR kısıtlamalarında, hedef yazmacın `0` olduğu her durumda yazılan değerin `0` olması cebirsel olarak kısıtlandı:
   ```rust
   // R0 her zaman sıfırdır kısıtı
   builder.when(cur[COL_DST_IDX_IS_ZERO].clone()).assert_zero(rd_val_new.clone());
   ```

### Preprocessed Trace ve Padding Soundness (Dolgu Satırı Sızıntısı)
ZK-STARK sistemlerinde trace matrisinin boyutu $2^N$ olmak zorundadır. Ancak gerçek bir program 5 adımda bitebilir. Bu durumda matrisin geri kalan 11 satırı **Padding (Dolgu)** satırları ile doldurulur.

Eğer padding satırları AIR ve CTLlookup denklemlerinden doğru şekilde yalıtılmazsa, kötü niyetli bir kanıtlayıcı dolgu satırlarında hayali program komutları çalıştırarak verifier'ı aldatabilir.

> [!CAUTION]
> **Padding Soundness Açığı:** Prover'ın, programın bittiği (HALT) satırlardan sonraki dolgu satırlarını kullanarak LogUp veriyoluna sahte program okuma/yazma (lookup) istekleri ekleyebilmesi ve verifier'ın bunu fark edememesi.

Bu açığı kapatmak için şu mimariyi entegre ettik:
1. **CPU Aktiflik Ayrımı (`COL_CPU_ACTIVE`):** Trace matrisine sadece programın gerçek adımlarında `1`, dolgu (padding) adımlarında `0` değerini alan bir aktiflik sütunu eklendi.
2. **Preprocessed Active Kolonu:** Program bytecode'unun lookup doğrulaması (Program CTL) yapılırken, sadece `COL_CPU_ACTIVE = 1` olan satırlar LogUp kümesine dahil edildi:
   ```rust
   // Preprocessed aktiflik kolonu ve memory lookup
   let term = alpha + memory_addr + memory_val;
   s_mem_next = s_mem + is_active * inv(term);
   ```


3. **Derece (Degree) Hizalaması:** Prover ve verifier'ın padding kararlarını aynı cömertlikle hesaplaması için trace derecesi formülü senkronize edildi:
   $$\text{degree} = (3 \cdot n_{\text{cpu}} + 1).\text{next\_power\_of\_two}().\text{max}(16)$$

### Aritmetik Ters Eleman Tanıklığı (Arithmetic Inverse Witness)
AIR denklemlerinde `if (x != 0)` koşulunu yazmak doğrudan imkansızdır. Polinomlar sürekli fonksiyonlar olduğundan, sıfır dışılık durumunu kanıtlamak için **Aritmetik Ters Eleman Tanığı (Arithmetic Inverse Witness)** yöntemi kullanılır.

Cebirsel kural şudur: Bir $x$ elemanının sıfır olmadığını kanıtlamak için, prover trace içine yardımcı bir $v$ (inverse) sütunu koyar. AIR bu $v$ değerinin gerçekten $x$'in tersi olduğunu ve $x \neq 0$ durumunu şu denklemlerle doğrular:
1. $$x \cdot (1 - x \cdot v) = 0$$
2. Eğer $x \neq 0$ ise, $x \cdot v = 1$ olmak zorundadır.

BudZKVM'de bu mekanizma şu işlemler için tam olarak implement edilmiştir:
* **`Div` (Bölme):** Paydanın sıfır olmadığı kanıtlanır ve tersi ile çarpım doğrulanır.
* **`Eq` / `Neq` (Eşitlik / Eşitsizlik):** İki değerin farkı $d = A - B$ hesaplanır. Farkın tersi $v = d^{-1}$ tanık olarak trace'e eklenir. Eşitlik durumu bu ters eleman üzerinden doğrulanır.
* **`Jnz` (Sıfır Değilse Atla):** Koşul yazmacının sıfır dışı olup olmadığı bu ters eleman tanığıyla denetlenerek dallanma doğruluğu kilitlenir.

```rust
// JNZ Aritmetik Ters Eleman Kısıtı (plonky3_air.rs)
// cond * (1 - cond * cond_inv) = 0
builder.when(cur[COL_IS_JNZ].clone()).assert_zero(
    cond.clone() * (one.clone() - cond.clone() * cond_inv.clone())
);
```

### Halt ve Padding Geçiş Kısıtları
Program bir kez `HALT` opcode'una ulaştığında, sanal makinenin durumunun donması (freeze) gerekir. Aksi takdirde, dürüst olmayan bir prover program bittikten sonra PC'yi veya yazmaçları değiştirebilir.

Yazılan transition kısıtları ile:
* `is_halt` aktif olduğunda, sonraki satırdaki `PC` o anki `PC` ile aynı kalmalıdır:
  $$\text{is\_halt} \cdot (\text{PC}_{\text{next}} - \text{PC}_{\text{current}}) = 0$$
* Registers ve hafıza üzerindeki tüm eyalet güncellemeleri durdurulur ve dolgu adımları boyunca kilitlenir.

### Gas Tüketim Sınırları ve Taşma Kontrolü
Gas limitinin aşılması durumunda VM'in durması yetmez; kanıtlayıcının gas limitini aşan bir trace üretmediği AIR düzeyinde doğrulanmalıdır.
* Her satırdaki gas tüketimi bir önceki satıra göre birikimli olarak artar:
  $$\text{gas\_used}_{\text{next}} - (\text{gas\_used}_{\text{current}} + \text{gas\_cost}_{\text{current}}) = 0$$
* En son satırda (veya program boyunca her adımda) toplam harcanan gas miktarının, belirlenen limiti aşmadığı inequality range check veya public input sınırları ile zorunlu kılınır:
  $$\text{gas\_used} \le \text{gas\_limit}$$

---

## 3. Serileştirme ve Public Input Güvenliği (Serialization & Public Inputs Envelope)

ZK-STARK ispatları üretildikten sonra ağlar (L1/L2 düğümleri) arasında taşınır. Bu taşıma ve doğrulama katmanındaki zafiyetler tüm sistemi çökertebilir.

### ExecutionPublicInputs ve Keccak256 Bağlamı
Verifier, ispatı doğrulamak için public input değerlerine ihtiyaç duyar. Eğer bu girdiler prover ile verifier arasında esnek ve korumasız taşınırsa, prover verifier'a gönderdiği public input parametrelerini yolda değiştirebilir.

Bu zafiyeti engellemek için **Keccak256 hash bağlamı** kurduk:
1. `ExecutionPublicInputs` yapısı, yürütme için kritik olan tüm parametreleri canonical byte serileştirmesiyle paketler:
   ```rust
   pub struct ExecutionPublicInputs {
       pub program_hash: [u8; 32],
       pub pre_state_root: [u8; 32],
       pub post_state_root: [u8; 32],
       pub gas_used: u64,
       pub gas_limit: u64,
       pub exit_code: u32,
       pub chain_id: u64,
   }
   ```
2. Bu byte dizisi Keccak256 ile hash'lenir ve elde edilen tek bir `public_input_hash` STARK ispatının transcriptine (Fiat-Shamir seed) eklenir.
3. Verifier, ispatı doğrulamadan önce kendi elindeki verilerden bu hash'i yeniden hesaplar ve STARK açılışında doğrular. Böylece en ufak bir parametre değişikliği (örneğin chain_id veya gas_used tahrifatı) ispatın geçersiz kılınmasına neden olur.

### Güvenli ProofEnvelope ve Bincode Bounded Decoding
Rust ekosistemindeki `bincode 1.3` sürümü, sınırlandırılmamış girdi boyutu çözme (unbounded decoding) işlemlerinde bellek tüketim zafiyetlerine (RustSec zafiyeti) sahiptir. Kötü niyetli bir saldırgan, verifier RPC düğümüne devasa boyutlu geçersiz bir proof byte dizisi göndererek düğümü çökertebilir (Denial of Service - DoS).

Bunu engellemek için proof taşıma katmanını **`ProofEnvelope`** ile sarmaladık:
* `ProofEnvelope` içinde versiyon bilgisi (`version: u32`), kullanılan backend (`backend: String`) ve asıl ispat byte dizisi bulunur.
* Verifier tarafındaki deserialization (byte çözme) işlemi kesinlikle sınırlandırılmış (bounded) bincode ayarlarıyla çalışır:
  ```rust
  let reader = bincode::options()
      .with_limit(10 * 1024 * 1024) // Maksimum 10 MB sınırı
      .with_fixint_encoding();
  let envelope: ProofEnvelope = reader.deserialize(bytes)?;
  ```
Bu basit ama kritik önlem, üretim ortamındaki L1 doğrulama düğümlerini DoS saldırılarına karşı korur.

---

## 4. Operasyonel Güvenlik, Eyalet Modeli ve CLI (Operational State & CLI Validation)

Kod güvenliği sadece matematikle bitmez. CLI arayüzü ve yerel dosya sistemindeki eyalet (state) yönetiminin de operasyonel olarak güvenli olması şarttır.

### StateBackend Commit ve Rollback Mekanizması
BudZKVM artık bir akıllı sözleşme eyaleti taşımaktadır. İşlem yürütülürken storage güncellemeleri anında diske yazılırsa ve yarı yolda bir `OutOfGas` hatası veya prover doğrulama hatası oluşursa eyalet tutarsız (corrupted) kalır.

Bunu önlemek için işlemsel (transactional) bir **`StateBackend`** tasarımı uyguladık:
* `StateBackend` yapısı içindeki güncellemeleri geçici bir günlük (journal/backup) üzerinde biriktirir.
* Eğer yürütme tamamen başarılı olursa ve üretilen STARK ispatı doğrulanırsa `commit()` çağrılır ve güncellemeler kalıcı eyalete uygulanır.
* Yürütme sırasında bir hata oluşursa veya üretilen ispat verifier tarafından doğrulanamazsa `rollback()` tetiklenerek eyalet eski tutarlı durumuna anında geri döndürülür.

#### Sıkı Kapsülleme ve CLI Entegrasyonu (Encapsulation)
Operasyonel güvenliği en üst düzeye çıkarmak için, `State` yapısının içindeki `accounts` HashMap'ini kesin olarak **private** hale getirdik. CLI (`bud-cli`) veya herhangi bir harici modül artık doğrudan bu harita üzerinde okuma/yazma gerçekleştiremez. 

Bunun yerine, yürütme boru hattı (`run_pipeline`) tüm eyalet erişimlerini `StateBackend` trait'inin sunduğu güvenli metotlar (`get_account`, `set_account`, `begin_transaction`, `commit`, `rollback`) üzerinden yürütür. Bu sayede:
1. Yürütme başında bir işlem günlüğü (`begin_transaction`) başlatılır.
2. İşlem sonucuna göre ya atomik disk yazımı tetiklenir (`commit`) ya da tüm tahrifatlar geri alınır (`rollback`).
3. State serileştirme işlemleri de `save_to` metodu ile doğrudan `State` yapısı içinden atomik işletim sistemi komutlarıyla yönetilir.

### 64 Derinlikli Sparse Merkle Tree (SMT) Eyalet Kökü
Erken sürümlerde eyalet kökü (state root), tüm hesapların düz bir Keccak256 hash'iydi. Düz hash modelleri ZK sistemleri için uygun değildir çünkü L1/L2 düğümlerinin kısmi eyalet kanıtlarını (inclusion proofs) doğrulamasına izin vermez.

BudZKVM eyalet kökü altyapısını **64 Derinlikli Sparse Merkle Tree (SMT)** mimarisiyle yeniden inşa ettik:
* **Anahtarlar (Keys):** Account ID'leri (u64) 256-bit dizisine dönüştürülerek ağaçta yaprak koordinatlarını belirler.
* **Yapraklar (Leaves):** Her bir aktif hesabın `nonce`, `balance`, `code_hash` dan `storage_root` alanları birleştirilerek hash'lenir (`hash_account`).
* **Boş Alt Ağaçlar (Sparse Subtrees):** Ağacın çoğunluğu boş hesaplardan oluştuğu için, $O(2^{64})$ işlem maliyetini önleyen önceden hesaplanmış `EMPTY_HASHES` önbelleği kullanılır.
* **Merkle İspatları (SMT Proofs):** `get_account_proof` metodu ile bir hesabın eyalette var olduğunu (Inclusion Proof) veya olmadığını (Non-membership Proof) kanıtlayan $O(\log n)$ (64 hash boyutu) Merkle ispatları üretilir. `verify_account_proof` fonksiyonu ile de bu kanıtlar saniyeler içinde doğrulanabilir.

### State Root Domain Separation
Farklı eyalet veya ağ sürümleri arasındaki çakışmaları engellemek için, Keccak256 eyalet kökü hesaplanırken **Domain Separation** (alan ayrımı) ön eki eklenmiştir:
```rust
let domain_prefix = b"BUDZKVM_STATE_ROOT_V1";
let mut hasher = Keccak::v256();
hasher.update(domain_prefix);
hasher.update(&bytes);
```
Bu ön ek sayesinde, BudZKVM eyalet kökleri diğer Keccak256 hash kullanan sistemlerden kriptografik olarak yalıtılır ve çakışma (collision) saldırıları önlenir.

### CLI Atomik Eyalet Yazımı ve Hizalama Kontrolleri
* **Atomik Kaydetme (Atomic Rename):** Eyalet dosyası (`state.json`) güncellenirken doğrudan üzerine yazılmaz. Önce geçici bir dosyaya (`state.json.tmp`) yazılır, diske senkronize edilir (`fsync`) ve ardından işletim sistemi düzeyinde atomik olarak asıl dosyanın üzerine taşınır (`rename`). Bu sayede elektrik kesintisi veya ani çökmelerde eyalet dosyası asla bozulmaz.
* **8-Byte Bytecode Alignment:** CLI üzerinden bytecode okunurken, verilerin 8-byte hizalı (aligned) olup olmadığı ve eksik/artık byte kalıp kalmadığı sıkı şekilde denetlenir. Hizalama dışı bytecode'lar çalıştırılmadan doğrudan reddedilir.

---

## 5. Güvenlik Test Matrisi ve Negatif Doğrulama (Testing Soundness Negatives)

Bir soundness kısıtının gerçekten çalışıp çalışmadığını test etmenin tek yolu, sisteme **geçersiz bir trace** verip verifier'ın bunu reddettiğini görmektir. Buna **Negatif Test** (Negative Testing) denir.

BudZKVM'de bu amaçla `bud-proof/tests/soundness_negatives.rs` entegrasyon test paketini yazdık.

### PC Tahrifatı Negatif Testi
Bu testte, geçerli bir programın (`ADD + HALT`) normal sanal makine izini alıyoruz. Ancak bu iz matrisinin içindeki `PC` (Program Counter) kolonundaki değerleri değiştirip sahte bir PC (`999`) enjekte ediyoruz.

```rust
// soundness_negatives.rs
let mut values = vec![Goldilocks::new(0); 16 * TRACE_WIDTH];
for (i, step) in vm.trace.iter().enumerate() {
    let row_start = i * TRACE_WIDTH;
    values[row_start] = Goldilocks::new(i as u64); // clk
    values[row_start + 1] = Goldilocks::new(999);  // TAMPERED PC!
    ...
}
```

Bu tahrif edilmiş matris Plonky3 prover ve verifier hattına sokulduğunda:
* Verifier, Out-of-Domain (OOD) evaluation adımında, geçiş polinomunun sıfır çıkmadığını (`nxt_pc - cur_next_pc != 0`) fark eder.
* Kriptografik doğrulama aşamasında ispat **`OodEvaluationMismatch`** hatasıyla anında reddedilir.

Bu negatif testlerin yeşil geçmesi (yani ispatın başarısız olması), AIR kısıtlarımızın ve soundness güvenliğimizin üretim ortamında tıkır tıkır çalıştığının en büyük kanıtıdır.

---

## Özet ve Audit Sonrası Kontrol Listesi

BudZKVM'i sıfırdan inşa ederken ve üretime hazırlarken uyguladığımız bu adımlar, onu sadece çalışan bir VM olmaktan çıkarıp, finansal düzeyde güvenlik sunan bir ZKVM haline getirmiştir. Bir ZKVM tasarlarken veya incelerken şu checklist her zaman elinizin altında olmalıdır:

1. **[x] R0 Koruması:** `R0` yazmacına yapılan her türlü yazma işleminin cebirsel olarak `0` değerine zorlandığından emin olun.
2. **[x] Padding İzolasyonu:** HALT satırlarından sonraki dolgu satırlarının LogUp CTL/lookup terimlerini kirletmediğini aktiflik selectorleri ile doğrulayın.
3. **[x] Aritmetik Ters Elemanlar:** Sıfır dışılık gerektiren dallanma ve bölme kurallarında ters eleman tanığının ($v$) cebirsel denklemlerle kilitlendiğinden emin olun.
4. **[x] Public Input Bağlayıcılığı:** Girdilerin Keccak256 hash'inin transcript'e tohum (seed) olarak beslendiğini doğrulayın.
5. **[x] Deserialization Güvenliği:** Byte çözme işlemlerinde DoS saldırılarını önlemek için boyut sınırları (bounded decoders) uygulayın. (postcard + MAX_PROOF_BYTES)
6. **[x] Negatif Testler:** Kritik AIR kurallarını tahrif eden negatif testler yazarak verifier'ın bunları reddettiğini kod düzeyinde kanıtlayın. (8 negatif test)
7. **[x] Comparison Soundness:** 64-bit decomposition + equality prefix flags ile Lt/Gt/Lte/Gte constraint'leri.
8. **[x] Bitwise Soundness:** Bit decomposition + cebirsel esdegerlik ile And/Or/Xor/Not constraint'leri.
9. **[x] Hash Soundness:** Poseidon4 (alpha=7, Goldilocks) ile deterministik hash, round 0 S-box AIR dogrulamasi.
10. **[x] Storage Soundness:** STORAGE_BASE adresleme ile memory LogUp'a dahil edilmis storage consistency.
11. **[x] Merkle Soundness:** poseidon4_hash tabanli 64-depth Merkle dogrulama, boolean output constraint.

> **Faz 0 Tamamlandi (2026):** Tum checklist maddeleri karsilanmistir. 31 opcode production-ready, 51 test (8 negatif dahil). Detayli dokumantasyon icin [Bolum 9: Faz 0 Stabilizasyonu](09_faz0_stabilizasyon.md).
