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
 * **Dürüst sınır:** F04-T08 sürüyor. Taşınan dosyalar: `App.tsx`,
 * `screens/Unlock.tsx`, `screens/Workspace.tsx`, `screens/Tasks.tsx`,
 * `screens/Exports.tsx`, `screens/Redaction.tsx`, `screens/Timeline.tsx`,
 * `screens/Checklists.tsx` ve `lib/checklist.ts`. Kalan ekranlar (Dosyalar/Cases,
 * Arama, İnceleme, Emsal, AI, Sağlayıcılar, Ayarlar) metinlerini hâlâ kendi
 * dosyalarında taşır; `lib/jobs.ts`, `lib/reader.ts`, `lib/providers.ts`,
 * `lib/search.ts`, `lib/reminders.ts`, `lib/export-coverage.ts` ve diğer yardımcı
 * lib'ler de kendi metinlerini taşır. Taşıma sırası ve sayısı
 * `docs/releases/F04/README.md` (F04-T08) içinde yazılıdır — burada "bitti"
 * denmez.
 */
export const tr = {
  common: {
    unknown: "—",
    back: "Geri",
    busy: "İşleniyor…",
    retryableSuffix: "yeniden denenebilir",
    case: "Dosya",
    noCase: "Dosya yok",
    none: "yok",
    save: "Kaydet",
    select: "Seçin…",
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
    lockedHint:
      "Kasa kilitli. Bu ekrandaki içerik gösterilmez ve bellekte tutulmaz.",
    goVault: "Kasa ekranına git",
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
  tasks: {
    lead: "Tarihi kullanıcı girer ve doğrular. Uygulama hukuki son gün hesaplamaz; tarih kaynağı “kullanıcı girdisi” olarak işaretlenir.",
    taskLabel: "Görev",
    titlePlaceholder: "Örn. Tebliğ tarihini kontrol et",
    dueLabel: "Tarih (isteğe bağlı)",
    reminderLabel: "Hatırlatma",
    add: "Ekle",
    titleRequired: "Görev başlığı zorunlu.",
    listTitle: "Görev listesi",
    recordCount: "{count} kayıt",
    includeDone: "Tamamlananlar",
    empty: "Görev yok.",
    noDate: "tarih yok",
    confirmed: "onaylı",
    pendingConfirm: "onay bekliyor",
    done: "tamamlandı",
    open: "açık",
    confirmDate: "Tarihi onayla",
    complete: "Bitti",
    remove: "Sil",
    reminderHint:
      "Hatırlatıcı yalnız kullanıcının açıkça onayladığı tarih için kurulur; “Tarihi onayla” denmedikçe bildirim üretilmez.",
    remindersTitle: "Yerel hatırlatıcılar",
    remindersSub: "{count} hatırlatıcı · {day}",
    enqueue: "Bildirim kuyruğuna yaz",
    noReminders:
      "Bugün için hatırlatıcı yok. Göreve tarih ve hatırlatma seçilip tarih onaylanmalı.",
    noNewReminders: "Yeni hatırlatıcı yok (bugün için zaten kuyrukta).",
    remindersQueued: "{queued} hatırlatıcı yerel bildirim kuyruğuna yazıldı.",
    taskFallback: "görev",
    privacyHint:
      "Bildirim metni müvekkil adı, dosya adı veya görev başlığı taşımaz; ayrıntı yalnız kasa açıkken uygulamada görünür.",
  },
  exports: {
    lead: "Her dışa aktarma kasa içinde kayda alınır: tür, hedef ve çıktı hash'i. Belge baytları yalnız açıkça seçilen formatta ve yalnız kullanıcı seçtiği yola yazılır.",
    formatJson: "Tam metadata: dosya, belgeler, olaylar, iddialar, notlar, görevler, anchor'lar",
    formatCsv: "Kronoloji ve görevler; tablo programlarında açılır",
    formatDocx: "Duruşma/devir raporu; düzenlenebilir belge",
    formatPdf: "Aynı raporun salt-okunur çıktısı",
    saveDialog: "Çıktıyı kaydet",
    coverageTitle: "İncelenen kapsam",
    coverageSub:
      "Sayılar kasadan okunur ve her çıktının içine gömülür. Kapsam dışı belge/sayfa “temiz” sayılmaz.",
    refresh: "Kapsamı tazele",
    coverageWarnings: "Kapsam uyarıları",
    coverageUnreadable: "Kapsam okunamadı; dışa aktarma yine kapsamı yazar.",
    writing: "Yazılıyor…",
    written: "yazıldı:",
    packageTitle: "Devir paketi",
    packageSub:
      "case.json + KAPSAM.md + manifest + README; belge baytları otomatik eklenmez",
    packageBusy: "Oluşturuluyor…",
    packageCreate: "Paket oluştur",
    packagePassword: "Paket parolası (en az 10 karakter, isteğe bağlı)",
    packagePasswordPlaceholder: "Boş bırakılırsa klasör açık metin kalır",
    packageHint:
      "Parola girilirse klasör Argon2id + AES-256-GCM ile şifrelenip tek `.dimpak` dosyasına dönüşür. Paylaşılacak belgeler için önce Maskeleme ekranından maskeli kopya üretin.",
    packageReady: "Paket hazır: {path}",
    packageFiles: "{files} dosya · manifest {hash}…",
    sealed: "Şifreli paket:",
    dirDialog: "Devir paketi klasörünü seç",
  },
  redaction: {
    lead: "Terimler belgenin metin katmanından ve görüntüsünden gerçekten çıkarılır; siyah kutu çizmek yeterli sayılmaz. Çıktı yeniden açılıp doğrulanmadan kaydedilmez.",
    modeTerms: "Terimle maskele",
    modeRegions: "Bölge seçerek maskele",
    docLabel: "Belge",
    termsLabel: "Maskelenecek terimler (her satır veya virgül ayrı)",
    termsHint: "Terimler birebir aranır; eşleşme yoksa o terim için bölge oluşmaz.",
    pageLabel: "Sayfa (0'dan)",
    loadPage: "Sayfayı getir",
    clearRegions: "Seçimleri temizle ({count})",
    previewAlt: "Sayfa {page} önizleme",
    previewHint:
      "Sayfayı getirin, fareyle dikdörtgen çizin. Seçimler punto koordinatına çevrilir.",
    runTerms: "Maskeli kopya üret",
    runRegions: "Seçili {count} bölgeyi maskele",
    masking: "Maskeleniyor…",
    preparing: "Maskeleme hazırlanıyor…",
    busyHint:
      "Maskeleme uzun belgede sürebilir; pencere donmaz. İş kuyruğundan iptal edilirse yarım çıktı kasaya yazılmaz.",
    regionsRequired: "En az bir bölge seçin.",
    termsRequired: "Belge ve en az bir terim gerekli.",
    resultTitle: "Maskeli kopya hazır",
    resultSub: "Yeni sürüm olarak kaydedildi; özgün sürüm değişmedi.",
    verified: "doğrulandı",
    newDocument: "Yeni belge",
    version: "Sürüm",
    maskedRegions: "Maskelenen bölge",
    remaining: "Kalan terim",
    engines: "Doğrulayan motorlar",
    streams: "Bağımsız tarama",
    streamsLine: "{scanned} akış açıldı · {binary} ikili akış",
    unreadableStreams: "{count} akış açılamadı",
    hiddenObjects: "Gizli nesne temizliği",
    hiddenObjectsLine: "{annotations} açıklama · {offpage} sayfa dışı metin · {scripts} JavaScript",
    ocrTitle: "Görüntü (OCR) doğrulaması",
    ocrYes: "evet",
    ocrNo: "hayır — tesseract yok, yalnız metin katmanı doğrulandı",
    warnings: "Doğrulama uyarıları",
    plan: "Plan",
    shareHint: "Bu kopya paylaşım içindir. Özgün belge ve eski sürümler kasadan silinmedi.",
  },
  timeline: {
    lead:
      "Olay, belge ve tebliğ tarihleri ayrı türlerdir. Ay/yıl veya bilinmeyen tarih desteklenir; " +
      "eksik gün uydurulmaz. Tarihi bilinmeyen olaylar ayrı grupta durur.",
    titleRequired: "Olay başlığı zorunlu.",
    confirmed: "onaylandı",
    draft: "taslak",
    rawNote: "ham: {raw}",
    confirm: "Onayla",
    edit: "Düzenle",
    closeEdit: "Düzenlemeyi kapat",
    sources: "Kaynaklar",
    closeSources: "Kaynakları kapat",
    remove: "Sil",
    fieldTitle: "Başlık",
    fieldDate: "Tarih",
    fieldPrecision: "Kesinlik",
    fieldRole: "Tarih türü",
    fieldDescription: "Açıklama",
    cancel: "Vazgeç",
    versionHint:
      "Kayıt sürümü {version}. Kayıt başka bir pencerede değiştiyse kaydetme reddedilir, " +
      "güncel sürüm yüklenir; eski veri sessizce ezilmez.",
    noSources: "Bu olaya bağlı kaynak yok.",
    removeLink: "Kaldır",
    sourceQuote: "Kaynak alıntı",
    noAnchors: "Anchor yok — okuyucuda alıntı oluşturun",
    relation: "İlişki",
    attach: "Bağla",
    eventTitle: "Olay başlığı",
    eventTitlePlaceholder: "Örn. Kira ödemesi yapılmadı",
    addEvent: "Olay ekle",
    userAssertionHint:
      "Bu kayıtlar kullanıcı beyanıdır; otomatik hukuki süre hesabı yapılmaz (ADR-0008).",
    knownTitle: "Tarihli olaylar",
    knownSub: "{count} olay · eskiden yeniye",
    knownEmpty: "Henüz tarihli olay yok.",
    unknownTitle: "Tarihi bilinmeyenler",
    unknownSub: "{count} olay · sıralamaya sokulmaz",
    role: {
      event: "Olay tarihi",
      document: "Belge tarihi",
      notification: "Tebliğ tarihi",
      contract: "Sözleşme tarihi",
      hearing: "Duruşma",
      deadline_candidate: "Süre adayı",
      unknown: "Rol bilinmiyor",
    },
    precision: {
      day: "Gün",
      month: "Ay",
      year: "Yıl",
      range: "Aralık",
      approximate: "Yaklaşık",
      unknown: "Bilinmiyor",
    },
    relationKind: {
      states: "belirtir",
      supports: "destekler",
      contradicts: "çelişir",
      context: "bağlam",
    },
  },
  checklists: {
    lead:
      "Lisanslı şablonlar sürüm, yazar ve içerik hash'iyle dosyaya kopyalanır. Öğe durumu " +
      "belgeye veya gerekçeye bağlanır; “bulunamadı” yalnız incelenen kapsamı anlatır.",
    reload: "Güncel kaydı yükle",
    state: "Durum",
    summary:
      "{total} öğe · {open} açık · {present} bulundu · {missing} incelenen kapsamda bulunamadı · " +
      "{notApplicable} uygulanamaz",
    catalogTitle: "Şablon kataloğu",
    catalogSub: "İçerik lisansı yazılım lisansından ayrıdır; kopyada sürüm ve kaynak korunur.",
    catalogEmpty: "Katalog boş.",
    practiceUnknown: "alan belirtilmemiş",
    templateItems: "{count} madde · içerik",
    sourceLabel: "kaynak:",
    verifyCopy: "Kopyayı doğrula",
    copyToCase: "Dosyaya kopyala",
    copyHint:
      "Aynı şablon sürümü ikinci kez kopyalanmaz; “Kopyayı doğrula” yinelenen öğe üretmeden " +
      "mevcut kopyayı denetler.",
    caseListTitle: "Dosya kontrol listesi",
    itemsSub: "{count} öğe",
    foundBadge: "{count} bulundu",
    missingBadge: "{count} bulunamadı",
    notApplicableBadge: "{count} uygulanamaz",
    empty: "Bu dosyada kontrol listesi yok. Katalogdan bir şablon kopyalayın.",
    itemMeta: "{ordinal}. madde · {category}",
    categoryGeneral: "genel",
    licenseUnknown: "lisans belirtilmemiş",
    templateMeta: "{title} sürüm {version} · {license} · içerik",
    noTemplateMeta: "şablon künyesi yok (kopya metni korunuyor)",
    linkedDocuments: "Bağlı belgeler ({count})",
    noDocuments: "Bu dosyada belge yok; önce belge içe aktarın.",
    noteMissing: "İncelenen kapsam",
    noteNotApplicable: "Gerekçe",
    noteOptional: "Not (isteğe bağlı)",
    notePlaceholderMissing: "Örn. 2020-2024 arası kira sözleşmeleri incelendi",
    notePlaceholder: "Kısa açıklama",
    removeLink: "kaldır",
    staleLinks: "Listede görünmeyen bağlar (belge silinmiş veya arşivlenmiş olabilir):",
    savedNote: "Kayıtlı not: {note}",
    missingScopeNote:
      "“İncelenen kapsamda bulunamadı” yalnız bakılan aralığı anlatır; dosyanın tamamında " +
      "belge yok anlamına gelmez.",
    stateLabel: {
      open: "Açık",
      present: "Bulundu",
      missing: "İncelenen kapsamda bulunamadı",
      not_applicable: "Uygulanamaz",
    },
    stateHelp: {
      open: "Henüz değerlendirilmedi.",
      present: "En az bir belge bağı gerekir.",
      missing: "İncelenen kapsam yazılmalı; “dosyada yok” iddiası üretilmez.",
      not_applicable: "Gerekçe yazılmalı.",
    },
    rule: {
      presentNeedsDocument: "“Bulundu” için en az bir belge seçin.",
      linksOnlyPresent: "Belge bağı yalnız “Bulundu” durumunda kullanılır.",
      missingNeedsScope: "İncelenen kapsamı yazın (örn. 2020-2024 arası sözleşmeler).",
      notApplicableNeedsReason: "“Uygulanamaz” için gerekçe yazın.",
      openClearsLinks: "Açık duruma dönerken belge bağlarını kaldırın.",
    },
    license: {
      version: "sürüm {version}",
      review: "gözden geçirme {date}",
    },
    copiedCount: "{count} öğe dosyada",
    noticeCopied: "{title} sürüm {version} dosyaya kopyalandı ({count} öğe, {license}).",
    noticeAlready: "{title} bu dosyada zaten var; yinelenen öğe eklenmedi.",
    noticeUpdated: "“{item}” güncellendi.",
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
