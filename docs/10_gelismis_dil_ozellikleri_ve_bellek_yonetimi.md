# Bölüm 10: Gelişmiş Dil Özellikleri ve Bellek Yönetimi

Daha önceki bölümlerde sanal makinenin (VM) nasıl çalıştığını, ZK kanıtlarının (proof) matematiğini ve derleyicinin (compiler) kodları nasıl temel bytecodelara çevirdiğini incelemiştik. Ancak modern ve "gerçek dünya" (real-world) akıllı sözleşmeleri yazabilmek için basit toplama/çıkarma işlemleri ve düz bir çalıştırma akışı (flat execution) yeterli değildir.

Bir programlama dilini güçlü kılan üç temel özellik vardır:
1. Kodun tekrar kullanılabilmesi için **Fonksiyonlar**.
2. Hataları daha kodu derlerken (compile-time) yakalayabilmek için **Tip Sistemi (Type System)**.
3. Karmaşık veri modelleri oluşturabilmek için **Veri Yapıları (Structs)** ve bunların bellek (Memory) yönetimi.

Bu bölümde, BudZKVM derleyicisinin (bud-compiler) bu üç büyük özelliği ZK-STARK kısıtlamalarına ve sınırlı register mimarisine nasıl uyarladığını adım adım inceleyeceğiz. Sıfırdan bir ZK dili tasarlamanın en zorlu ve en keyifli kısımlarından birine hoş geldiniz!

---

## 1. Kullanıcı Tanımlı Fonksiyonlar ve Caller-Saved Registers

### Fonksiyon İhtiyacı
Büyük bir akıllı sözleşme yazarken kodu parçalara bölmemiz gerekir. Örneğin, hem imza doğrulayan hem de bakiye kontrol eden bir kodda, bu işlemleri ayrı fonksiyonlara (`verify_signature` ve `check_balance`) ayırmak kodun okunabilirliğini artırır.

BudL dilinde bir fonksiyon şu şekilde tanımlanır ve çağrılır:
```rust
fn add_and_mul(a: u64, b: u64, c: u64) -> u64 {
    let sum = a + b;
    return sum * c;
}

pub fn main() {
    let res = add_and_mul(1, 2, 42);
    emit Result(res);
}
```

### Call ve Ret Opcodeları
Sanal makinemizde fonksiyon çağrıları için iki özel komut bulunur: `Call` ve `Ret`.
* `Call`: Mevcut işlem satırını (Program Counter - PC) stack'e (yığına) kaydeder ve hedeflenen fonksiyonun satırına atlar.
* `Ret`: Stack'in en üstündeki adresi alır ve o adrese geri döner.

Ancak ortada büyük bir sorun vardır: **Register Sınırı.** BudVM'de sadece 32 adet register (kaydedici) bulunur (`R0` - `R31`). Eğer `main` fonksiyonu `R5` içine önemli bir değer kaydetmişse ve `add_and_mul` fonksiyonunu çağırırsa, `add_and_mul` fonksiyonu habersizce `R5`'i kendi işlemleri için kullanıp üzerine yazabilir! `main` fonksiyonuna geri dönüldüğünde `R5`'teki veri bozulmuş olacaktır.

### Caller-Saved Register Stratejisi
Bu sorunu çözmek için derleyici **Caller-Saved** (Çağıran Tarafından Korunan) yaklaşımını kullanır:
1. `main` fonksiyonu bir çağrı (Call) yapmadan hemen önce, o anda aktif olarak kullandığı tüm register'ları (örneğin `R1`, `R2`, `R3`) sırayla `Push` komutuyla Stack'e atarak güvene alır.
2. Fonksiyonun parametreleri (`1, 2, 42`) yeni tahsis edilen register'lara konur.
3. `Call` komutu çalıştırılır. Alt fonksiyon istediği register'ı özgürce kullanır.
4. Alt fonksiyon işini bitirince sonucu stack'e atar ve `Ret` yapar.
5. `main` fonksiyonu kaldığı yerden başlarken ilk iş olarak stack'teki eski register değerlerini `Pop` komutuyla geri yükler. (Restore işlemi).

Bu sayede sadece 32 register ile sonsuz derinlikte fonksiyon çağrıları güvenle yapılabilir.

---

## 2. Semantik Analiz ve Statik Tip Sistemi

Sadece değişkenleri tutan bir derleyici çok tehlikelidir. Eğer kullanıcı `let x = true; let y = x + 5;` yazarsa, makine seviyesinde bu `1 + 5 = 6` olarak çalışır ve mantıksal bir hataya (Bug) dönüşür. ZK akıllı sözleşmelerinde buglar milyonlarca dolara mal olabilir.

### Semantic Analyzer (Semantik Analizör) Nedir?
Derleme aşamasında kodun Parser (Sözdizimi) ağacını çıkarıldıktan sonra, makine koduna (Codegen) geçmeden önceki denetim aşamasıdır.

Semantik Analizör şu kuralları işletir:
* **Tip Uyuşmazlığı:** `u64` beklenen bir yere `bool` veya `field` atanıyor mu? 
* **Fonksiyon İmzaları:** `add_and_mul(1, 2, 42)` çağrısı tam olarak 3 adet `u64` parametre mi alıyor? Eğer parametre sayısı eksikse veya tipleri yanlışsa derleme anında durdurulur (`CompileError::TypeError`).
* **Return Tipleri:** Eğer bir fonksiyon `-> u64` döndüreceğini belirtmişse, içindeki `return` ifadesi gerçekten bir `u64` üretiyor mu?
* **Bilinmeyen Değişkenler:** Daha önce tanımlanmamış bir değişkene (`a = 5`) erişilmeye mi çalışılıyor?

Desteklenen temel tipler şunlardır:
- `u64`: 64-bit işaretsiz tamsayı.
- `bool`: Mantıksal doğru/yanlış.
- `field`: Goldilocks sonlu cisim elemanı ($p = 2^{64} - 2^{32} + 1$). ZK kanıtlarına özel kriptografik hesaplamalar için.
- `struct`: Kullanıcı tanımlı karmaşık veri tipleri.

---

## 3. Struct ve Dinamik Bellek (Heap Memory) Yönetimi

### Neden Belleğe (Memory) İhtiyacımız Var?
Register'lar çok hızlıdır ancak yapısal olarak birer sayıdırlar. Eğer kullanıcının şöyle bir verisi varsa:
```rust
struct User {
    id: u64,
    balance: u64,
    is_active: bool,
}
```
Bu `User` verisini register'larda taşımak çok zordur (3 farklı register gerekir ve fonksiyonlara parametre geçerken karmaşa yaratır). Bunun yerine bu veriyi **Bellekte (Memory)** bir blok halinde tutmalı ve değişkenlerde sadece bu bloğun "başlangıç adresini" (Pointer) taşımalıyız.

### r31'in HEAP_PTR Olarak Rezerve Edilmesi
BudVM'de kullanılmayan geniş bir bellek uzayı (Memory) mevcuttur. Derleyici, 31 numaralı register'ı (`r31`) özel bir amaç için feda eder: **Heap Pointer**.
Program başladığında `r31` belli bir bellek adresine (örneğin `4096`) ayarlanır. Burası boş bellek havuzunun başlangıcıdır.

### Struct Literal (Obje Yaratma)
Kullanıcı `let u = User { id: 1, balance: 100, is_active: true };` dediğinde derleyici arka planda şunları yapar:
1. Yeni bir pointer register'ı ayırır ve `r31`'in değerini buraya kopyalar (örn. adres 4096).
2. `id` değeri olan `1`'i belleğin `4096` adresine yazar (`Opcode::Store`).
3. `balance` değeri olan `100`'ü belleğin `4096 + 8 = 4104` adresine yazar.
4. `is_active` değeri olan `true (1)`'i belleğin `4104 + 8 = 4112` adresine yazar.
5. `r31` (Heap Pointer) register'ını yeni objenin boyutu kadar (3 field * 8 byte = 24) artırıp `4120`'ye eşitler.
6. `u` değişkenine sadece objenin başlangıç adresi olan `4096` değeri atanır.

### Field Access (Özellik Okuma)
Kullanıcı kodun ilerleyen satırlarında `let b = u.balance;` yazdığında:
1. Semantik Analizör `u`'nun bir `User` struct'ı olduğunu, `balance` field'ının ise bu struct'ın 2. sıradaki elemanı (yani 8. byte'taki offseti) olduğunu bilir.
2. Derleyici `Opcode::Load` komutunu üretir. Bu komut, `u`'nun tuttuğu adrese (4096) gider, üzerine offset'i (8) ekler (4104 adresi) ve oradaki `100` değerini okuyup `b` değişkenine atar.

Bu "Pass-by-Reference" (Referansla Taşıma) yaklaşımı sayesinde:
- Struct'lar fonksiyonlara argüman olarak geçirildiğinde koca bir veri kümesi kopyalanmaz, sadece tek bir pointer adresi aktarılır.
- ZKVM'in yürütme adımı (Execution trace) inanılmaz derecede küçülür ve Prover'ın kanıt üretme süresi ciddi şekilde hızlanır.

İşte tüm bu sistemlerin birleşimi, BudZKVM'i sadece basit matematik hesapları yapan bir oyuncak değil, modern diller (Rust, Solidity) seviyesinde bir ZK Akıllı Sözleşme (Smart Contract) dili haline getirmektedir.
