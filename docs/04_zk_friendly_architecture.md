# Bölüm 4: ZK Dostu Mimari (ZK-Friendly Architecture)

Sanal Makinemiz (VM) tıkır tıkır çalışıyor ve bize bir "Execution Trace" (Çalıştırma İzi) dizisi veriyor. Şimdi bu verileri alıp, bir STARK kanıtlayıcısına yedirebilmek için nasıl bir matrise dönüştüreceğimizi konuşacağız. İşin matematiği ve mühendisliği tam burada başlıyor.

## Execution Trace Bir Matristir

Bir ZK-STARK kanıtlayıcısı (Prover) kod okuyamaz. Yalnızca sayılarla dolu, iki boyutlu devasa bir matris anlar. Bu matrisin satırlarına (rows) **Step (Adım)**, sütunlarına (columns) ise **Register veya State** denir.

Matrisin boyutu (satır sayısı) kriptografik nedenlerden ötürü (FFT işlemleri) her zaman **2'nin kuvveti (Power of Two)** olmak zorundadır (16, 256, 1024, 65536 vs.).

### Geleneksel Tek Tablo Yaklaşımı (Neden Kötü?)

Başlangıçta BudZKVM'i yazarken her satıra tüm CPU durumunu koymayı denedik. 
- 1 sütun `PC`
- 1 sütun `Opcode`
- 32 sütun (Bütün genel amaçlı register'lar: R0, R1 ... R31)

Bu yaklaşım STARK kanıtını yazmak açısından çok basittir. Satır $i$ ile satır $i+1$'i kıyaslarsınız. Eğer Opcode `ADD R1, R2, R3` ise, R1'in güncellendiğini, **fakat diğer 31 register'ın aynen kaldığını** kontrol edersiniz.

**Sorun:** Prover açısından bu korkunç bir israftır. Çoğu işlemde (örneğin JMP) hiçbir register değişmez, ama siz yinede "R0 değişmedi, R1 değişmedi... R31 değişmedi" diye 32 tane ayrı kısıtlama denklemi (constraint) yazarsınız. Trace çok şişer, Prover yavaşlar ve bellek yetersizliğinden çöker.

### Çözüm: Çoklu Tablo (Multi-Table) ve Geniş İz (Wide Trace) Mimarisi

Bütün durumu tek bir tabloda tutmak yerine, işlemcinin mimarisini alt parçalara (Chiplets) böleriz. BudZKVM'de (Stage 2) bu mimariyi uyguladık:

1. **CPU Tablosu:** Sadece o anki komutun okuduğu ve yazdığı değerleri tutar.
2. **Register Tablosu:** Tüm register erişimlerinin kronolojik olarak değil, "Register Index"lerine göre sıralandığı ayrı bir alan.

Bu ikisini BudZKVM'de "Wide Trace" adı verilen tek bir matriste yan yana birleştirdik:

| CLK | PC | Opcode | ... CPU Columns ... | REG_CLK | REG_IDX | REG_VAL | REG_IS_WRITE |
|---|---|---|---|---|---|---|---|
| 0 | 0 | Load | ... | 0 | 0 | 0 | 1 |
| 1 | 1 | Add | ... | 1 | 0 | 5 | 0 |
| 2 | 2 | Sub | ... | 4 | 0 | 15| 1 |
| ... | ... | ... | ... | 2 | 1 | 10| 0 |

*(Dikkat ederseniz sağ taraftaki Register tablosu zamana (CLK) göre değil, Register Numarasına (REG_IDX) göre sıralanmıştır.)*

## Memory/Register Consistency (Tutarlılık Sorunu)

Eğer CPU tablosu ile Register tablosu ayrı mantıklara sahipse, CPU'nun $R1$'den okuduğu değerin, o anda $R1$'in **gerçekten sahip olduğu değer** olduğunu nasıl kanıtlarız? 

Bu STARK dünyasının en ünlü sorunlarından biridir ve çözümü **Permutation Argument (Permütasyon Argümanı)** veya **LogUp** adı verilen tekniklerdir.

Kısaca:
1. CPU, R1'den `5` okuduğunu iddia eder ve bunu bir "Veriyolu (Bus)" havuzuna atar.
2. Register tablosu, o anda R1'in içinde `5` olduğunu kontrol eder ve bu işlemi onaylayıp havuzdan çeker.
3. Günün sonunda havuzun toplamı/çarpımı sıfır (veya bir) çıkarsa, CPU ile Register tablosu "Tutarlı" demektir. Hiçbir değer yoktan var edilmemiş veya kaybolmamıştır.

## `COL_REG_SAME` ve Sub-Clock Ordering

BudZKVM'i geliştirirken karşılaştığımız en büyük engellerden biri "Read-after-Write (RaW)" sıralamasıydı. Eğer aynı clock cycle'da hem okuma hem yazma yapılıyorsa (Örn: `R1 = R1 + R2`), Register tablosunda okuma işleminin yazma işleminden **önce** gelmesini garanti etmeliyiz. Bunu çözmek için `sub_clk` adında yeni bir parametre ekledik ve sıralamayı `(idx, clk, sub_clk)` olarak güncelledik.

Ayrıca Register tablosunun bütünlüğünü sağlamak için **COL_REG_SAME** adında yardımcı bir boolean sütun oluşturduk.
* Eğer bir sonraki satır aynı register'ı gösteriyorsa `COL_REG_SAME = 1`
* Eğer bir sonraki satır yeni bir register'a (Örn: R1'den R2'ye) geçmişse `COL_REG_SAME = 0`

Bu basit hile, geçiş kısıtlamalarının (Transition Constraints) derecesini (degree) dramatik ölçüde düşürdü ve performanslı bir Prover elde etmemizi sağladı.

Mimarimizi masaya yatırdık. Peki bu tabloların "doğruluğunu" kontrol eden matematiksel formüller (Kısıtlamalar) koda nasıl dökülüyor? Bir sonraki bölümde **STARK ve Plonky3** ile bu denklemleri (AIR) yazacağız.
