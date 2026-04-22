# Bölüm 5: STARK, AIR ve Plonky3 (bud-proof)

Sıra geldi sihrin gerçeğe dönüştüğü yere. Elimizde VM'den aldığımız geniş ve detaylı bir "Execution Trace" (Çalıştırma İzi) var. Amacımız, bu matrisi alıp ZK-STARK kullanarak kriptografik olarak kanıtlamak. Bunun için Polygon'un geliştirdiği, sektör standardı haline gelmeye başlayan **Plonky3** kütüphanesini kullanıyoruz. Projemizdeki `bud-proof` modülü tamamen bu işe adanmıştır.

## Plonky3 Neden Önemli?

Eskiden (örneğin Winterfell kullanırken) kısıt dereceleri (constraint degrees), domain boyutları ve blowup faktörlerini manuel olarak çok hassas şekilde ayarlamak gerekiyordu. Plonky3, STARK kanıtlarının matematiğini daha modüler ve esnek bir mimariye oturtur. Özellikle **Goldilocks** cismi (field) gibi donanım dostu küçük asal sayıları yerel olarak çok iyi destekler. Bu sayede kanıt üretme süresi ciddi şekilde kısalır.

## AIR (Algebraic Intermediate Representation)

Bir ZKVM'in kalbi AIR'dır. AIR, Execution Trace'in doğruluğunu kontrol eden **matematiksel kurallar bütünüdür**. 
* Geleneksel programlamada doğruluğu `if (A + B == C)` ile kontrol ederiz.
* AIR dünyasında ise bu denklemi sıfıra eşitlemek zorundayız: `(A + B) - C = 0`

Eğer tüm satırlar için tüm denklemlerinizin sonucu sıfır çıkıyorsa, STARK kanıtı başarılı olur. Tek bir satırda, tek bir kısıtlama sıfırdan farklı bir sonuç verirse (örneğin VM yanlış bir matematik hesabı yapmışsa), sistem "Constraint failed" hatası verir ve kanıt üretilemez.

### Geçiş Kısıtlamaları (Transition Constraints)

`plonky3_air.rs` dosyasını incelerseniz, `BudAir` implementasyonunda `eval` fonksiyonunu görürsünüz. Bu fonksiyon, trace üzerinde "şu anki satır (`cur`)" ve "bir sonraki satır (`nxt`)" arasında kontroller yapar.

Örneğin PC (Program Counter) kuralını yazalım:
*"Eğer program bitmediyse, bir sonraki satırın PC'si, şu anki satırın next_pc'sine eşit olmalıdır."*

```rust
// is_not_halt = 1 - is_halt
builder.when_transition().assert_zero(is_not_halt.clone() * (nxt[COL_PC] - next_pc.clone()));
```
Bu denklemde, eğer `is_halt` sıfırsa `is_not_halt` 1 olur. Eğer `nxt[COL_PC]` ile `next_pc` farklıysa, sonuç sıfır olmaz ve kanıt patlar. Plonky3'teki `AirBuilder` bu matrisi bizim için polinomlara dönüştürür.

### Selector Sütunlarının Gücü

Daha önce Opcode'ların (0x01 = Add vb.) trace'e eklendiğini söylemiştik. Ancak polinom matematiğinde `if (opcode == 0x01)` yazamazsınız. Bunun yerine BudZKVM trace'ine **Selector Sütunları** eklenmiştir: `COL_IS_ADD`, `COL_IS_SUB`, `COL_IS_JMP` vb.

Eğer işlem bir Toplama (ADD) ise, trace oluşturulurken `COL_IS_ADD` sütununa `1` yazılır, diğerlerine `0` yazılır.
AIR içindeki kuralımız şöyle şekillenir:

```rust
builder.when(cur[COL_IS_ADD].clone()).assert_eq(rd_val_new.clone(), rs1_val.clone() + rs2_val.clone());
```
Bu sayede her bir matematiksel denklem, sadece kendi opcode'u aktif olduğunda çalışır.

## Register Tablosu Kısıtlamaları

Önceki bölümde bahsettiğimiz "Register Consistency" (Tutarlılık) kontrolünü Plonky3'te nasıl yazdığımıza bakalım:

*"Eğer bir sonraki satırda aynı register'da kalıyorsak (`r_same = 1`) VE bu bir okuma işlemiyse (`nr_write = 0`), register'ın içindeki değer DEĞİŞMEMELİDİR."*

Bunu polinom diliyle şu şekilde ifade ederiz:
```rust
builder.when_transition().assert_zero(
    r_active.clone() * nr_active.clone() * r_same.clone() * 
    (one.clone() - nr_write) * (nr_val - r_val)
);
```
İşte bir ZKVM'in hafıza bütünlüğünü koruyan, hacklenmesini ve dışarıdan veri sızdırılmasını engelleyen güvenlik duvarı tam olarak bu matematiksel formüllerdir.

Bir sonraki ve son bölümde, son kullanıcının tüm bunlarla uğraşmadan kod yazmasını sağlayan **Derleyici (Compiler) ve Komut Satırı (CLI)** araçlarımızı inceleyeceğiz.
