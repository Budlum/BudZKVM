# Bölüm 1: ZKVM Nedir ve Neden Kendi ZKVM'imizi Yapıyoruz?

Hoş geldin! Bir sanal makine (Virtual Machine) veya derleyici yazmak yazılım mühendisliğinin en zevkli konularından biridir. İşin içine "Zero-Knowledge" (Sıfır Bilgi) kanıtları girdiğinde ise bu konu hem büyüleyici hem de zorlu bir hal alır.

Bu bölümde temel kavramları oturttuktan sonra mimari kararlarımızın arkasındaki nedenleri inceleyeceğiz.

## Geleneksel VM vs. ZKVM

Geleneksel bir sanal makineyi (örneğin JVM, EVM veya bir WASM motoru) bir durum makinesi (state machine) olarak düşünebiliriz. Kod (bytecode) ve başlangıç durumu (initial state) içeri girer, VM talimatları adım adım işletir ve ortaya yeni bir durum (final state) çıkar. 

**Peki bu durumun doğru hesaplandığına nasıl güveniriz?**
Geleneksel dünyada tek yol **yeniden çalıştırmaktır (Re-execution)**. Eğer ben bir programın sonucunun `X` olduğunu iddia ediyorsam, sen de aynı programı kendi bilgisayarında çalıştırıp `X` sonucunu elde edip etmediğine bakarsın. Blockchain dünyasında on binlerce node'un aynı kodu tekrar tekrar çalıştırmasının (örneğin Ethereum'da) sebebi budur. Bu çok yavaş ve pahalıdır.

**ZKVM (Zero-Knowledge Virtual Machine)** ise bu paradigmayı değiştirir. ZKVM, kodu çalıştırırken aynı zamanda bu çalıştırmanın **matematiksel bir kanıtını (proof)** üretir. 
* Kanıtı üretmek (Proving) zordur ve donanım gerektirir.
* Ancak kanıtı doğrulamak (Verifying) inanılmaz derecede hızlıdır (milisaniyeler sürer) ve tekrar çalıştırmaya gerek bırakmaz.

## Neden Kendi ZKVM'imizi Yapıyoruz?

Piyasada RISC Zero, SP1, Cairo gibi harika ZKVM'ler var. Neden oturup "BudZKVM" adında kendi sanal makinemizi sıfırdan yazalım?

1. **Öğrenme ve Hakimiyet:** Bir ZKVM'in "içini açıp" nasıl çalıştığını anlamanın en iyi yolu onu yapmaktır. Polinomların, AIR (Algebraic Intermediate Representation) kısıtlamalarının ve CPU mimarisinin nasıl bir araya geldiğini ancak ellerinizi kirleterek öğrenebilirsiniz.
2. **Özelleştirme (Customization):** Genel amaçlı bir ZKVM (örneğin RISC-V tabanlı) her şeyi yapabilir ancak bazı işlemlerde yavaş kalabilir. Kendi uygulamanıza veya blockchain'inize özel kriptografik opcodelar (örneğin yerleşik Keccak veya Poseidon hash komutları) eklemek isterseniz, kendi ISA'nize sahip olmak büyük avantajdır.
3. **Performans:** ZKVM mimarisinde, VM'in "ZK-Friendly" (Sıfır Bilgi Kanıtlarına uygun) tasarlanması gerekir. Geleneksel mimariler ZK devresi (circuit) içine oturtulurken devasa kanıt boyutları ortaya çıkarabilir. BudZKVM, tamamen ZK-Friendly olması için register erişimlerinden kontrol akışına (control flow) kadar özel olarak tasarlanmıştır.

## ZK-Friendly Tasarım Ne Demektir?

Eğer bir yazılımcıysanız, bir if-else bloğu veya bir array erişimi sizin için çok ucuzdur. Ancak bir ZK kanıtlayıcı (Prover) için:
* `if-else` koşulları polinom denklemleriyle (dereceyi artırmadan) ifade edilmelidir.
* Rastgele bellek (RAM) erişimi çok pahalıdır. RAM, polinomlar dünyasında bir tablo ve o tabloda arama (lookup) işlemi demektir.
* Standart 32-bit veya 64-bit tamsayı (integer) matematiği ZK'da zordur, çünkü ZKVM'ler bir "Prime Field" (Asal Cisim) üzerinde, örneğin modülo bir asal sayı üzerinden işlem yapar. 

İşte "Crafting a ZKVM" süreci, geleneksel yazılım mühendisliği ile bu polinom matematiğinin kısıtlamalarını uyum içinde çalıştırma sanatıdır.

Bir sonraki bölümde, sanal makinemizin konuşacağı dili, yani **ISA (Instruction Set Architecture)** ve **Bytecode** tasarımını (`bud-isa`) inceleyerek donanımın en alt katmanından inşaya başlayacağız.
