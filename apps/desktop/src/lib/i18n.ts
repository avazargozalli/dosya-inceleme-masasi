/**
 * Merkezi Türkçe sözlük (F04-T08).
 *
 * **Kural:** arayüz metni bileşenin içinde gömülü string olarak yazılmaz; buradan
 * `t()` ile gelir. Sözlük `as const`'tur: yazım hatası olan anahtar **derleme
 * hatasıdır** (çalışma zamanında sessizce boş metne düşmez).
 *
 * Parametreli metinlerde eksik parametre sessizce `{ad}` bırakmaz — `t()` hata atar;
 * ekranda yarım cümle görünmesindense hata görünmesi yeğdir (uydurma metin yok).
 *
 * **Dürüst sınır:** F04-T08'in ilk yarısı (merkezi sözlük) bu turda *çekirdek +
 * kabuk + kasa ekranları* için tamamlandı; kalan ekranlar (Dosyalar, Arama,
 * İnceleme, Emsal, Maskeleme, Çıktılar, AI, Ayarlar, Kronoloji, Görevler, Kontrol
 * listesi) metinlerini hâlâ kendi dosyalarında taşır. Taşıma sırası ve sayısı
 * `docs/releases/F04/README.md` (F04-T08) içinde yazılıdır — burada "bitti"
 * denmez.
 */
export const tr = {
  common: {
    unknown: "—",
    back: "Geri",
    busy: "İşleniyor…",
    retryableSuffix: "yeniden denenebilir",
  },
  units: {
    byte: "bayt",
    kib: "KiB",
    mib: "MiB",
    second: "sn",
    millisecond: "ms",
  },
  dates: {
    missing: "tarih bilinmiyor",
    unreadable: "tarih okunamadı",
    unsupported: "tarih biçimi desteklenmiyor",
  },
  nav: {
    aria: "Ana gezinme",
    group: "Çalışma",
    vault: "Kasa",
    files: "Dosyalar",
    search: "Arama",
    timeline: "Kronoloji",
    tasks: "Görevler",
    checklists: "Kontrol listesi",
    review: "İnceleme",
    precedents: "Emsal",
    redaction: "Maskeleme",
    exports: "Çıktılar",
    ai: "AI bağlantıları",
    aiReview: "AI inceleme",
    settings: "Ayarlar",
  },
  shell: {
    mark: "Dİ",
    appName: "Dosya İnceleme",
    tagline: "yerel · şifreli",
    unlocked: "Kasa açık",
    locked: "Kasa kilitli",
    lock: "Kasayı kilitle",
    crumbRoot: "Kasa",
    version: "v{version}",
    comingLead: "Bu ekran {phase} fazında açılır. Şu an çalışıyormuş gibi gösterilmiyor.",
    comingEmpty: "Hazır değil — sahte veri veya pasif buton üretilmez (INV-09).",
  },
  steps: {
    aria: "Kurulum adımları",
    vault: "Kasa",
    recovery: "Kurtarma",
    start: "Kullanmaya başla",
  },
  unlock: {
    recoveryTitle: "Kurtarma anahtarını şimdi kaydet",
    recoveryLead:
      "Bu anahtar yalnızca bir kez gösterilir. Parolanı unutursan kasayı bununla açarsın. " +
      "Anahtarı kasa dosyasından ayrı bir yerde sakla; birlikte saklamak korumasını azaltır.",
    recoveryAria: "Kurtarma anahtarı",
    recoveryHint: "8 grup · 52 karakter · tireler yalnız okunabilirlik içindir.",
    savedCheck: "Anahtarı güvenli bir yere kaydettim.",
    openVault: "Kasayı aç",
    recoveryNotice:
      "Anahtarı kaybetmediğinden emin olmadan bu ekranı kapatma. Uygulama anahtarı tekrar göstermez.",
    title: "Kasa",
    lead:
      "Belgeler cihazında şifreli bir kasada durur. Kasa kapalıyken içerik okunamaz; arama " +
      "indeksi ve yedekler de aynı şifreleme kapsamındadır.",
    modesAria: "Kasa işlemleri",
    modeCreate: "Yeni kasa",
    modeOpen: "Mevcut kasayı aç",
    modeRecovery: "Kurtarma anahtarı",
    pathLabel: "Kasa konumu",
    pathHint:
      "Ev klasörüne göreli yol veya tam yol. Klasör yoksa oluşturulur; mevcut kasa dosyası " +
      "üzerine yazılmaz.",
    recoveryKeyLabel: "Kurtarma anahtarı",
    recoveryKeyHint: "Tireler ve boşluklar önemsiz; büyük/küçük harf fark etmez.",
    passwordCreate: "Parola oluştur",
    password: "Parola",
    passwordHint: "En az 12 karakter. Parola cihazda kalır; hiçbir sunucuya gönderilmez.",
    passwordConfirm: "Parola (tekrar)",
    submitCreate: "Kasayı oluştur",
    submitOpen: "Kilidi aç",
    submitRecovery: "Kurtarma ile aç",
    phaseTitle: "Bu fazda ne çalışıyor?",
    phaseVault: "Kasa oluşturma",
    phaseVaultValue: "Argon2id + AES-256-GCM",
    phaseUnlock: "Kilit açma",
    phaseUnlockValue: "dosyadan okuma, anahtar yalnız bellekte",
    phaseRecovery: "Kurtarma anahtarı",
    phaseRecoveryValue: "32 bayt, base32 gösterim",
    phaseImport: "Belge içe aktarma",
    phaseImportValue: "henüz yok",
    badgeReal: "gerçek",
    badgePhase: "F05",
  },
  workspace: {
    title: "Kasa açık",
    lead:
      "Şifreli kasa hazır. Bu fazda belge içe aktarma kapalı; F05 ile dosya ve sürüm yönetimi " +
      "açılacak. Aşağıdaki bilgiler gerçek kasa başlığından okunur.",
    cardTitle: "Kasa bilgisi",
    cardSub: "Başlık sürümü ve konum; içerik değil.",
    badgeOpen: "açık",
    idLabel: "Kasa kimliği",
    pathLabel: "Konum",
    encryptionLabel: "Şifreleme",
    encryptionValue: "SQLCipher hedefi · blob AEAD etkin · başlık v1",
    recoveryLabel: "Kurtarma",
    recoveryValue: "parola yuvası + kurtarma yuvası ayrı sarılmış",
    nextTitle: "Sıradaki",
    nextBody: "F03: SQLCipher şeması, migrasyonlar, kilit ve yedek. F04: dosya çalışma alanı.",
  },
  labels: {
    caseKind: {
      review: "İnceleme dosyası",
      reference_archive: "Kaynak arşivi",
    },
    sensitivity: {
      normal: "Normal",
      high: "Yüksek",
      client_confidential: "Müvekkil sırrı",
    },
    aiPolicy: {
      off: "AI kapalı",
      local_only: "Yalnız yerel model",
      cli_allowed: "CLI izinli",
      api_allowed: "API izinli",
    },
  },
} as const;

type Leaves<T> = {
  [K in keyof T & string]: T[K] extends string ? K : `${K}.${Leaves<T[K]>}`;
}[keyof T & string];

/** Sözlükteki bütün yaprak anahtarlar (`"unlock.title"` gibi). */
export type MessageKey = Leaves<typeof tr>;

/** Metinde `{ad}` yer tutucusu. */
const PLACEHOLDER = /\{([a-zA-Z0-9_]+)\}/g;

function lookup(key: string): string | undefined {
  let node: unknown = tr;
  for (const part of key.split(".")) {
    if (typeof node !== "object" || node === null) return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return typeof node === "string" ? node : undefined;
}

/** Sözlükteki bütün anahtarlar (test ve denetim için; UI'ya verilmez). */
export function messageKeys(): MessageKey[] {
  const out: MessageKey[] = [];
  const walk = (node: unknown, prefix: string): void => {
    if (typeof node === "string") {
      out.push(prefix as MessageKey);
      return;
    }
    if (typeof node !== "object" || node === null) return;
    for (const [k, v] of Object.entries(node as Record<string, unknown>)) {
      walk(v, prefix === "" ? k : `${prefix}.${k}`);
    }
  };
  walk(tr, "");
  return out;
}

/**
 * Sözlükten metin. `params` verilirse `{ad}` yer tutucuları doldurulur.
 * Bilinmeyen anahtar ya da eksik parametre **hata atar** — yarım cümle gösterilmez.
 */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const raw = lookup(key);
  if (raw === undefined) throw new Error(`i18n: bilinmeyen anahtar "${key}"`);

  if (params === undefined) {
    if (PLACEHOLDER.test(raw)) {
      PLACEHOLDER.lastIndex = 0;
      throw new Error(`i18n: "${key}" parametre ister`);
    }
    return raw;
  }

  const missing: string[] = [];
  const out = raw.replace(PLACEHOLDER, (_match, name: string) => {
    const value = params[name];
    if (value === undefined) {
      missing.push(name);
      return `{${name}}`;
    }
    return String(value);
  });
  PLACEHOLDER.lastIndex = 0;
  if (missing.length > 0) {
    throw new Error(`i18n: "${key}" için eksik parametre: ${missing.join(", ")}`);
  }
  return out;
}
