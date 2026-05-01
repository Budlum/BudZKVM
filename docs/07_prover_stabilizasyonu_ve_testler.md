# Bölüm 7: Prover Stabilizasyonu ve Testler

Bu bölüm genel bir "sıfırdan ZKVM yazalım" metni değildir. BudZKVM reposundaki gerçek prover kodunun Plonky3 0.5.2 ile uyumlu, test edilebilir ve adım adım geliştirilebilir hale getirilmesini anlatır. Amaç, okuyucunun `bud-proof` modülüne baktığında hangi dosyanın hangi matematiksel sorumluluğu taşıdığını görmesi ve yeni bir hata çıktığında nereden başlaması gerektiğini bilmesidir.

Bu bölümde üç şeyi birlikte tutacağız:

1. Plonky3 0.5.2 tip sistemi ve konfigürasyon sınırları.
2. İki fazlı trace yapısı: ana trace ve yardımcı trace.
3. Prover adapter, serde ve test stratejisi.

## Neden Stabilizasyon Aşaması Var?

Bir ZKVM'de VM'in çalışması tek başına yeterli değildir. VM doğru sonucu üretse bile prover şunları ayrıca kanıtlamalıdır:

* Her satır geçerli bir opcode çalıştırıyor.
* Program counter doğru ilerliyor.
* Register değerleri tutarlı kalıyor.
* Okuma ve yazma olayları aynı mantıksal belleğe bağlı.
* Halt durumundan sonra trace yanlış şekilde devam etmiyor.
* Kanıt byte dizisine çevrilip geri okunabiliyor.

BudZKVM'de bu sorumlulukların büyük bölümü `bud-proof` crate'i içindedir. Stabilizasyon aşaması, bu crate'in Plonky3'ün güncel API'siyle konuşmasını ve ileride gerçek lookup/permutation kuralları eklendiğinde kırılmayacak bir iskelet kurmasını sağlar.

## Dosya Haritası

Prover tarafını okurken şu dosyaları birlikte düşünmek gerekir:

* `bud-proof/src/plonky3_air.rs`: VM trace'i üstündeki opcode, PC, register ve halt kısıtlarının yazıldığı ana AIR.
* `bud-proof/src/plonky3_prover.rs`: BudZKVM dünyasındaki `ProofSystem` trait'ini Plonky3 tabanlı prover'a bağlayan adapter.
* `bud-proof/src/bud_stark/config.rs`: Plonky3 PCS, challenger, domain ve field tiplerinin merkezi olarak tanımlandığı yer.
* `bud-proof/src/bud_stark/proof.rs`: Commitments, opened values ve proof nesnesinin taşınabilir yapısı.
* `bud-proof/src/bud_stark/prover.rs`: Main trace, auxiliary trace, challenge üretimi, commitment ve opening akışının kurulduğu çekirdek prover.
* `bud-proof/src/bud_stark/verifier.rs`: Proof içeriğinin aynı transcript akışıyla doğrulandığı taraf.
* `bud-proof/src/bud_stark/folder.rs`: AIR kısıtlarının prover ve verifier tarafında aynı mantıkla katlandığı constraint folder.
* `bud-proof/src/bud_stark/sub_builder.rs`: AIR'in alt pencereler üzerinde çalışmasını sağlayan yardımcı builder.

Bu ayrım önemlidir. `plonky3_air.rs` VM'in neyi kanıtladığını söyler. `bud_stark` altındaki dosyalar ise bu kuralların STARK kanıtına nasıl dönüştürüldüğünü yönetir.

## Faz 1: Tip Sistemi ve Konfigürasyon

Plonky3 0.5.2 ile çalışırken en kritik konu, generic tiplerin tek bir yerde ve tutarlı biçimde tanımlanmasıdır. Eğer `Val<SC>`, `Challenge`, `Domain`, `Pcs` ve `Challenger` sınırları farklı dosyalarda farklı şekillerde kurulursa Rust derleyicisi çok uzun type inference hataları üretir.

BudZKVM'de bu yüzden `bud_stark/config.rs` merkezi dosya haline getirilir. Buradaki hedef şudur:

* Ana field tipi `SC::Val` üzerinden okunur.
* Packed değerler recursive type alias döngüsüne girmeden tanımlanır.
* PCS proof ve commitment tipleri ortak alias'larla taşınır.
* Challenger ve domain tipleri prover ile verifier arasında birebir aynı kalır.

Bu fazın çıktısı şudur: Prover ve verifier ayrı ayrı "ben hangi PCS'i kullanıyorum?" sorusuna cevap vermek zorunda kalmaz. İkisi de aynı `StarkGenericConfig` üzerinden konuşur.

## Proof ve Serde Sınırları

`Proof<SC>` yapısı sadece basit alanlardan oluşmaz. İçinde commitment'lar, opened values ve PCS proof bulunur. Bunların her biri `SC` generic parametresine bağlıdır. Derleyicinin otomatik serde sınırlarını çıkarması bu yüzden zorlaşır.

BudZKVM'de çözüm, serde sınırlarını açık yazmaktır. Proof şu mantıkla ele alınır:

* Commitment tipi serialize edilebiliyorsa proof serialize edilebilir.
* Challenge tipi serialize edilebiliyorsa opened values serialize edilebilir.
* PCS proof tipi serialize edilebiliyorsa tüm kanıt byte dizisine çevrilebilir.

Bu yaklaşım `bincode` desteğini geri getirir ve CLI/L1 entegrasyonu için gereken proof taşıma katmanını sadeleştirir. Proof formatının daha sonra kalıcı bir wire format haline gelmesi istenirse bu dosya doğal sınır noktasıdır.

## Faz 2: Ana Trace ve Yardımcı Trace

BudZKVM prover mimarisi iki fazlıdır:

1. Main trace commit edilir.
2. Transcript üzerinden challenge üretilir.
3. Challenge kullanılarak auxiliary trace üretilir.
4. Main ve auxiliary opening'ler birlikte doğrulanır.

Bu yapı cross-table lookup ve permutation kuralları için gereklidir. Örneğin CPU tablosundaki bir register okuması, register event tablosundaki önceki yazma ile bağlanmak istediğinde sadece ana trace yeterli olmaz. Lookup accumulator değerleri challenge'a bağlı olarak auxiliary trace içinde taşınır.

Şu anki stabilizasyon aşamasında auxiliary trace iskeleti hazır tutulur. Adapter tarafında `generate_aux_trace` benzeri kapatma fonksiyonu Fiat-Shamir randomness değerlerini alır ve yardımcı matrisi üretir. Gerçek register/memory permutation mantığı eklendikçe bu fonksiyon all-one placeholder yerine accumulator sütunları hesaplayacaktır.

## Constraint Folder Ne İşe Yarar?

AIR kısıtları iki farklı bağlamda çalıştırılır:

* Prover tarafında trace satırları packed field elemanlarıdır.
* Verifier tarafında opening değerleri challenge field elemanlarıdır.

`folder.rs` bu iki dünyayı aynı AIR koduna bağlar. `PermutationAirBuilder` implementasyonu özellikle önemlidir çünkü auxiliary trace penceresi burada AIR'e sunulur. Eğer `permutation()` boş pencere döndürürse lookup kısıtları yazılsa bile gerçek yardımcı sütunlara bağlanamaz.

Bu yüzden `AuxWindow` şu iki pencereyi taşımalıdır:

* `current_slice`: mevcut satırdaki auxiliary değerler.
* `next_slice`: bir sonraki satırdaki auxiliary değerler.

Prover tarafında bu değerler packed base trace satırlarından challenge elemanlarına paketlenir. Verifier tarafında ise opening değerleri base coefficient parçalarından yeniden challenge elemanına toparlanır.

## Sub Builder ve Pencere API'si

`sub_builder.rs`, AIR'in trace'in belirli bir aralığı üzerinde çalışmasını sağlar. Bu mekanizma register tablosu, CPU tablosu veya ileride eklenecek alt tablolar için gereklidir. Yeni WindowAccess API'sinde doğrudan `current_slice()` ve `next_slice()` kullanmak daha net bir model verir.

Sub builder'ın dikkat etmesi gereken şey şudur: Ana builder hangi bağlamı destekliyorsa sub builder da onu doğru şekilde ileri taşımalıdır. Yani sadece `AirBuilder` değil, ihtiyaç oldukça şu yetenekler de forward edilmelidir:

* `AirBuilderWithContext`
* `PeriodicAirBuilder`
* `ExtensionBuilder`
* `PermutationAirBuilder`

Bu forwarding eksik olduğunda hata genellikle AIR dosyasında görünür, ama kök sebep builder trait zincirindedir.

## Adapter Akışı

`plonky3_prover.rs` BudZKVM'nin dış dünyaya gösterdiği kanıt API'sidir. Burada yapılması gereken iş Plonky3 detaylarını VM'den ayırmaktır.

Kanıt üretimi şu sırayla akar:

1. VM programı çalıştırır ve `Step` trace'i üretir.
2. Adapter trace satırlarını `RowMajorMatrix<Goldilocks>` formatına çevirir.
3. `BudAir` program ve başlangıç register durumuyla kurulur.
4. `StarkConfig` oluşturulur.
5. `prove` çağrısı main trace, auxiliary generator ve public input ile çalıştırılır.
6. Dönen proof `bincode` ile byte dizisine çevrilir.

Doğrulama tarafında akış tersine döner:

1. Proof byte dizisi deserialize edilir.
2. Aynı AIR ve config tekrar kurulur.
3. `verify` çağrısı proof ve public input üzerinde çalışır.
4. Hata varsa `false`, başarı varsa `true` döner.

Bu adapter'ın amacı CLI, L1 entegrasyonu veya testlerin Plonky3 iç tiplerini bilmesini engellemektir.

## Test Stratejisi

Stabilizasyon testleri iki sınıfta düşünülmelidir.

İlk sınıf VM davranışını prover ile birlikte test eder:

* Basit `ADD + HALT` programı kanıtlanmalı ve doğrulanmalıdır.
* `ADD`, `SUB`, `MUL` gibi aritmetik opcode'lar aynı trace içinde çalışmalıdır.
* Immediate yükleme akışı kanıtlanmalıdır.
* Halt sonrası trace mantığı bozulmamalıdır.

İkinci sınıf proof taşıma katmanını test eder:

* Üretilen proof byte dizisi deserialize edilip tekrar doğrulanmalıdır.
* Rastgele veya bozuk byte dizisi doğrulamadan geçmemelidir.
* Serde sınırları değiştiğinde testler proof formatındaki kırılmayı yakalamalıdır.

Bu testler matematiksel güvenliği tek başına kanıtlamaz, ama prover entegrasyonundaki kırılmaları erken yakalar. Özellikle Plonky3 güncellemesi yaparken önce bu testlerin yeşil kalması gerekir.

## Bugün Stabil Olan Parçalar

Mevcut stabilizasyon çalışmasıyla hedeflenen taban şudur:

* `cargo check` prover crate'i dahil temiz çalışır.
* `bud-proof` testleri Goldilocks field üzerinde proof üretip doğrular.
* Proof byte dizisi `bincode` üzerinden taşınabilir.
* Main trace ve auxiliary trace için folder iskeleti aynı AIR'e bağlanır.
* Sub builder yeni pencere API'siyle uyumludur.

Bu noktadan sonra yapılacak işler daha hedeflidir. Artık derleyici hatalarının çoğu tip sistemi gürültüsü olmaktan çıkar, gerçek AIR veya lookup mantığına işaret eder.

## Sıradaki Sertleştirme Adımları

Stabil tabanın üstüne şu işler eklenmelidir:

* Auxiliary trace placeholder'ı gerçek register permutation accumulator'larıyla değiştirmek.
* CPU, register ve memory tabloları arasında cross-table lookup kurallarını tamamlamak.
* Public input bağlamını netleştirmek: program hash, başlangıç state'i ve final state proof'a bağlanmalı.
* `COL_IS_HALT` kısıtını ayrıca denetlemek: halt sonrası satırlar program sonunu zayıflatmamalı.
* Bitwise opcode'ların field içinde boolean decomposition kurallarını tamamlamak.
* Proof formatını uzun vadeli uyumluluk için versiyonlamak.

Bu işler bittikçe Bölüm 5'teki AIR açıklaması daha matematiksel, bu bölümdeki stabilizasyon notları ise daha operasyonel kalmalıdır. Böylece kitap hem ZKVM mimarisini öğrenen okuyucuya hem de BudZKVM kodunu geliştiren kişiye aynı anda hizmet eder.
