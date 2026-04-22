# Bölüm 3: Sanal Makine İnşası (bud-vm)

Komut setimizi (ISA) tanımladık. Şimdi bu komutları alıp gerçekten çalıştıracak olan "kalbi", yani Sanal Makineyi (VM) inşa edeceğiz. Bu modüle `bud-vm` adını verdik.

Sıradan bir yazılım geliştiricisi için VM yazmak karmaşık bir `switch-case` döngüsünden ibarettir. Ancak bir **ZKVM** yazdığınızı asla unutmamalısınız. VM'in her adımını öyle bir kaydetmeliyiz ki, daha sonra ZK Prover (Kanıtlayıcı) bu adımları alıp matematiksel denklemlere dökebilsin.

## VM'in Durumu (State)

Bir VM'in anlık halini (State) neler oluşturur?
1. **Program Counter (PC):** Şu an hangi komut satırını çalıştırıyoruz?
2. **Registers:** R0'dan R31'e kadar register'ların o anki değerleri.
3. **Memory/Storage:** Uygulamanın kalıcı veri depolama alanı.
4. **Execution Trace (Çalıştırma İzi):** Geçmişte yapılan tüm işlemlerin "log" kayıtları (ZKVM'ler için kritik!).

## Çalıştırma Döngüsü (Fetch-Decode-Execute)

Bir işlemcinin klasik döngüsüdür:

1. **Fetch (Getir):** `PC` değerinin gösterdiği adresten sıradaki komutu al.
2. **Decode (Çöz):** Komutun içindeki Opcode, src1, src2, dst ve imm değerlerini ayrıştır.
3. **Execute (Çalıştır):** Opcode'un gerektirdiği işlemi yap, sonucu `dst` register'ına yaz ve `PC`'yi bir sonraki komuta geçir.

`bud-vm/src/lib.rs` içindeki `step()` fonksiyonu tam olarak bunu yapar:

```rust
pub fn step(&mut self) -> Result<bool, VmError> {
    // 1. Fetch
    let instruction = self.program.get(self.pc).unwrap();
    
    // 2. Decode (Önceden ayrıştırılmış Instruction struct'ını kullanıyoruz)
    let src1_val = self.registers[instruction.src1 as usize];
    let src2_val = self.registers[instruction.src2 as usize];
    let mut dst_val = 0;
    let mut next_pc = self.pc + 1; // Default olarak bir sonraki satır

    // 3. Execute
    match instruction.opcode {
        Opcode::Add => dst_val = src1_val.wrapping_add(src2_val),
        Opcode::Load => dst_val = instruction.imm as u64,
        Opcode::Jmp => next_pc = (self.pc as i32 + instruction.imm) as usize,
        // Diğer opcode'lar...
        Opcode::Halt => return Ok(false), // Döngüyü kır
    }

    // Register'ı güncelle
    self.registers[instruction.dst as usize] = dst_val;

    // Execution Trace'i kaydet!
    self.trace.push(Step {
        pc: self.pc,
        instruction: instruction.clone(),
        src1_idx: instruction.src1,
        src2_idx: instruction.src2,
        dst_idx: instruction.dst,
        src1_val,
        src2_val,
        dst_val,
        next_pc,
    });

    self.pc = next_pc;
    Ok(true) // Çalışmaya devam et
}
```

## Neden Execution Trace (İz) Kaydediyoruz?

Klasik bir VM'de `step` işlemini yapıp eski state'i unuturuz. Fakat ZK dünyasında Prover, **her bir clock cycle'da (saat vuruşunda) ne olduğunu bilmek zorundadır.** Prover'ın işi, *"VM gerçekten bu adımları doğru hesapladı mı?"* sorusunu bir STARK devresi üzerinden kanıtlamaktır.

Bu yüzden VM çalışırken her bir `Step` objesini bir listeye ekleriz. Buna **Execution Trace** denir. Bu liste daha sonra ZK Prover'a gönderilecek ve satır satır, sütun sütun devasa bir matrise (matrix) dönüştürülecektir.

## Storage ve State Root

Gerçek dünya uygulamalarında (örneğin akıllı sözleşmelerde) sadece register'lar yetmez, key-value bazlı bir "Storage" (depolama) ihtiyacımız vardır.

`bud-vm` içinde, basit bir `HashMap` kullanmak yerine ZK'da kanıtlanabilir bir veri yapısı kullanmamız gerekir. Bu genellikle bir **Merkle Tree (Merkle Ağacı)** veya **Sparse Merkle Tree (SMT)** olur.

Eğer VM `SWrite` (Storage Write) komutunu işletirse, ağaçtaki bir yaprağın değeri güncellenir ve ağacın **Root (Kök)** değeri değişir. Prover, sadece en son Root değerini public input olarak paylaşarak, milyarlarca verilik bir veritabanının bütünlüğünü birkaç byte ile kanıtlamış olur.

Sanal makinemiz artık kodu çalıştırıp Execution Trace'i üretebiliyor. Ancak bu Trace'i ZK matematiğine (polinomlara) oturtmak hiç kolay değil. Bir sonraki bölümde bu mimari sorunu nasıl çözeceğimizi ve **ZK Dostu Mimariyi** inceleyeceğiz.
