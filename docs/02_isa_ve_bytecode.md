# Bölüm 2: Komut Seti Mimarisi ve Bytecode (bud-isa)

Bir Sanal Makine (VM) inşa etmenin ilk adımı, makinenin anlayacağı dili tasarlamaktır. Bu dile **Instruction Set Architecture (ISA)**, yani Komut Seti Mimarisi denir. ISA, VM'in donanımının (veya yazılım emülasyonunun) dış dünyayla olan sözleşmesidir.

BudZKVM için `bud-isa` isimli ayrı bir crate (Rust kütüphanesi) oluşturduk. Neden ayrı? Çünkü bu dil tanımını hem VM (çalıştırmak için), hem Compiler (kodu derlemek için), hem de Prover (kanıtlamak için) ortak olarak kullanacaktır.

## Register Tabanlı vs. Stack Tabanlı

Sanal makineler genellikle ikiye ayrılır:
1. **Stack Tabanlı (Örn: EVM, JVM):** İşlemler bir yığın (stack) üzerinden yapılır. `PUSH 5`, `PUSH 3`, `ADD` gibi. Gerçeklemesi kolaydır, derleyici yazması görece kolaydır ancak aynı işlemi yapmak için çok fazla komut gerekir. STARK kanıtlayıcılarında "Stack Derinliği"ni takip etmek ZK açısından karmaşık (ve masraflı) olabilir.
2. **Register Tabanlı (Örn: LuaVM, ARM, RISC-V, BudZKVM):** Veriler CPU içindeki sınırlı sayıdaki "Register"larda (yazmaç) tutulur. `ADD R1, R2, R3` (R2 ile R3'ü topla, R1'e yaz) gibi. Komutlar daha uzundur ama daha az adımda daha çok iş yapılır. ZKVM'ler için tablo yapısına (Trace) çok daha kolay oturtulur.

**Karar:** BudZKVM **Register tabanlı** bir mimari kullanır. 32 adet genel amaçlı (R0'dan R31'e) register'ımız vardır. **R0 özel bir register'dır: donanımsal olarak her zaman 0 değerini tutar.** Bu sabit (hardwired to zero) özellik, hem VM'in determinizmi hem de STARK soundness'i için kritik önem taşır.

## Bir Komutun (Instruction) Yapısı

Bir CPU komutu havada uçuşan sihirli kelimeler değil, basit birer sayıdır (Bytecode). BudZKVM'de her bir komut `u64` (64-bit işaretsiz tamsayı) olarak kodlanır:

```rust
pub struct Instruction {
    pub opcode: Opcode,  // Hangi işlem yapılacak? (Örn: ADD, LOAD, JMP)
    pub rd: u8,          // Sonuç hangi register'a yazılacak? (destination)
    pub rs1: u8,         // İlk argüman hangi register'dan okunacak? (source 1)
    pub rs2: u8,         // İkinci argüman hangi register'dan okunacak? (source 2)
    pub imm: i32,        // Sabit (Immediate) bir değer var mı?
}
```

### Kodlama Formatı (Encoding)

`Instruction::encode()` her instruction'ı tek bir `u64` olarak paketler:

| Bit Aralığı | Alan | Açıklama |
|-------------|------|----------|
| 0-7 | `opcode` | İşlem kodu (0x00-0x1E) |
| 8-12 | `rd` | Hedef register (0-31) |
| 13-17 | `rs1` | Birinci kaynak register (0-31) |
| 18-22 | `rs2` | İkinci kaynak register (0-31) |
| 23-54 | `imm` | 32-bit işaretli immediate değer |

Bu sayede her instruction 8 byte'lık sabit boyutlu bir kelimedir — bu, L1 entegrasyonunda bytecode hizalama kontrolü için kritik avantaj sağlar.

## Opcodes (İşlem Kodları) ve Üretim Durumu

BudZKVM ISA'sı **Production** ve **Experimental** olarak iki profile ayrılır. Production profilinde yalnızca AIR constraint'leri tamamlanmış, matematiksel olarak sound opcode'lar kullanılabilir. Experimental opcode'lar geliştirme aşamasındadır ve `cfg(feature = "experimental")` olmadan derleme veya çalışma zamanında reddedilir.

### ✅ Production Opcode'lar (Güncel: Faz 0 sonrası)

| Opcode | Hex | Açıklama |
|--------|-----|----------|
| `Halt` | 0x00 | Programı durdur |
| `Add` | 0x01 | `rd = rs1 + rs2` (wrapping) |
| `Sub` | 0x02 | `rd = rs1 - rs2` (wrapping) |
| `Mul` | 0x03 | `rd = rs1 * rs2` (wrapping) |
| `Div` | 0x04 | `rd = rs1 * rs2^{-1} mod P` (Goldilocks bölme) |
| `Inv` | 0x05 | `rd = rs1^{-1} mod P` (modüler ters) |
| `And` | 0x06 | `rd = rs1 & rs2` (bitwise AND) |
| `Or` | 0x07 | `rd = rs1 \| rs2` (bitwise OR) |
| `Xor` | 0x08 | `rd = rs1 ^ rs2` (bitwise XOR) |
| `Not` | 0x09 | `rd = (rs1 == 0) ? 1 : 0` (lojik NOT) |
| `Eq` | 0x0A | `rd = (rs1 == rs2) ? 1 : 0` |
| `Neq` | 0x0B | `rd = (rs1 != rs2) ? 1 : 0` |
| `Lt` | 0x0C | `rd = (rs1 < rs2) ? 1 : 0` (64-bit karşılaştırma) |
| `Gt` | 0x0D | `rd = (rs1 > rs2) ? 1 : 0` |
| `Lte` | 0x0E | `rd = (rs1 <= rs2) ? 1 : 0` |
| `Gte` | 0x0F | `rd = (rs1 >= rs2) ? 1 : 0` |
| `Jmp` | 0x10 | `pc += imm` (koşulsuz atlama) |
| `Jnz` | 0x11 | `rs1 != 0` ise `pc += imm`, değilse `pc += 1` |
| `Call` | 0x12 | Dönüş adresini stack'e koy, `pc += imm` |
| `Ret` | 0x13 | Stack'ten dönüş adresini al, `pc`'yi güncelle |
| `Load` | 0x14 | `rd = memory[rs1 + imm]` veya `rd = imm` (rs1=0 ise) |
| `Store` | 0x15 | `memory[rs1 + imm] = rs2` |
| `Push` | 0x16 | `rs1` değerini stack'e koy |
| `Pop` | 0x17 | Stack'ten değer al, `rd`'ye yaz |
| `Assert` | 0x18 | `rs1 != 0` değilse programı durdur |
| `Poseidon` | 0x19 | `rd = Poseidon4(rs1, rs2)` (4 round, alpha=7) |
| `Log` | 0x1A | `rs1` değerini event log'una ekle |
| `SRead` | 0x1B | `rd = storage[imm]` (storage okuma) |
| `SWrite` | 0x1C | `storage[imm] = rs1` (storage yazma) |
| `Syscall` | 0x1D | `rd = sistem_çağrısı(imm)` |
| `VerifyMerkle` | 0x1E | `rd = MerkleDoğrula(root, leaf, path)` | Poseidon4 tabanlı 64-depth |

> **Faz 0 Sonrası:** Tüm 31 opcode production seviyesindedir. Experimental opcode kalmamıştır. Her opcode'un VM implementasyonu ve AIR constraint'i tamamlanmıştır.

## Bytecode Formatı ve L1 Entegrasyonu

`Instruction::encode()` her instruction'ı tek bir `u64` olarak paketler. CLI ve L1 entegrasyonunda bu değerler little-endian byte dizisine çevrilir:

```rust
let bytes: Vec<u8> = bytecode
    .iter()
    .flat_map(|instruction| instruction.to_le_bytes())
    .collect();
```

Budlum L1 `infra` reposundaki `TransactionType::ContractCall` bu formatı kullanır. `tx.data` alanı boş olamaz ve uzunluğu 8'in katı olmalıdır; her 8 byte bir BudZKVM instruction'ıdır.

## ZK-Friendly Encoding (ZK Dostu Kodlama)

Geleneksel VM'lerde bu `Instruction` struct'ı, bit-shifting yöntemleriyle tek bir 32-bit integer içine sıkıştırılır (Örn: `0b00000001_00000001_00000000_00001010`). Ancak ZKVM'lerde bit-shifting (bit kaydırma) polinomlar dünyasında çok "pahalı" bir işlemdir. Asal sayılar üzerinden çalıştığımız için bit-level operasyonlar karmaşık tablolar gerektirir.

Bu yüzden STARK temelli VM'lerde, komutların Decode (çözülme) işlemini ZKVM'e yaptırmaktan olabildiğince kaçınırız. 
**Püf Nokta:** BudZKVM'de `Instruction` bileşenleri (`opcode`, `rd`, `rs1`, `rs2`, `imm`) Execution Trace (Çalıştırma İzi) matrisinde ayrı ayrı sütunlara yerleştirilir. Böylece Prover, bit parçalama yapmak zorunda kalmaz, direkt sütun değerlerini alıp matematiksel denkleme koyar.
