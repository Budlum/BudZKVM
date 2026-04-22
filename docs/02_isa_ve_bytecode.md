# Bölüm 2: Komut Seti Mimarisi ve Bytecode (bud-isa)

Bir Sanal Makine (VM) inşa etmenin ilk adımı, makinenin anlayacağı dili tasarlamaktır. Bu dile **Instruction Set Architecture (ISA)**, yani Komut Seti Mimarisi denir. ISA, VM'in donanımının (veya yazılım emülasyonunun) dış dünyayla olan sözleşmesidir.

BudZKVM için `bud-isa` isimli ayrı bir crate (Rust kütüphanesi) oluşturduk. Neden ayrı? Çünkü bu dil tanımını hem VM (çalıştırmak için), hem Compiler (kodu derlemek için), hem de Prover (kanıtlamak için) ortak olarak kullanacaktır.

## Register Tabanlı vs. Stack Tabanlı

Sanal makineler genellikle ikiye ayrılır:
1. **Stack Tabanlı (Örn: EVM, JVM):** İşlemler bir yığın (stack) üzerinden yapılır. `PUSH 5`, `PUSH 3`, `ADD` gibi. Gerçeklemesi kolaydır, derleyici yazması görece kolaydır ancak aynı işlemi yapmak için çok fazla komut gerekir. STARK kanıtlayıcılarında "Stack Derinliği"ni takip etmek ZK açısından karmaşık (ve masraflı) olabilir.
2. **Register Tabanlı (Örn: LuaVM, ARM, RISC-V, BudZKVM):** Veriler CPU içindeki sınırlı sayıdaki "Register"larda (yazmaç) tutulur. `ADD R1, R2, R3` (R2 ile R3'ü topla, R1'e yaz) gibi. Komutlar daha uzundur ama daha az adımda daha çok iş yapılır. ZKVM'ler için tablo yapısına (Trace) çok daha kolay oturtulur.

**Karar:** BudZKVM **Register tabanlı** bir mimari kullanır. 32 adet genel amaçlı (R0'dan R31'e) register'ımız vardır.

## Bir Komutun (Instruction) Yapısı

Bir CPU komutu havada uçuşan sihirli kelimeler değil, basit birer sayıdır (Bytecode). BudZKVM'de her bir komut `u64` (64-bit işaretsiz tamsayı) olarak temsil edilebilir ancak biz tasarım gereği komutları daha okunabilir ve kolay "decode" edilebilir bir struct olarak tanımladık:

```rust
pub struct Instruction {
    pub opcode: Opcode,  // Hangi işlem yapılacak? (Örn: ADD, LOAD, JMP)
    pub dst: u8,         // Sonuç hangi register'a yazılacak?
    pub src1: u8,        // İlk argüman hangi register'dan okunacak?
    pub src2: u8,        // İkinci argüman hangi register'dan okunacak?
    pub imm: i32,        // Sabit (Immediate) bir değer var mı?
}
```

### 1. Opcodes (İşlem Kodları)

VM'imizin yapabildiği temel işlemlerin listesi `bud-isa/src/lib.rs` içinde tanımlıdır:

* **ALU (Aritmetik/Mantık):** `Add`, `Sub`, `Mul`
* **Karşılaştırma:** `Eq` (Eşit mi?), `Lt` (Küçük mü?)
* **Kontrol Akışı (Control Flow):** `Jmp` (Koşulsuz atla), `Jnz` (Sıfır değilse atla), `Halt` (Programı bitir)
* **Veri Taşıma:** `Load` (Immediate değeri register'a yükle)
* **Özel ZK Kodları:** `Assert` (Durumu kanıtla), `Log` (Kanıt dışı konsola yaz)

Her opcode'un arkasında bir sayısal karşılık (discriminant) vardır. Örneğin `Add` işlemi `0x01`'dir. ZK Prover (Kanıtlayıcı) bu sayıları baz alarak hangi polinom kısıtlamasının (constraint) aktifleşeceğine karar verir.

### 2. Immediate (Sabit) Değerler

Bir register'a "10" sayısını koymak isterseniz, `10` sayısı hafızada başka bir yerde mi durmalı, yoksa komutun içinde mi gelmeli? Komutun içine gömülen bu sabit sayılara **Immediate** denir.

BudZKVM'de `Load R1, 10` komutu, `dst = 1`, `imm = 10`, `opcode = Load` anlamına gelir.

## ZK-Friendly Encoding (ZK Dostu Kodlama)

Geleneksel VM'lerde bu `Instruction` struct'ı, bit-shifting yöntemleriyle tek bir 32-bit integer içine sıkıştırılır (Örn: `0b00000001_00000001_00000000_00001010`). Ancak ZKVM'lerde bit-shifting (bit kaydırma) polinomlar dünyasında çok "pahalı" bir işlemdir. Asal sayılar üzerinden çalıştığımız için bit-level operasyonlar karmaşık tablolar gerektirir.

Bu yüzden STARK temelli VM'lerde, komutların Decode (çözülme) işlemini ZKVM'e yaptırmaktan olabildiğince kaçınırız. 
**Püf Nokta:** BudZKVM'de `Instruction` bileşenleri (`opcode`, `dst`, `src1`, `src2`, `imm`) Execution Trace (Çalıştırma İzi) matrisinde ayrı ayrı sütunlara yerleştirilir. Böylece Prover, bit parçalama yapmak zorunda kalmaz, direkt sütun değerlerini alıp matematiksel denkleme koyar.

Bir sonraki bölümde, bu komut setini alıp canlandıran, bellek yönetimini yapan ve bytecode'u adım adım çalıştıran **Sanal Makine'yi (VM)** (`bud-vm`) inşa etmeye başlayacağız.
