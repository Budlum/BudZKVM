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

`bud-vm/src/lib.rs` içindeki `step(program)` fonksiyonu tam olarak bunu yapar:

```rust
pub fn step(&mut self, program: &[u64]) {
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
    });

}
```

## Gas Metering

`Vm::new(memory_size)` varsayılan olarak `1_000_000` gas limiti ile gelir. Test ve L1 entegrasyonları için `Vm::with_gas_limit(memory_size, gas_limit)` kullanılabilir.

Gas maliyetleri bilinçli olarak basit tutulmuştur:

* Basit ALU ve branch komutları çoğunlukla `1` gas.
* `Load`, `Store`, `SRead`, `SWrite` gibi memory/storage işlemleri `3` gas.
* `Call`, `Ret`, `Push`, `Pop` `2` gas.
* `Syscall` `5` gas.
* `Poseidon` ve `VerifyMerkle` `10` gas.

Limit aşılırsa VM `Out of gas` hatasıyla durur. Budlum L1 entegrasyonunda bu hata transaction failure'a çevrilir ve sender state'i atomik olarak değişmeden kalır.

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

## Storage ve State Root

Gerçek dünya uygulamalarında (örneğin akıllı sözleşmelerde) sadece register'lar yetmez, key-value bazlı bir "Storage" (depolama) ihtiyacımız vardır.

`bud-vm` içinde, basit bir `HashMap` kullanmak yerine ZK'da kanıtlanabilir bir veri yapısı kullanmamız gerekir. Bu genellikle bir **Merkle Tree (Merkle Ağacı)** veya **Sparse Merkle Tree (SMT)** olur.

Eğer VM `SWrite` (Storage Write) komutunu işletirse, ağaçtaki bir yaprağın değeri güncellenir ve ağacın **Root (Kök)** değeri değişir. Prover, sadece en son Root değerini public input olarak paylaşarak, milyarlarca verilik bir veritabanının bütünlüğünü birkaç byte ile kanıtlamış olur.

Sanal makinemiz artık kodu çalıştırıp Execution Trace'i üretebiliyor. Ancak bu Trace'i ZK matematiğine (polinomlara) oturtmak hiç kolay değil. Bir sonraki bölümde bu mimari sorunu nasıl çözeceğimizi ve **ZK Dostu Mimariyi** inceleyeceğiz.
