# Crafting a ZKVM: BudZKVM Rehberi

Bu kitap, sıfırdan bir Sanal Makine (VM) ve bu makine üzerinde çalışan programların doğruluğunu kriptografik olarak kanıtlayabilen bir ZKVM (Zero-Knowledge Virtual Machine) tasarlama rehberidir.

Bu rehber, popüler "Crafting Interpreters" kitabının felsefesini benimseyerek, konuyu tamamen pratik, koda dayalı ve adım adım bir yaklaşımla ele alır. Örnek uygulama olarak **BudZKVM** projesini inceliyoruz.

## Bu Kitap Kimler İçin?
* Kriptografi ve ZK-STARK kavramlarına meraklı geliştiriciler.
* Kendi sanal makinesini, komut setini (ISA) veya derleyicisini yazmak isteyenler.
* Plonky3 gibi modern ZK kanıtlayıcı çerçevelerinin (framework) gerçek dünya projelerinde nasıl kullanıldığını görmek isteyenler.

## BudZKVM Mimarisinin Temel Bileşenleri
BudZKVM, modüler bir yaklaşımla tasarlanmıştır. Kitap boyunca aşağıdaki bileşenleri adım adım inşa edeceğiz:

1. **`bud-isa` (Instruction Set Architecture):** VM'in anladığı donanım komutları ve bu komutların bytecode formatında nasıl kodlandığı.
2. **`bud-vm` (Sanal Makine):** Bytecode'u adım adım çalıştıran (fetch-decode-execute), register ve memory durumunu güncelleyen çekirdek yapı.
3. **`bud-compiler` (Derleyici):** Yüksek seviyeli (ya da assembly benzeri) Bud dilini, `bud-isa` bytecode'una çeviren derleyici.
4. **`bud-proof` (ZK Kanıtlayıcı):** Plonky3 tabanlı, VM'in `Execution Trace`'ini (çalıştırma izi) alıp doğru çalıştığına dair kriptografik kanıt (STARK proof) üreten modül.
5. **`bud-cli` (Komut Satırı):** Tüm bu modülleri bir araya getiren ve kullanıcıya sunan arayüz.

## İçindekiler

1. [Bölüm 1: Giriş - ZKVM Nedir ve Neden Kendi ZKVM'imizi Yapıyoruz?](01_giris.md)
2. [Bölüm 2: Komut Seti Mimarisi ve Bytecode (bud-isa)](02_isa_ve_bytecode.md)
3. [Bölüm 3: Sanal Makine İnşası (bud-vm)](03_virtual_machine.md)
4. [Bölüm 4: ZK Dostu Mimari Tasarımı](04_zk_friendly_architecture.md)
5. [Bölüm 5: STARK, AIR ve Plonky3 (bud-proof)](05_stark_ve_plonky3.md)
6. [Bölüm 6: Derleyici ve Ekosistem (bud-compiler & bud-cli)](06_compiler_ve_ekosistem.md)

---
> **Not:** Bu rehberdeki kod örnekleri Rust dilinde yazılmıştır. Rust'ın temel bellek güvenliği konseptlerine aşina olmak faydalı olacaktır.
