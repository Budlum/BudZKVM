# Bölüm 6: Derleyici ve Ekosistem (bud-compiler & bud-cli)

Artık elimizde komut setini anlayan (ISA), bu komutları çalıştırıp Execution Trace üreten bir sanal makine (VM) ve bu trace'in doğruluğunu matematiksel olarak kanıtlayan bir ZK Prover (Plonky3) var. 

Ancak bir sorun var: Hiçbir geliştirici oturup `Instruction { opcode: Add, dst: 1, src1: 2, src2: 3, imm: 0 }` şeklinde elle bytecode yazmak istemez. Geliştiricilerin `let a = b + c;` gibi yüksek seviyeli kodlar yazabilmesi gerekir. İşte bu noktada **Derleyici (Compiler)** devreye girer.

## Bud Derleyicisi (bud-compiler)

Projemizdeki `bud-compiler` crate'i, Bud adını verdiğimiz yüksek seviyeli veya assembly benzeri basit dili alıp, bizim VM'imizin anladığı bytecode'a çevirir. Bir derleyici yazmak başlı başına bir sanat olsa da, temel adımları şunlardır:

1. **Lexer (Sözcük Analizi):** Kaynak kodunu karakter karakter okuyup anlamlı kelimelere (Token'lara) böler. Örneğin `let x = 5;` ifadesi şu tokenlara dönüşür: `[LET, IDENT(x), EQ, NUMBER(5), SEMICOLON]`.
   
   > [!NOTE]
   > **Yorum Satırı Desteği:** Lexer katmanında tek satırlı (`// ...`) ve çok satırlı blok yorumlar (`/* ... */`) Logos tabanlı kurallarla dinamik olarak taranır ve derleme aşamasına girmeden temiz bir şekilde yoksayılır (`logos::skip`).
   
2. **Parser (Sözdizimi Analizi):** Token dizisini alıp bir "Abstract Syntax Tree" (Soyut Sözdizimi Ağacı - AST) oluşturur. Bu ağaç kodun mantıksal yapısını yansıtır.
   
   #### Operatör Önceliği ve Parantezlerin Çözümü (Operator Precedence)
   Düz ve recursive-descent parser tasarımlarında en sık yapılan hata aritmetik ifadelerin düz bir sırayla (soldan sağa) çözülmesidir. Örneğin `2 + 3 * 4` ifadesinin sonucu düz bir parser ile `20` çıkarken, matematiksel olarak `14` olması gerekir. 
   
   Bud derleyicisinde bu sorunu **Operatör Önceliği (Operator Precedence)** katmanlandırmasıyla çözdük:
   * **`parse_expr`**: Karşılaştırma operatörlerini (`==`, `!=`, `<`, `>`, `<=`, `>=`) çözümler.
   * **`parse_arith`**: Toplama ve çıkarma işlemlerini (`+`, `-`) çözümler.
   * **`parse_term`**: Çarpma ve bölme işlemlerini (`*`, `/`) çözümler.
   * **`parse_primary`**: En yüksek önceliğe sahip olan literal sayıları, hexadecimal sayıları (`0x...`), değişken isimlerini ve **parantez gruplamalarını** (`( ... )`) çözümler.
   
   Bu sayede `(2 + 3) * 4` gibi gruplamalar ve `2 + 3 * 4` gibi öncelikli işlemler matematiksel kurallara tam uyumlu şekilde derlenir.

   #### Paniksiz Hata Yönetimi (Result-Based Parsing)
   Erken aşama derleyici tasarımlarında parser, karşılaştığı herhangi bir sözdizimi hatasında `panic!()` fırlatıyor ve derleyici sınırı `std::panic::catch_unwind` ile bu panikleri yakalıyordu. Bu yaklaşım hem kırılgandır hem de Rust dilinin güvenlik felsefesine aykırıdır.
   
   Parser mimarisini tamamen **Result-based** olacak şekilde yeniden tasarladık:
   * Tüm parse metotları artık `Result<ASTNode, CompileError>` döner.
   * Hata durumunda derleyici paniklemek yerine `CompileError::ParserError(String)` üreterek hatayı yukarıya (`?` operatörüyle) temiz bir şekilde fırlatır.
   * Derleyicinin tüm hata fırlatma durumları `test_parser_error_propagation` negatif testleriyle güvence altına alınmıştır.

3. **Semantic Analyzer (Anlamsal Analiz):** Değişkenler tanımlanmış mı? Tipler uyuşuyor mu? Kullanılmayan değişken var mı? gibi mantıksal hataları yakalar.
4. **Code Generation (Kod Üretimi):** İşte bizim ISA'mız burada devreye girer. AST üzerinde gezilerek (traversal) her bir düğüm için uygun `Instruction` üretilir. Örneğin `x = 5` ifadesi `Load R1, 5` komutuna dönüştürülür.

### Kontrol Akışı: `while` ve `for`

Bud dili artık iki temel döngü formunu destekler:

```bud
while (count < 4) {
    count = count + 1;
}

for i in 0..5 {
    sum = sum + i;
}
```

`while` doğrudan condition + `Jnz` + geri `Jmp` desenine çevrilir. `for i in start..end` ise compiler tarafından şu mantığa indirgenir:

1. `start` bir loop register'ına yüklenir.
2. `end` bir kez hesaplanır ve sabit range sınırı olarak tutulur.
3. Her iterasyonda `loop_reg < end_reg` karşılaştırılır.
4. Gövde çalıştıktan sonra `loop_reg = loop_reg + 1` yapılır.

Bu form yarı-açık aralık kullanır: `0..5`, `0,1,2,3,4` değerlerini üretir.

### Register Tahsisi (Register Allocation)

Derleyici yazmanın en zor kısımlarından biri Register yönetimidir. Bizim 32 adet register'ımız var. Eğer programda 50 tane değişken varsa ne olacak? Derleyici, artık kullanılmayan değişkenlerin (out of scope) register'larını boşa çıkarmalı ve yeni değişkenlere tahsis etmelidir. Çok karmaşık programlarda register'lar dolarsa değişkenler Memory/Storage'a yazılır (Buna "Spilling" denir).

## CLI ile Sistemi Birleştirme (bud-cli)

Tüm bu modülleri bir araya getiren "orkestra şefi" `bud-cli` isimli komut satırı aracıdır.

Sistemin tam akışı şu şekilde işler:
1. Kullanıcı `bud-cli run --program benimkodum.bud` komutunu çalıştırır.
2. CLI, dosyayı okur ve `bud-compiler`'a gönderir. Derleyici bytecode'u (komut listesini) geri döndürür.
3. CLI, bu bytecode'u `bud-vm`'e yükler ve VM'i çalıştırır.
4. VM çalışmasını bitirir ve sonuçlar ile birlikte bir "Execution Trace" (Çalıştırma İzi) üretir.
5. CLI, bu Trace'i alır ve `bud-proof` modülüne (Plonky3) gönderir.
6. Plonky3, AIR kısıtlamalarını kontrol eder, matris matematiğini uygular ve bir **ZK Proof (Sıfır Bilgi Kanıtı)** üretir.
7. İsteğe bağlı olarak bu kanıt, `verify` fonksiyonu kullanılarak çok kısa bir sürede doğrulanır.

Örnek döngü programı repo kökünde bulunur:

```bash
nix develop --command cargo run -p bud-cli -- run --program example_loop.bud
```

Bu örnek hem `for` hem `while` kullanır. Beklenen event çıktısı `[10, 6]` şeklindedir:

* `for i in 0..5`: `0 + 1 + 2 + 3 + 4 = 10`
* `while count < 4`: `0 + 1 + 2 + 3 = 6`

```rust
// bud-cli içinden örnek bir akış
let trace = vm.trace; // VM'in ürettiği loglar
let num_steps = trace.len();

// Kanıt üretme (Ağır İşlem)
let proof = Prover::prove(&trace, num_steps);
println!("Proof generated ({} bytes)", proof.data.len());

// Kanıt doğrulama (Çok Hızlı)
let ok = Prover::verify(&proof, num_steps);
println!("Proof valid: {}", ok);
```

## Budlum L1 Entegrasyonu

BudZKVM bytecode'u artık Budlum L1 `infra` reposu içinde `TransactionType::ContractCall` olarak çalıştırılabilir. Bu entegrasyonda:

1. Client BudZKVM bytecode'u little-endian `u64` instruction byte dizisi olarak `tx.data` alanına koyar.
2. L1 `src/execution/zkvm.rs` bytecode'u decode eder.
3. VM gas limitiyle çalıştırılır.
4. `bud-proof` ile proof üretilir ve verify edilir.
5. Sadece başarılı execution sonrası sender fee ve nonce state'i güncellenir.

Bu sayede CLI'da üretilen bytecode ile L1 transaction payload formatı aynı kalır.

## Sonuç ve Gelecek

Tebrikler! Sifirdan baslayarak, kendi komut setini tanimlayan, kodu calistiran ve sonucun dogrulugunu kriptografik olarak kanitlayan tam tesekkullu bir ZKVM tasarladiniz.

**Faz 0 Tamamlandi (31/31 opcode production, 51 test, 0 failure):**
* Tum opcode'larin AIR constraint'leri tamamlandi (Comparison 64-bit decomposition, Bitwise cebirsel esdegerlik, Poseidon4 hash, Storage STORAGE_BASE memory LogUp, VerifyMerkle poseidon4 tabanli).
* `postcard` serilestirme (bounded, DoS korumali).
* `RUST_LOG=info` ile tum pipeline'da structured tracing.
* 8 negatif test (tampered comparison, bitwise, poseidon S-box, storage, PC, public inputs, program, proof bytes).
* CI: fmt + check + clippy + test + docs link check + cargo deny.

**Sırada Ne Var? (Faz 1: Performans)**
* Benchmark suite (criterion), proving/verification time olcumleri.
* Prover paralellestirme optimizasyonu (Rayon).
* Proof boyut optimizasyonu (FRI parametre tuning).

**Sırada Ne Var? (Faz 2: Dil ve Compiler)**
* Struct/Kayit destegi, Mapping (Map<K,V>), Standart kutuphane.
* Hata mesajlari ve source span iyilestirmesi (miette).
* Debug modu ve step-by-step interactive debugger.

**Sırada Ne Var? (Faz 3: ZK Gelistirmeler)**
* Recursive proof aggregation (coklu transaction -> tek block proof).
* ZK mode (zero-knowledge), Verifier WASM/ EVM target.
* Poseidon multi-round tam AIR dogrulamasi.
