# Bölüm 3: Sanal Makine İnşası (bud-vm)

Komut setimizi (ISA) tanımladık. Şimdi bu komutları alıp gerçekten çalıştıracak olan "kalbi", yani Sanal Makineyi (VM) inşa edeceğiz. Bu modüle `bud-vm` adını verdik.

Sıradan bir yazılım geliştiricisi için VM yazmak karmaşık bir `switch-case` döngüsünden ibarettir. Ancak bir **ZKVM** yazdığınızı asla unutmamalısınız. VM'in her adımını öyle bir kaydetmeliyiz ki, daha sonra ZK Prover (Kanıtlayıcı) bu adımları alıp matematiksel denklemlere dökebilsin.

## VM'in Durumu (State)

Bir VM'in anlık halini (State) neler oluşturur?
1. **Program Counter (PC):** Şu an hangi komut satırını çalıştırıyoruz?
2. **Registers:** R0'dan R31'e kadar register'ların o anki değerleri.
3. **Stack:** `Call`, `Ret`, `Push`, `Pop` için kullanılan küçük yürütme yığını.
4. **Memory/Storage:** Uygulamanın geçici memory ve key-value storage alanı.
5. **Gas Sayaçları:** `gas_used` ve `gas_limit`. Sonsuz döngü ve DoS risklerini kesmek için her instruction maliyetlendirir.
6. **Execution Trace (Çalıştırma İzi):** Geçmişte yapılan tüm işlemlerin "log" kayıtları (ZKVM'ler için kritik!).

## Çalıştırma Döngüsü (Fetch-Decode-Execute)

Bir işlemcinin klasik döngüsüdür:

1. **Fetch (Getir):** `PC` değerinin gösterdiği adresten sıradaki komutu al.
2. **Decode (Çöz):** Komutun içindeki Opcode, src1, src2, dst ve imm değerlerini ayrıştır.
3. **Execute (Çalıştır):** Opcode'un gerektirdiği işlemi yap, sonucu `dst` register'ına yaz ve `PC`'yi bir sonraki komuta geçir.

`bud-vm/src/lib.rs` içindeki `step(program)` fonksiyonu tam olarak bunu yapar. Güncel VM'de ilk kural şudur: Eğer VM zaten halt olmuşsa veya `pc` program dışına çıkmışsa yeni trace satırı üretilmez.

```rust
pub fn step(&mut self, program: &[u64]) {
    if self.halted || self.pc >= program.len() {
        self.halted = true;
        return;
    }

    // 1. Fetch
    let raw_inst = program[self.pc];
    let inst = Instruction::decode(raw_inst);
    let cur_pc = self.pc;

    // Her instruction gas tüketir.
    self.consume_gas(Self::gas_cost(inst.opcode));
    
    // 2. Decode
    let src1_val = self.registers[inst.rs1 as usize];
    let src2_val = self.registers[inst.rs2 as usize];

    // 3. Execute
    let (dst_val, next_pc) = match inst.opcode {
        Opcode::Add => {
            let result = src1_val.wrapping_add(src2_val);
            self.registers[inst.rd as usize] = result;
            self.pc += 1;
            (result, cur_pc + 1)
        }
        Opcode::Call => {
            let target = (cur_pc as i64 + inst.imm as i64) as usize;
            self.stack.push((cur_pc + 1) as u64);
            self.pc = target;
            ((cur_pc + 1) as u64, target)
        }
        Opcode::Ret => {
            let target = self.stack.pop().expect("Return stack underflow") as usize;
            self.pc = target;
            (target as u64, target)
        }
        Opcode::Halt => {
            self.halted = true;
            (0, cur_pc)
        }
        // Diğer opcode'lar...
    };

    // Execution Trace'i kaydet!
    self.trace.push(Step {
        pc: cur_pc,
        instruction: inst,
        src1_idx: inst.rs1,
        src2_idx: inst.rs2,
        dst_idx: inst.rd,
        src1_val,
        src2_val,
        dst_val,
        next_pc,
        registers: self.registers,
    });

}
```

Bu küçük guard, prover açısından çok önemlidir. Program dışına çıkan bir branch ya da jump için sahte bir instruction satırı üretmeyiz; VM deterministik olarak halt eder. Böylece trace uzunluğu ve trace içeriği aynı bytecode için her zaman aynıdır.

## Gas Metering

`Vm::new(memory_size)` varsayılan olarak `1_000_000` gas limiti ile gelir. Test ve L1 entegrasyonları için `Vm::with_gas_limit(memory_size, gas_limit)` kullanılabilir.

Gas maliyetleri bilinçli olarak basit tutulmuştur:

* Basit ALU ve branch komutları çoğunlukla `1` gas.
* `Load`, `Store`, `SRead`, `SWrite` gibi memory/storage işlemleri `3` gas.
* `Call`, `Ret`, `Push`, `Pop` `2` gas.
* `Syscall` `5` gas.
* `Poseidon` ve `VerifyMerkle` `10` gas.

Limit aşılırsa VM `Out of gas` hatasıyla durur. Budlum L1 entegrasyonunda bu hata transaction failure'a çevrilir ve sender state'i atomik olarak değişmeden kalır.

Phase 2 kapsamında gas davranışı testlerle sabitlendi. `Load + Push + Syscall + Halt` gibi küçük programlarda `gas_used` tam beklenen toplam maliyeti verir. Sonsuz döngü örneği olan `Jmp 0` ise limit aşıldığında `Out of gas` ile kesilir.

## Deterministik Hata ve Kenar Durumu Semantiği

Bir ZKVM'de "panic attı mı atmadı mı?" gibi davranışların rastlantısal veya Rust build moduna bağlı olması tehlikelidir. Bu yüzden BudVM'de bazı kenar durumlarını açıkça tanımlıyoruz.

### Program Dışı PC

Eğer `pc >= program.len()` ise:

* `halted = true` olur.
* Yeni `Step` satırı eklenmez.
* Register ve memory değişmez.

Bu durum özellikle program dışına sıçrayan `Jmp` ve `Jnz` instruction'ları için önemlidir. Kontrol akışı bir sonraki `step` çağrısında deterministik olarak biter.

### Halt Sonrası Step

`Halt` instruction'ı execute edildikten sonra:

* `pc` aynı kalır.
* Trace'e `Halt` satırı bir kez eklenir.
* Sonraki `step` çağrıları trace'e yeni satır eklemez.
* Register ve memory değişmez.

Bu davranış prover tarafındaki `COL_IS_HALT` kısıtlarını güçlendirmek için temel kabulümüzdür.

### Memory Erişimi

`Load` iki modda çalışır:

* `rs1 == 0` ise `imm` immediate değer olarak `rd` register'ına yazılır.
* `rs1 != 0` ise `register[rs1] + imm` adresinden 8 byte little-endian word okunur.

Geçersiz memory okuması `0` döndürür. Geçersiz memory yazması no-op olur. Geçersiz kabul edilen durumlar:

* Negatif adres.
* `usize` içine sığmayan adres.
* `addr + 8` taşması.
* `addr + 8 > memory.len()`.

Bu davranış `Load` ve `Store` için `memory_word_addr` yardımcı fonksiyonu ile merkezileştirilmiştir.

### Register Erişimi

Normal `rd`, `rs1` ve `rs2` alanları ISA decode sırasında 5 bit ile maskelenir; bu yüzden `0..32` aralığındadır. Ancak `VerifyMerkle`, path register'ını `imm` üzerinden seçer. `imm` negatifse veya register aralığı dışındaysa path değeri `0` kabul edilir. Bu sayede kötü bytecode doğrudan index panic üretmez.

### Aritmetik Semantigi

BudVM aritmetigi Goldilocks asal cismi (P = 2^64 - 2^32 + 1) uzerinde calisir:

* `Add`, `Sub`, `Mul`: wrapping u64 aritmetigi. Debug/release farki yok.
* `Div`: Goldilocks field-native moduler bolme: `rd = rs1 * rs2^{-1} mod P`. Payda sifirsa sonuc 0.
* `Inv`: Moduler ters: `rd = rs1^{-1} mod P`. Girdi sifirsa sonuc 0.
* **`Poseidon`**: 4-round Poseidon hash (alpha=7, width=8). Iki register degerini alir, Goldilocks cisminde Poseidon permutasyonu uygular.
* **`VerifyMerkle`**: 64-depth Merkle proof dogrulama. `rs1` = root, `rs2` = leaf, `imm` = bellek adresi. Bellek layout'u: `[key: u64, 64x sibling: u64]` (520 byte). Her level'de key'in bitine gore `poseidon4_hash` ile hash yonu belirlenir.
* **`Not`**: Lojik NOT — `rs1 == 0` ise 1, degilse 0 dondurur.
* **`Eq/Neq`**: Karsilastirma. `Lt/Gt/Lte/Gte`: 64-bit karsilastirma.
* **`And/Or/Xor`**: Bitwise islemler. `And`: bitwise AND, `Or`: bitwise OR, `Xor`: bitwise XOR.
* **`SRead/SWrite`**: Storage okuma/yazma. `imm` ile belirtilen slot'a erisir. Bellek uzerinde `STORAGE_BASE + slot` adresinde saklanir (LogUp CTL icin).

## Call Stack ve Stack Opcodes

BudZKVM'in ana veri modeli register tabanlıdır, fakat `Call`, `Ret`, `Push`, `Pop` için VM içinde `Vec<u64>` tabanlı bir stack vardır.

* `Call`: dönüş adresini stack'e koyar.
* `Ret`: dönüş adresini stack'ten alır.
* `Push`: `rs1` register değerini stack'e koyar.
* `Pop`: stack'ten aldığı değeri `rd` register'ına yazar.

Stack underflow durumları panic ile yakalanır. Bu davranış, proof/backend katmanında başarısız execution olarak ele alınır.

## Neden Execution Trace (İz) Kaydediyoruz?

Klasik bir VM'de `step` işlemini yapıp eski state'i unuturuz. Fakat ZK dünyasında Prover, **her bir clock cycle'da (saat vuruşunda) ne olduğunu bilmek zorundadır.** Prover'ın işi, *"VM gerçekten bu adımları doğru hesapladı mı?"* sorusunu bir STARK devresi üzerinden kanıtlamaktır.

Bu yüzden VM çalışırken her bir `Step` objesini bir listeye ekleriz. Buna **Execution Trace** denir. Bu liste daha sonra ZK Prover'a gönderilecek ve satır satır, sütun sütun devasa bir matrise (matrix) dönüştürülecektir.

`Step` satırları artık sadece "hangi opcode çalıştı?" bilgisini taşımaz. Her satırda:

* `pc` ve `next_pc`
* decode edilmiş instruction
* `src1_idx`, `src2_idx`, `dst_idx`
* execute öncesi `src1_val`, `src2_val`
* instruction sonucu `dst_val`
* execute sonrası 32 register'lık snapshot

bulunur. Ayrıntılı trace sözleşmesi için [BudVM Trace Schema](vm_trace_schema.md) dokümanına bakın.

## Trace Fixture Testleri

Phase 2'de VM trace davranışını fixture testleriyle sabitledik. Bu testler `bud-vm/tests/trace_fixtures.rs` içinde durur ve üç ana akışı kapsar:

1. Aritmetik: `Load`, `Add`, `Sub`, `Mul`, `Halt`.
2. Kontrol akışı: `Jnz`, `Jmp`, program dışına çıkınca deterministik halt.
3. Memory/storage/event: `Store`, memory `Load`, `SWrite`, `SRead`, `Log`.

Bu testler sadece final register sonucunu kontrol etmez. Her `Step` satırında `pc`, `next_pc`, opcode, operand değerleri ve seçilmiş register snapshot'ları karşılaştırılır. Böylece VM refactor edildiğinde prover'ın beslendiği trace formatı sessizce değişmez.

## Storage ve State Root

Gerçek dünya uygulamalarında (örneğin akıllı sözleşmelerde) sadece register'lar yetmez, key-value bazlı bir "Storage" (depolama) ihtiyacımız vardır.

`bud-vm` içinde, basit bir `HashMap` kullanmak yerine ZK'da kanıtlanabilir bir veri yapısı kullanmamız gerekir. Bu genellikle bir **Merkle Tree (Merkle Ağacı)** veya **Sparse Merkle Tree (SMT)** olur.

Eğer VM `SWrite` (Storage Write) komutunu işletirse, ağaçtaki bir yaprağın değeri güncellenir ve ağacın **Root (Kök)** değeri değişir. Prover, sadece en son Root değerini public input olarak paylaşarak, milyarlarca verilik bir veritabanının bütünlüğünü birkaç byte ile kanıtlamış olur.

Sanal makinemiz artık kodu çalıştırıp Execution Trace'i üretebiliyor. Ancak bu Trace'i ZK matematiğine (polinomlara) oturtmak hiç kolay değil. Bir sonraki bölümde bu mimari sorunu nasıl çözeceğimizi ve **ZK Dostu Mimariyi** inceleyeceğiz.
