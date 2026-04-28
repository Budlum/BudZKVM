# Bölüm 6: Derleyici ve Ekosistem (bud-compiler & bud-cli)

Artık elimizde komut setini anlayan (ISA), bu komutları çalıştırıp Execution Trace üreten bir sanal makine (VM) ve bu trace'in doğruluğunu matematiksel olarak kanıtlayan bir ZK Prover (Plonky3) var. 

Ancak bir sorun var: Hiçbir geliştirici oturup `Instruction { opcode: Add, dst: 1, src1: 2, src2: 3, imm: 0 }` şeklinde elle bytecode yazmak istemez. Geliştiricilerin `let a = b + c;` gibi yüksek seviyeli kodlar yazabilmesi gerekir. İşte bu noktada **Derleyici (Compiler)** devreye girer.

## Bud Derleyicisi (bud-compiler)

Projemizdeki `bud-compiler` crate'i, Bud adını verdiğimiz yüksek seviyeli veya assembly benzeri basit dili alıp, bizim VM'imizin anladığı bytecode'a çevirir. Bir derleyici yazmak başlı başına bir sanat olsa da, temel adımları şunlardır:

1. **Lexer (Sözcük Analizi):** Kaynak kodunu karakter karakter okuyup anlamlı kelimelere (Token'lara) böler. Örneğin `let x = 5;` ifadesi şu tokenlara dönüşür: `[LET, IDENT(x), EQ, NUMBER(5), SEMICOLON]`.
2. **Parser (Sözdizimi Analizi):** Token dizisini alıp bir "Abstract Syntax Tree" (Soyut Sözdizimi Ağacı - AST) oluşturur. Bu ağaç kodun mantıksal yapısını yansıtır.
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

Tebrikler! Sıfırdan başlayarak, kendi komut setini tanımlayan, kodu çalıştıran ve sonucun doğruluğunu kriptografik olarak kanıtlayan tam teşekküllü bir ZKVM tasarladınız.

**Peki Sırada Ne Var?**
* **Memory ve Storage Chiplet:** Şu anda register tablosu üzerinden consistency (tutarlılık) sağlıyoruz. Aynı mantığı (LogUp veya Permutation Argument) kalıcı depolama (RAM/Storage) için kurarak karmaşık akıllı sözleşmeleri destekleyebilirsiniz.
* **Continuations (Süreklilik):** RAM ve işlem gücü limitleri yüzünden trace boyutu çok büyüyemez. Çok büyük programları kanıtlamak için Execution Trace'i parçalara bölüp (chunk) ayrı ayrı kanıtlamanız ve sonra bunları birleştirmeniz (Recursive Proofs) gerekir.
* **Contract State Bridge:** Budlum L1 içindeki account state/storage ile BudZKVM `SRead/SWrite` alanını daha güçlü bir state root protokolüne bağlamak gerekir.

Bu rehber, devasa ZK okyanusunda sadece bir başlangıçtı. Artık "ZKVM Nasıl Çalışır?" sorusuna verebilecek koda dayalı, pratik bir yanıtınız var. 

Mutlu kodlamalar!
