# Bölüm 6: Derleyici ve Ekosistem (bud-compiler & bud-cli)

Artık elimizde komut setini anlayan (ISA), bu komutları çalıştırıp Execution Trace üreten bir sanal makine (VM) ve bu trace'in doğruluğunu matematiksel olarak kanıtlayan bir ZK Prover (Plonky3) var. 

Ancak bir sorun var: Hiçbir geliştirici oturup `Instruction { opcode: Add, dst: 1, src1: 2, src2: 3, imm: 0 }` şeklinde elle bytecode yazmak istemez. Geliştiricilerin `let a = b + c;` gibi yüksek seviyeli kodlar yazabilmesi gerekir. İşte bu noktada **Derleyici (Compiler)** devreye girer.

## Bud Derleyicisi (bud-compiler)

Projemizdeki `bud-compiler` crate'i, Bud adını verdiğimiz yüksek seviyeli veya assembly benzeri basit dili alıp, bizim VM'imizin anladığı bytecode'a çevirir. Bir derleyici yazmak başlı başına bir sanat olsa da, temel adımları şunlardır:

1. **Lexer (Sözcük Analizi):** Kaynak kodunu karakter karakter okuyup anlamlı kelimelere (Token'lara) böler. Örneğin `let x = 5;` ifadesi şu tokenlara dönüşür: `[LET, IDENT(x), EQ, NUMBER(5), SEMICOLON]`.
2. **Parser (Sözdizimi Analizi):** Token dizisini alıp bir "Abstract Syntax Tree" (Soyut Sözdizimi Ağacı - AST) oluşturur. Bu ağaç kodun mantıksal yapısını yansıtır.
3. **Semantic Analyzer (Anlamsal Analiz):** Değişkenler tanımlanmış mı? Tipler uyuşuyor mu? Kullanılmayan değişken var mı? gibi mantıksal hataları yakalar.
4. **Code Generation (Kod Üretimi):** İşte bizim ISA'mız burada devreye girer. AST üzerinde gezilerek (traversal) her bir düğüm için uygun `Instruction` üretilir. Örneğin `x = 5` ifadesi `Load R1, 5` komutuna dönüştürülür.

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

## Sonuç ve Gelecek

Tebrikler! Sıfırdan başlayarak, kendi komut setini tanımlayan, kodu çalıştıran ve sonucun doğruluğunu kriptografik olarak kanıtlayan tam teşekküllü bir ZKVM tasarladınız.

**Peki Sırada Ne Var?**
* **Memory ve Storage Chiplet:** Şu anda register tablosu üzerinden consistency (tutarlılık) sağlıyoruz. Aynı mantığı (LogUp veya Permutation Argument) kalıcı depolama (RAM/Storage) için kurarak karmaşık akıllı sözleşmeleri destekleyebilirsiniz.
* **Continuations (Süreklilik):** RAM ve işlem gücü limitleri yüzünden trace boyutu çok büyüyemez. Çok büyük programları kanıtlamak için Execution Trace'i parçalara bölüp (chunk) ayrı ayrı kanıtlamanız ve sonra bunları birleştirmeniz (Recursive Proofs) gerekir.

Bu rehber, devasa ZK okyanusunda sadece bir başlangıçtı. Artık "ZKVM Nasıl Çalışır?" sorusuna verebilecek koda dayalı, pratik bir yanıtınız var. 

Mutlu kodlamalar!
