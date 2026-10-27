//! §17.3 **kaynak bütçesi**: belge worker'ları için eşzamanlılık kapısı.
//!
//! Şartname başlangıç ayarı: *"aynı anda en fazla iki belge worker işi; büyük
//! dosyada bir iş"*. Bu modül o iki kuralı **tek yerde** tutar; ayrıştırma
//! yolu (`commands::run_parse`) izin almadan süreç başlatmaz.
//!
//! Tasarım kararları:
//!
//! - **Kasa başına tek yazıcı korunur.** Havuz yalnız *süreç* sayısını
//!   sınırlar; veritabanı yazımları kasa kilidinin arkasında seridir. Yani iki
//!   worker paralel çalışır, iki yazma asla aynı anda olmaz.
//! - **Büyük belge tek başına.** Eşiği aşan belge, çalışan iş kalmadığında
//!   kabul edilir ve o çalışırken yeni iş alınmaz (bellek tavanı).
//! - **Küçük belge büyük işi beklemez** (§17.3): büyük iş sırada beklerken
//!   küçük işler normal kapasiteyi kullanmaya devam eder. Sonsuz açlığı
//!   önlemek için büyük iş `LARGE_FAIRNESS_WAIT` süresini aştıysa yeni küçük iş
//!   kabul edilmez; çalışanlar bitince büyük iş girer.
//! - Sayaçlar yalnız **sayı** taşır (belge adı/yolu yok).

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Aynı anda en fazla iki belge worker işi (§17.3 başlangıç ayarı).
pub const MAX_CONCURRENT_PARSES: usize = 2;

/// "Büyük dosyada bir iş" eşiği: bu boyutun üstündeki belge tek başına koşar.
pub const LARGE_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;

/// Büyük iş bu süreden uzun beklediyse yeni küçük iş alınmaz (açlık önleyici).
pub const LARGE_FAIRNESS_WAIT: Duration = Duration::from_secs(2);

/// Bu süreden uzun bekleyen iş arayüze "sırada" olarak bildirilir.
pub const QUEUE_NOTICE_WAIT: Duration = Duration::from_millis(150);

/// İşin ağırlığı: belge boyutundan türetilir, çağıran karar vermez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseWeight {
    Normal,
    Large,
}

impl ParseWeight {
    /// Boyut eşiği aşarsa büyük iş. Sınır **dahil** (>= eşik).
    #[must_use]
    pub fn for_bytes(bytes: u64) -> Self {
        if bytes >= LARGE_DOCUMENT_BYTES {
            Self::Large
        } else {
            Self::Normal
        }
    }
}

#[derive(Debug, Default)]
struct PoolState {
    running: usize,
    exclusive: bool,
    waiting: usize,
    peak: usize,
    granted: u64,
    waited: u64,
    large_wait_since: Option<Instant>,
}

/// Eşzamanlılık kapısı. `AppState` içinde tek örnek tutulur.
#[derive(Debug)]
pub struct ParsePool {
    state: Mutex<PoolState>,
    ready: Condvar,
}

impl Default for ParsePool {
    fn default() -> Self {
        Self::new()
    }
}

impl ParsePool {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(PoolState {
                running: 0,
                exclusive: false,
                waiting: 0,
                peak: 0,
                granted: 0,
                waited: 0,
                large_wait_since: None,
            }),
            ready: Condvar::new(),
        }
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, PoolState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// `acquire` + kuyruk bildirimi: bekleme `QUEUE_NOTICE_WAIT`'i aştığında
    /// `notice` **bir kez** çağrılır (arayüz "sırada" gösterir).
    ///
    /// Dönüş: `Some(süre)` iş **gerçekten sıraya girdiyse** (kapı en az bir kez
    /// kapalıydı), `Some` olmayan `None` ise izin beklemeden verildiyse. Eski
    /// uygulama `Some`ı yalnız bildirim eşiği (150 ms) aşıldığında döndürüyordu;
    /// bu, "sıraya girdi mi" sorusunu zamanlamaya bağlı kılıyordu (ölçüldü:
    /// 100 ms bekleyen büyük iş `None` alıyordu).
    pub fn acquire_noticing(
        &self,
        weight: ParseWeight,
        notice: &mut dyn FnMut(),
    ) -> (PoolPermit<'_>, Option<Duration>) {
        let mut state = self.lock_state();
        state.waiting += 1;
        if weight == ParseWeight::Large && state.large_wait_since.is_none() {
            state.large_wait_since = Some(Instant::now());
        }
        let started = Instant::now();
        let mut noticed = false;
        let mut queued = false;
        loop {
            if self.admissible(&state, weight) {
                break;
            }
            queued = true;
            let (guard, _) = match self.ready.wait_timeout(state, Duration::from_millis(25)) {
                Ok(pair) => pair,
                Err(poisoned) => poisoned.into_inner(),
            };
            state = guard;
            if !noticed && started.elapsed() >= QUEUE_NOTICE_WAIT {
                noticed = true;
                notice();
            }
        }
        let waited = started.elapsed();
        state.waiting -= 1;
        state.running += 1;
        state.granted += 1;
        if weight == ParseWeight::Large {
            state.exclusive = true;
            state.large_wait_since = None;
        }
        state.peak = state.peak.max(state.running);
        if waited >= QUEUE_NOTICE_WAIT {
            state.waited += 1;
        }
        drop(state);
        let waited_out = if queued { Some(waited) } else { None };
        (PoolPermit { pool: self, weight }, waited_out)
    }

    fn admissible(&self, state: &PoolState, weight: ParseWeight) -> bool {
        match weight {
            // Büyük iş: çalışan kimse yokken girer, tek başına koşar.
            ParseWeight::Large => state.running == 0,
            // Küçük iş: büyük iş çalışmıyorsa ve kapasite varsa. Büyük iş
            // `LARGE_FAIRNESS_WAIT`'i aştıysa yeni küçük iş alınmaz.
            ParseWeight::Normal => {
                let fairness_blocked = state
                    .large_wait_since
                    .map(|since| since.elapsed() >= LARGE_FAIRNESS_WAIT)
                    .unwrap_or(false);
                !state.exclusive && !fairness_blocked && state.running < MAX_CONCURRENT_PARSES
            }
        }
    }
}

/// İzin tutamacı: düşünce kapasite açılır ve bekleyenler uyandırılır.
#[derive(Debug)]
pub struct PoolPermit<'a> {
    pool: &'a ParsePool,
    weight: ParseWeight,
}

impl Drop for PoolPermit<'_> {
    fn drop(&mut self) {
        let mut state = self.pool.lock_state();
        state.running = state.running.saturating_sub(1);
        if self.weight == ParseWeight::Large {
            state.exclusive = false;
        }
        drop(state);
        // Uyanan herkes kendi kapısını yeniden dener.
        self.pool.ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn quiet() -> impl FnMut() {
        || {}
    }

    #[test]
    fn weight_threshold_is_inclusive() {
        assert_eq!(ParseWeight::for_bytes(0), ParseWeight::Normal);
        assert_eq!(
            ParseWeight::for_bytes(LARGE_DOCUMENT_BYTES - 1),
            ParseWeight::Normal
        );
        assert_eq!(
            ParseWeight::for_bytes(LARGE_DOCUMENT_BYTES),
            ParseWeight::Large
        );
    }

    /// **§17.3 iki iş sınırı:** beş iş aynı anda istek verirse yalnız ikisi
    /// koşar; gözlenen eşzamanlılık tepe değeri 2'yi **aşmaz**.
    #[test]
    fn at_most_two_parses_run_at_once() {
        let pool = Arc::new(ParsePool::new());
        let live = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..5 {
            let pool = Arc::clone(&pool);
            let live = Arc::clone(&live);
            let peak = Arc::clone(&peak);
            handles.push(std::thread::spawn(move || {
                let (_permit, _waited) = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());
                let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(60));
                live.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for handle in handles {
            handle.join().expect("iş parçacığı");
        }
        let state = pool.lock_state();
        assert_eq!(peak.load(Ordering::SeqCst), MAX_CONCURRENT_PARSES);
        assert_eq!(state.peak, MAX_CONCURRENT_PARSES);
        assert_eq!(state.running, 0);
        assert_eq!(state.granted, 5);
        assert_eq!(state.waiting, 0);
    }

    /// **Büyük dosyada bir iş:** çalışan küçük iş varken büyük iş girmez;
    /// küçükler bitince **tek başına** koşar ve o sırada küçük iş alınmaz.
    #[test]
    fn large_document_runs_alone() {
        let pool = Arc::new(ParsePool::new());
        let (small_permit, _) = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());
        let pool_for_large = Arc::clone(&pool);
        let large_done = Arc::new(AtomicUsize::new(0));
        let done = Arc::clone(&large_done);
        let large = std::thread::spawn(move || {
            let (_permit, waited) =
                pool_for_large.acquire_noticing(ParseWeight::Large, &mut quiet());
            assert!(waited.is_some(), "büyük iş beklemiş olmalı");
            let state = pool_for_large.lock_state();
            assert_eq!(state.running, 1, "tek başına koşmalı");
            assert!(state.exclusive);
            done.fetch_add(1, Ordering::SeqCst);
        });
        // **Sabit uyku yerine kapı:** yüklü makinede iş parçacığı 100 ms içinde
        // başlamayabilir; sabit uyku testi zamanlamaya bağlı kılar (ölçüldü:
        // yüklü koşuda `waited` None dönüyordu). Büyük iş **beklemeye
        // girdiğinde** devam edilir.
        let deadline = Instant::now() + Duration::from_secs(5);
        while pool.lock_state().waiting == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(pool.lock_state().waiting, 1, "büyük iş beklemeye girmeli");
        assert_eq!(large_done.load(Ordering::SeqCst), 0);
        drop(small_permit);
        large.join().expect("büyük iş");
        assert_eq!(large_done.load(Ordering::SeqCst), 1);
        assert_eq!(pool.lock_state().peak, 1);
    }

    /// **Açlık önleyici:** büyük iş beklerken küçük işler normal kapasiteyi
    /// kullanır; ancak `LARGE_FAIRNESS_WAIT` aşıldıktan sonra yeni küçük iş
    /// alınmaz (çalışanlar bitince büyük iş girer).
    #[test]
    fn small_jobs_do_not_starve_a_waiting_large_job() {
        let pool = Arc::new(ParsePool::new());
        let first = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());
        let second = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());

        let pool_for_large = Arc::clone(&pool);
        let entered = Arc::new(AtomicUsize::new(0));
        let entered_large = Arc::clone(&entered);
        let large = std::thread::spawn(move || {
            let (_permit, _) = pool_for_large.acquire_noticing(ParseWeight::Large, &mut quiet());
            entered_large.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
        });

        // Büyük iş **sıraya girdi mi?** Sabit uyku yerine durum kapısı: yüklü
        // makinede iş parçacığı gecikebilir, o zaman "adalet penceresi" hiç
        // başlamamış olurdu.
        let deadline = Instant::now() + Duration::from_secs(5);
        while pool.lock_state().large_wait_since.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        let since = pool
            .lock_state()
            .large_wait_since
            .expect("büyük iş sıraya girmeli");

        // Pencere dolmadan küçük işler kapasiteyi kullanır: bir izin boşalınca
        // (running 2 → 1) yeni küçük iş **alınır**.
        drop(second);
        let (third, _) = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());
        assert_eq!(
            pool.lock_state().running,
            2,
            "pencere dolmadan küçük iş alınır"
        );
        drop(third);

        // Pencere dolduktan sonra: kapasite var (running=1) ama yeni küçük iş
        // alınmaz — büyük iş açlığa düşmez.
        let remain = LARGE_FAIRNESS_WAIT.saturating_sub(since.elapsed());
        std::thread::sleep(remain + Duration::from_millis(60));
        let acquired = Arc::new(AtomicUsize::new(0));
        let pool_for_small = Arc::clone(&pool);
        let acquired_in = Arc::clone(&acquired);
        let small = std::thread::spawn(move || {
            let (_permit, _) = pool_for_small.acquire_noticing(ParseWeight::Normal, &mut quiet());
            acquired_in.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(20));
        });
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            acquired.load(Ordering::SeqCst),
            0,
            "adalet penceresi dolduktan sonra yeni küçük iş alınmamalı"
        );

        // Son izin de bırakılınca büyük iş **tek başına** girer, sonra küçük iş.
        drop(first);
        let deadline = Instant::now() + Duration::from_secs(10);
        while entered.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(entered.load(Ordering::SeqCst), 1, "büyük iş girmiş olmalı");
        large.join().expect("büyük iş");
        small.join().expect("küçük iş");
        assert_eq!(
            acquired.load(Ordering::SeqCst),
            1,
            "büyük iş bitince bekleyen küçük iş girer"
        );
        assert_eq!(pool.lock_state().running, 0);
    }

    /// Kuyruk bildirimi eşiği aşınca **bir kez** çağrılır.
    #[test]
    fn queue_notice_fires_once_when_waiting_is_long() {
        let pool = Arc::new(ParsePool::new());
        let first = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());
        let second = pool.acquire_noticing(ParseWeight::Normal, &mut quiet());
        let notices = Arc::new(AtomicUsize::new(0));
        let notices_in = Arc::clone(&notices);
        let pool_for_wait = Arc::clone(&pool);
        let waiter = std::thread::spawn(move || {
            pool_for_wait.acquire_noticing(ParseWeight::Normal, &mut || {
                notices_in.fetch_add(1, Ordering::SeqCst);
            });
        });
        // Bildirim eşiği **geçildikten sonra** en çok bir kez: sabit uyku
        // yerine kapı (yüklü makinede iş parçacığı gecikebilir).
        let deadline = Instant::now() + Duration::from_secs(5);
        while notices.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            notices.load(Ordering::SeqCst),
            1,
            "bildirim bir kez gelmeli"
        );
        drop(second);
        drop(first);
        waiter.join().expect("bekleyen");
        assert_eq!(
            notices.load(Ordering::SeqCst),
            1,
            "izin verildikten sonra ikinci bildirim yok"
        );
    }

    /// Sayaçlar yalnız sayıdır; boş havuz tutarlı görünür.
    #[test]
    fn pool_starts_empty() {
        let pool = ParsePool::new();
        let state = pool.lock_state();
        assert_eq!(state.running, 0);
        assert_eq!(state.peak, 0);
        assert_eq!(state.granted, 0);
        assert!(state.large_wait_since.is_none());
    }
}
