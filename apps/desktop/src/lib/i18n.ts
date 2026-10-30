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
 * `screens/Checklists.tsx`, `screens/Ai.tsx`, `screens/Search.tsx`,
 * `screens/Settings.tsx`, `components/JobQueueCard.tsx`,
 * `components/WorkerBudgetCard.tsx`, `lib/checklist.ts`, `lib/candidate.ts`,
 * `lib/transferScope.ts`, `lib/jobs.ts`, `lib/parseProgress.ts`, `lib/budget.ts`,
 * `lib/export-coverage.ts`, `lib/reminders.ts`, `lib/search.ts`,
 * `lib/search-scope.ts`, `lib/rollback.ts`, `components/Notes.tsx`,
 * `lib/notes.ts`, `screens/Cases.tsx` ve `lib/ocr.ts`. Kalan ekranlar
 * (İnceleme, Emsal, Sağlayıcılar) ve yardımcı bileşenler
 * (`DualReader`) metinlerini hâlâ kendi dosyalarında taşır;
 * `lib/providers.ts`, `lib/arch.ts`, `lib/intext.ts`, `lib/inventory.ts`,
 * `lib/package.ts`, `lib/precedents.ts`, `lib/attack.ts`, `lib/reader.ts` ve
 * diğer yardımcı lib'ler de kendi metinlerini taşır. Taşıma sırası ve sayısı
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
  transferScope: {
    skip: {
      notParsed: "ayrıştırılmamış",
      emptyText: "metin yok",
      noVersion: "sürüm yok",
      missing: "bulunamadı",
    },
    skippedSummary: "Gönderilmeyen {total} belge: {parts}",
    scopeNoPreview: "Onay kapsamı: {documents} belge · içerik önizlemesi alınmadı",
    scopeWithPreview:
      "Onay kapsamı: {documents} belge · gönderilecek: {previewDocuments} belge, " +
      "{segments} segment, {bytes}",
  },
  candidate: {
    kind: {
      event: "Olay",
      person: "Kişi",
      date: "Tarih",
      claim: "İddia",
      summary: "Özet",
      answer: "Yanıt",
    },
    kindHint: {
      event: "title zorunlu; date, participants, description isteğe bağlı.",
      person: "name zorunlu; role, organization isteğe bağlı.",
      date: "rawText + kind zorunlu; isoDate yalnız gün kesinse (YYYY-AA-GG).",
      claim: "text zorunlu; assertedBy, amountText isteğe bağlı.",
      summary: "text zorunlu; en az bir alıntı taşımalı.",
      answer: "text zorunlu; question isteğe bağlı.",
    },
    sample: {
      eventTitle: "Ödeme yapılmadı",
      eventDate: "12 Mart 2024",
      personName: "Ahmet Yılmaz",
      personRole: "davaci",
      dateRaw: "12 Mart 2024",
      claimText: "Kira bedeli ödenmedi.",
      summaryText: "Dosyada ödeme yapılmadığı ileri sürülüyor.",
      answerText: "Ödeme yapıldığına dair belge yok.",
    },
    state: {
      valid: "geçerli",
      partial: "kısmi",
      invalid: "geçersiz",
      legacy: "doğrulanmamış (eski kayıt)",
    },
    quoteState: {
      verified: "alıntılar birebir doğrulandı",
      partial: "alıntı kısmen doğrulandı",
      failed: "alıntı doğrulanamadı",
      noQuotes: "alıntı yok",
      notChecked: "alıntı denetlenmedi",
      legacy: "denetlenmemiş (eski kayıt)",
    },
    line: {
      exact: "birebir bulundu",
      normalized: "normalizasyonla eşleşti",
      ambiguous: "birden fazla yerde",
      pageMismatch: "başka sayfada bulundu",
      notFound: "metinde yok",
      unverifiable: "doğrulanamadı",
    },
    unverifiableReason: {
      versionNotFound: "belge sürümü bulunamadı",
      caseMismatch: "belge başka dosyaya ait",
      noText: "belgenin metni yok",
    },
    position: {
      missing: "konum yok",
      page: "sayfa {page}",
      pageChar: "sayfa {page} · karakter {char}",
    },
    summary: {
      none: "alıntı yok",
      exact: "{exact}/{total} alıntı birebir",
    },
    draft: {
      empty: "Aday yükü boş.",
      notObject: "Aday yükü bir JSON nesnesi olmalı.",
      unparsable: "JSON çözülemedi: {message}",
      unknownError: "bilinmeyen hata",
    },
    blocked: {
      legacy: "Aday bu sürümde doğrulanmamış; önce yeniden doğrulayın.",
      quoteFailed: "Adaydaki alıntı belge metninde bulunamadı; kaynak olarak kullanılamaz.",
      invalidSchema: "Aday şeması geçersiz; yükü düzeltin.",
    },
  },
  ai: {
    lead:
      "AI yalnız aday üretir. İçerik cihazdan ancak dosya kapsamlı, süreli ve iptal " +
      "edilebilir bir aktarım onayıyla çıkar; her gönderim makbuzla kayda alınır.",
    offTitle: "AI kapalı.",
    offLead: "Kayıtlı sağlayıcı yalnız",
    offTail: "Bu durumda hiçbir içerik gönderilmez;",
    offEnd: "döner.",
    provider: "Sağlayıcı",
    providerNone: "yok — yalnız off",
    coverage: "Kapsam",
    coverageLine:
      "{documents} belge · {pages} sayfa · {ocrPages} OCR · {emptyPages} boş sayfa · " +
      "{unparsed} ayrıştırılmamış",
    purpose: "Amaç",
    purposeSummary: "Özet",
    purposeEvents: "Olay çıkarımı",
    purposeClaims: "İddia çıkarımı",
    approve: "Aktarım onayı ver",
    showContent: "Gönderilecek içeriği göster",
    run: "Çalıştır",
    revoke: "İptal et",
    previewTitle: "Gönderilecek içerik",
    previewSub: "{documents} belge · {segments} segment · {bytes}",
    skippedSuffix: "— bu belgeler gönderilmez.",
    previewEmpty: "ayrıştırılmış metin yok — bu dosyada gönderilecek içerik bulunmuyor",
    consentTail: "Onay {id}… · {provider} · iptal edilene kadar geçerli.",
    runsTitle: "Çalıştırmalar",
    runsSub: "Durum, sağlayıcı ve kullanım raporu · aday doğrulaması seçili çalıştırmada",
    colKind: "Tür",
    colState: "Durum",
    colStarted: "Başlangıç",
    colCandidates: "Adaylar",
    runsEmpty: "Henüz çalıştırma yok. Sağlayıcı bağlanınca burada görünür.",
    selectRun: "Seç ve adayları gör",
    selectedRun: "Seçili · adayları yenile",
    validateTitle: "Aday doğrula ve kaydet",
    validateSub:
      "Yük şemaya göre doğrulanır, alıntılar belgenin gerçek metninde aranır; durum " +
      "hesaplanır — elle “geçerli” yazılamaz.",
    currentRun: "Seçili çalıştırma:",
    noRun: "Önce yukarıdaki tablodan bir çalıştırma seçin; aday o çalıştırmaya bağlanır.",
    kindLabel: "Aday türü",
    draftLabel: "Aday yükü (JSON)",
    validate: "Doğrula",
    validateAndSave: "Doğrula ve kaydet",
    errorBadge: "{count} hata",
    warningBadge: "{count} uyarı",
    colQuote: "Alıntı",
    colPosition: "Konum",
    colOccurrences: "Tekrar",
    colText: "Metin",
    claimedPage: "model sayfa {page} dedi",
    issuesSummary: "Doğrulama sorunları ({count})",
    limits:
      "Doğrulamanın sınırları: yalnız kasada ayrıştırılmış metin aranır; OCR yazım hatası " +
      "eşleşmeyi etkileyebilir, maskeli bağlamın özgün kaynağa geri eşlenmesi bu adımda " +
      "yapılmaz. Aday metni modelin ürettiği hâliyle saklanır.",
    candidatesTitle: "Adaylar",
    candidatesSub: "Kabul edilene kadar hukuki kayıt sayılmaz",
    colSchema: "Şema",
    colContent: "İçerik",
    neverChecked: "denetlenmemiş",
    candidateBadge: "aday: {state}",
    quotesToggle: "{count} alıntı",
    issuesToggle: "{count} sorun",
    revalidate: "Yeniden doğrula",
    promote: "İncelemeye gönder",
    promoteTitle: "İnceleme kuyruğuna gönder",
    noticeValid: "Aday doğrulandı ve kaydedildi (geçerli).",
    noticePartial:
      "Aday kaydedildi; kısmi — kaynakları eksik veya alıntılar tam doğrulanmadı.",
    noticeInvalid: "Aday kaydedildi ama geçersiz; incelemeye gönderilemez.",
    noticePromoted:
      "Aday inceleme kuyruğuna gönderildi; kabul edilene kadar hukuki kayıt değil.",
  },
  jobQueue: {
    title: "İş kuyruğu",
    sub: "Ayrıştırma ve uzun işler kalıcı kayıtta tutulur. Uygulama çökerse yarıda kalan iş açılışta yeniden sıraya alınır; \"Başlat\" ile hemen sürdürülür.",
    refresh: "Yenile",
    locked: "Kuyruk kasa açılınca görünür.",
    colType: "Tür",
    colState: "Durum",
    colProgress: "İlerleme",
    colAttempt: "Deneme",
    colAction: "Eylem",
    empty: "Kayıtlı iş yok.",
    progressAria: "İş ilerlemesi: {line}",
    recover: "Bekleyen işleri kurtar",
    recoverNote:
      "Kurtarma, kesintiye uğramış işleri yeniden sıraya alır; iptal istenmiş işleri kapatır.",
    unblocked: "Engel kaldırıldı; iş yeniden sıraya alındı.",
    limit:
      "Sınır: kuyruk yürütücüsü tek süreçlidir (kasa başına tek yazıcı). Parola bekleyen belge işi \"Engelli\" kalır; parolayla yeniden ayrıştırınca sıraya döner. İlerleme sütunu ayrıştırma sırasında yazılır (en çok 0,3 sn'de bir güncellenir, kart 1,2 sn'de bir tazelenir) — çok kısa belgede sayaç 0'dan %100'e atlayabilir; bitmiş işte sayaç gösterilmez. Kuyruk gecikmesi ölçümü release derlemesinde yapılır.",
    busy: "Kuyruk okunuyor…",
    unavailable: "Kuyruk okunamadı.",
  },
  workerBudget: {
    title: "Kaynak bütçeleri",
    subLead:
      "Her worker türü kendi CPU, bellek, çıktı, açık dosya ve süre sınırıyla koşar; aşım ayrı hata koduyla (",
    subTail: ") raporlanır.",
    refresh: "Yenile",
    locked: "Bütçe profilleri kasa açılınca okunur.",
    colProfile: "Profil",
    colCpu: "CPU",
    colMemory: "Bellek",
    colOutput: "Çıktı",
    colFiles: "Dosya",
    colTime: "Süre",
    busy: "Profiller okunuyor…",
    notFound: "Profil bulunamadı.",
    limitLead: "Sınır: CPU süresi ve açık dosya sayısı POSIX kabuğunda",
    limitMiddle:
      "ile uygulanır — Windows'ta bu ikisi uygulanmaz (süre ve çıktı bütçesi uygulanır). Bellek gözcüsü 500 ms'de bir örnek alır; sınır örnekler arasında aşılabilir, aşımda süreç durdurulur ve iş",
    limitTail:
      "ile kapanır. AI profilinde CPU sınırı yoktur: çağrı yerel süreç değildir, duvar saati ve yanıt boyutu ölçülür. Bu değerler başlangıç ayarlarıdır; F15 ölçümüyle güncellenir.",
  },
  jobs: {
    state: {
      queued: "Bekliyor",
      blocked: "Engelli",
      running: "Yürüyor",
      validating: "Doğrulanıyor",
      retry_wait: "Tekrar bekliyor",
      canceling: "İptal ediliyor",
      interrupted: "Kesildi",
      partial: "Kısmi bitti",
      completed: "Bitti",
      failed: "Başarısız",
      canceled: "İptal edildi",
      leased: "Sahiplenildi",
      succeeded: "Bitti",
      cancelled: "İptal edildi",
    },
    stateLower: {
      running: "yürüyor",
      validating: "doğrulanıyor",
      queued: "bekliyor",
      blocked: "engelli",
      retry_wait: "tekrar bekliyor",
      canceling: "iptal ediliyor",
      interrupted: "kesildi",
      partial: "kısmi bitti",
      completed: "bitti",
      failed: "başarısız",
      canceled: "iptal edildi",
    },
    type: {
      parse_document: "Belge ayrıştırma",
      import: "İçe aktarma",
      ocr_page: "OCR",
      index: "Dizinleme",
      export: "Çıktı",
      backup: "Yedek",
    },
    blockedReason: {
      password: "parola bekliyor",
      dependency: "bağımlılık bekliyor",
      quota: "kota bekliyor",
      permission: "izin bekliyor",
      vault_locked: "kasa kilitli",
    },
    unknownState: "bilinmeyen ({state})",
    unknownBlocked: "engel: {reason}",
    action: {
      cancel: "İptal et",
      retry: "Yeniden dene",
      run: "Başlat",
      unblock: "Engeli kaldır",
    },
    cancelNeverRan: "İş iptal edildi (hiç çalışmadı).",
    cancelStopped: "Çalışan süreç durduruldu; iş iptal ediliyor, sonuç yazılmayacak.",
    cancelRequested:
      "İptal istendi; iş sonucu bekleniyor — süreç durmadan \"iptal tamam\" denmez.",
    summaryEmpty: "Kuyruk boş — bekleyen iş yok.",
    summaryTotal: "{count} iş",
    summaryPart: "{count} {state}",
    attentionFailed:
      "{count} iş başarısız. Hata kodu iş listesinde; \"Yeniden dene\" yeni bir deneme açar.",
    attentionBlocked:
      "{count} iş engelli (parola/bağımlılık/kota). Engel kalkınca \"Engeli kaldır\" ile sıraya döner.",
    attentionDependency:
      "{count} iş bağımlılığı bittiği için hiç çalıştırılmadı (sessizce beklemez).",
    attentionRetry: "{count} iş tekrar denemesi bekliyor; süresi gelince kendiliğinden alınır.",
    attentionInterrupted:
      "{count} iş kesintiye uğradı; \"Bekleyen işleri kurtar\" ile yeniden sıraya alınır.",
    metaAttempt: "deneme {attempt}",
    start: {
      verbRetry: "Yeniden deneme",
      verbRun: "İş",
      line: "{verb} başladı — kaynak: {source} · {settings}; sonucu satırda göreceksiniz.",
      sourceStored: "kaydedilmiş ayarlar",
      sourceDefault: "varsayılanlar (özgün ayar bu işte kayıtlı değil)",
      settings: "OCR {ocr}, kodlama {encoding}",
      settingsDefault: "varsayılan ayarlar",
      ocrOn: "açık ({lang})",
      ocrOff: "kapalı",
      passwordRetry:
        "Belge parola ile açılmıştı; parola saklanmaz. Yeniden deneme başlatılmadı — Dosyalar ekranından parolayı girip yeniden ayrıştırın.",
      passwordRun:
        "Belge parola ile açılmıştı; parola saklanmaz. İş başlatılmadı — Dosyalar ekranından parolayı girip yeniden ayrıştırın.",
      notRunnableRetry:
        "İş yeniden sıraya alındı, ama bu iş türünü kendiliğinden başlatan yürütücü yok; ilgili ekrandan yeniden çalıştırın.",
      notRunnableRun:
        "Bu iş türünü kendiliğinden başlatan yürütücü yok; ilgili ekrandan çalıştırın.",
      noInputRetry:
        "İş yeniden sıraya alındı, ama girdisi (belge sürümü) çözülemedi; belgeyi belge ekranından yeniden ayrıştırın.",
      noInputRun:
        "İş başlatılamadı: girdisi (belge sürümü) çözülemedi; belgeyi belge ekranından yeniden ayrıştırın.",
      fallbackRetry: "İş yeniden sıraya alındı.",
      fallbackRun: "İş başlatılamadı.",
    },
    recovery: {
      requeued: "{count} iş yeniden sıraya alındı",
      canceled: "{count} iş iptal edildi",
      failed: "{count} iş tekrar bütçesi bittiği için kapandı",
      interruptedOnly:
        "{count} iş kesildi olarak işaretlendi; yeniden sıraya alınacak bir şey yok.",
      none: "Yarıda kalmış iş yok; yeniden sıraya alınacak bir şey bulunmadı.",
    },
  },
  parseProgress: {
    stage: {
      start: "Hazırlanıyor",
      queued: "Sırada (en fazla 2 ayrıştırma)",
      open: "Belge açıldı",
      pages: "Sayfalar okunuyor",
      store: "Kasaya yazılıyor",
      redact: "Sayfalar maskeleniyor",
      pack: "PDF paketleniyor",
      verify: "Çıktı doğrulanıyor",
      scan: "Bağımsız tarama",
    },
    fallback: "Ayrıştırılıyor",
    line: "{label}: {done} / {total} · %{percent}",
    countLine: "{label}: {done}",
  },
  budget: {
    zeroMib: "0 MiB",
    mib: "{value} MiB",
    notApplicable: "uygulanmaz",
    seconds: "{value} sn",
    zeroMs: "0 ms",
    milliseconds: "{value} ms",
    cpuLine: "CPU {value}",
    memoryLine: "bellek {value}",
    outputLine: "çıktı {value}",
    filesLine: "dosya {value}",
    timeLine: "süre {value} sn",
    peakMemory: "tepe bellek {value}",
    response: "yanıt {value}",
    openFiles: "{count} dosya",
    dimension: {
      cpu: "CPU süresi",
      memory: "bellek",
      output: "çıktı boyutu",
      open_files: "açık dosya sayısı",
      time: "süre",
    },
    warningLimit: "Bütçe aşıldı: {label} (sınır {limit})",
    warning: "Bütçe aşıldı: {label}",
  },
  exportCoverage: {
    row: {
      documents: "Belge",
      versions: "Belge sürümü",
      parseRuns: "Ayrıştırma koşusu",
      pages: "Sayfa",
      textPages: "Metinli sayfa",
      ocrPages: "OCR ile okunan sayfa",
      emptyPages: "Boş sayfa",
      segments: "Metin segmenti",
      unparsed: "Ayrıştırılmamış belge",
      failedRuns: "Başarısız ayrıştırma koşusu",
    },
    headline:
      "{documents} belge · {pages} sayfa ({textPages} metinli, {ocrPages} OCR, {emptyPages} boş) · {unparsed} ayrıştırılmamış belge",
    state: {
      none: "Kapsam yok",
      full: "Kapsam tam",
      partial: "Kapsam kısmi",
    },
    stateDetail: {
      none: "Dosyada belge yok; bu çıktı hiçbir incelemeyi kapsamaz.",
      full: "Her belge ayrıştırıldı ve her sayfada metin var.",
      partial: "Kapsam dışı içerik var ({count} uyarı); incelenmedi ≠ temiz.",
    },
    embedWhere: {
      json: "coverage bloğu",
      csv: "baştaki # yorum satırları",
      docx: "“İncelenen kapsam” bölümü",
      pdf: "“İncelenen kapsam” bölümü",
      package: "KAPSAM.md + manifest.coverage",
    },
    embedTarget: {
      json: "JSON",
      csv: "CSV",
      docx: "DOCX",
      pdf: "PDF",
      package: "Devir paketi",
    },
    embedNote: "Kapsam çıktıya gömüldü: {headline}",
    embedNoteWhere: "Kapsam çıktıya gömüldü ({where}): {headline}",
    emptyWarning:
      "Belgesiz dosya: çıktı yalnız dosya künyesini taşır, hiçbir belge incelenmiş sayılmaz.",
  },
  reminders: {
    notification: "Bu dosyada yaklaşan bir görev tarihi var.",
    label: {
      day_before: "Yarın için hatırlatıcı",
      on_day: "Bugün için hatırlatıcı",
      off: "Hatırlatıcı kapalı",
    },
    policy: {
      none: "Hatırlatma yok",
      on_day: "Günü gelince",
      day_before: "Bir gün önce",
    },
  },
  searchFilters: {
    dateRole: {
      event: "Olay",
      document: "Belge",
      notification: "Tebliğ",
      contract: "Sözleşme",
      hearing: "Duruşma",
      deadline_candidate: "Süre adayı",
      unknown: "Bilinmiyor",
    },
    ocrStatus: {
      any: "Tümü",
      ocr: "OCR ile çıkarıldı",
      native: "Belgeden (yerel) metin",
    },
    textOrigin: {
      native: "Yerel metin",
      ocr: "OCR",
      mixed: "Karma (OCR + metin)",
      none: "Metin yok",
      unknown: "Kaynak bilinmiyor",
    },
    describe: {
      documentTypes: "Belge türü: {types}",
      dateRole: "Tarih rolü: {role}",
      tags: "Etiket (hepsi): {tags}",
      ocrStatus: "Metin kaynağı: {label}",
    },
    result: {
      shown: "{returned} sonuç gösteriliyor",
      shownMore: "{shown} · devamı var",
      withTotal: "{total} eşleşme · {shown}",
    },
    total: {
      counting: "Eşleşme sayısı hesaplanıyor…",
      failed: "Eşleşme sayısı alınamadı: {message}",
      staleQuery: "Sayı önceki sorguya ait; yeni aramada tazelenir.",
      done: "{total} eşleşme · sayım {ms} ms sürdü",
    },
    verification: {
      base: "{candidates} aday doğrulandı · {ms} ms",
      stoppedEarly: "{base} · sayfa doldu, tarama kesildi",
      exhausted: "{base} · adaylar tükendi",
    },
    mode: {
      auto: "Otomatik",
      exact: "Tam",
      flexible: "Esnek",
      phrase: "İfade",
      docket: "E./K. numarası",
    },
    modeHint: {
      exact:
        "Türkçe karakterler korunur (ı ≠ i, ş ≠ s); kelimeler bitişik geçmeli, son kelime önek olabilir.",
      flexible:
        "Türkçe karakterler sadeleşir (ı→i, ş→s); noktalama önemsiz, son kelime önek olabilir.",
      phrase: "Kelimeler aynı sırada; araya noktalama girebilir, son kelime tam olmalı.",
      docket: "2024/123 E. biçimi; baştaki sıfırlar yok sayılır, E./K. rolü eşleşir.",
      autoPrefix: "Otomatik: {label} —",
    },
  },
  searchScope: {
    label: {
      case_documents: "Dosya belgeleri",
      precedent_library: "Emsal kütüphanesi",
    },
    notice: {
      case_documents:
        "Kapsam: yalnız aktif dosyanın belgeleri. Emsal kütüphanesi bu aramaya dahil edilmez.",
      precedent_library:
        "Kapsam: emsal kütüphanesi. Sonuçlar aktif dosyanın belgelerinden gelmez; dosya aramasına karışmaz ve delil sayılmaz.",
    },
    citationMissing: "Künye eksik",
    citationDocket: "E. {no}",
    citationDecision: "K. {no}",
    matchSource: {
      metadata: "künye numarası",
      text: "karar metni",
    },
    summary: {
      base: "{matched} eşleşme · {scanned} tam metin tarandı",
      withoutText: "{base} · {count} kayıt tam metinsiz (aranmadı)",
    },
    snippetForCopy: "{citation}\n…{snippet}\n(kaynak: emsal kütüphanesi, sürüm {version})",
  },
  search: {
    leadStart: "İki kapsam vardır ve",
    leadEmphasis: "karışmaz",
    leadRest:
      ": dosya belgeleri (yalnız seçili dosya) ve emsal kütüphanesi (yalnız emsal kayıtlarının tam metinleri). Her sonuç kendi kapsamıyla etiketlenir; emsal sonucu dosyaya delil olarak eklenmez.",
    scopeGroup: "Arama kapsamı",
    queryLabel: "Aranan metin",
    queryPlaceholder: "Örn. ödeme, \"kira bedeli\", 2024/123 E.",
    modeLabel: "Arama kipi",
    searching: "Aranıyor…",
    submit: "Ara",
    filterType: "Belge türü",
    filterTypeAll: "Tüm türler",
    filterOcr: "Metin kaynağı",
    filterDateRole: "Tarih rolü (belgeye bağlı olay)",
    filterDateRoleAll: "Tüm roller",
    tagsLabelStart: "Etiketler (seçilenlerin",
    tagsLabelEmphasis: "hepsi",
    tagsLabelRest: "belgede olmalı)",
    tagsEmpty: "Bu dosyada henüz etiket yok. Belge listesinden (Dosyalar ekranı) etiket ekleyin.",
    tagCount: "{count} belgede",
    searchingIn: "{title} içinde aranıyor.",
    noCaseYet: "Önce bir dosya oluşturun.",
    serverFilters: "Filtreler sunucuda uygulanır; sayfa başına {size} sonuç gelir.",
    clearFilters: "Filtreleri temizle",
    libraryFiltersNote:
      "Emsal kütüphanesi kapsamında dosya seçimi ve belge filtreleri (tür, etiket, metin kaynağı, tarih rolü) uygulanmaz. Kütüphane yalnız emsal kayıtlarının tam metnini ve künye numaralarını tarar; sayfa başına {size} sonuç gelir.",
    staleFilters: "Filtreler",
    staleMode: "Arama kipi",
    staleTail: "değişti; aşağıdaki sonuçlar",
    staleEmphasis: "önceki",
    staleRest: "ayarlarla alındı.",
    searchAgain: "Yeni ayarlarla ara",
    resultsTitle: "Sonuçlar",
    resultsSub: "{summary} · kapsam: yalnız bu dosya (emsal kütüphanesi hariç) · kip: {mode}",
    filterCount: "{count} filtre",
    emptyNoMatch: "Eşleşme yok.",
    emptyRelax: "Filtreleri gevşetmeyi deneyin.",
    emptyParsed: "Belgeleri “Ayrıştır” ile metne çevirdiniz mi?",
    unknownDocument: "belge bilinmiyor",
    pageBadge: "sayfa {page}",
    openLeft: "Solda aç",
    openLeftTitle: "Sonucu sol panelde aç",
    openRight: "Sağda aç",
    openRightTitle: "Sonucu sağ panelde aç",
    noVersionTitle: "Sonuçta sürüm kimliği yok",
    hitMeta: "segment {segment} · sürüm {version}",
    loadingMore: "Yükleniyor…",
    loadMore: "Daha fazla sonuç",
    cursorMissing: "Devamı var ancak imleç gelmedi; yeniden arayın.",
    allShown: "Tüm sonuçlar gösterildi.",
    openExplain:
      "“Solda aç / Sağda aç” sonucun kaydedildiği sürümü açar; iki sonucu yan yana koyup karşılaştırabilirsiniz.",
    libraryTitle: "Emsal kütüphanesi sonuçları",
    librarySubPrefix: "{summary} · kapsam:",
    librarySubSuffix: "· kip: {mode}",
    libraryQuery: "Sorgu: “{query}”",
    libraryEmpty:
      "Emsal kütüphanesinde eşleşme yok. Bu sonuç “karar yok” hükmü değildir; künyeyi esas/karar numarasıyla da arayabilirsiniz.",
    libraryBadge: "emsal",
    matchBadge: "eşleşme: {label}",
    copied: "Kopyalandı",
    copyQuote: "Alıntıyı kopyala",
    libraryMeta: "sürüm {version} · edinim {date}",
    positionLine: "konum {position}",
    positionMissing: "konum yok (künye eşleşmesi)",
    libraryNote:
      "Bu sonuçlar aktif dosyaya delil olarak eklenmez. Alıntı, emsal kaydındaki tam metinle doğrulanmalıdır; tam metni olmayan kayıtlar metin aramasında taranmaz.",
    errorNoVersion: "Sonuç belge sürümü taşımıyor; okuyucuda açılamaz. Belgeyi yeniden ayrıştırın.",
    errorClipboard: "Panoya kopyalama izni yok; metni seçip elle kopyalayın.",
  },
  settings: {
    title: "Ayarlar ve yedek",
    lead: "Yedek, kasa başlığını, SQLCipher veritabanını ve şifreli blob'ları tek klasöre kopyalar. Açık metin üretilmez; yedeği açmak için kasa parolası veya kurtarma anahtarı gerekir.",
    pickDirTitle: "Yedek klasörünü seç",
    passwordMismatch: "Yeni parolalar eşleşmiyor.",
    restoreDestRequired: "Geri yükleme için hedef konum girin.",
    vaultTitle: "Kasa",
    vaultOpenShort: "açık",
    vaultLockedShort: "kilitli",
    vaultOpenBadge: "kasa açık",
    vaultLockedBadge: "kasa kilitli",
    vaultId: "Kimlik",
    vaultPath: "Konum",
    schemaVersion: "Şema sürümü",
    appVersion: "Sürüm",
    versionValue: "{version} (pre-alpha)",
    preUpdateTitle: "Güncelleme öncesi yedek alındı.",
    preUpdateLead: "Bu kasa yeni şemaya geçirilmeden önce doğrulanmış bir kopya yazıldı:",
    preUpdateTail:
      "Güncelleme yarıda kalırsa bu dosya geri konur; uygulama yedek alınamadıysa güncellemeyi başlatmaz.",
    rollbackTitle: "Güncelleme öncesi yedeğe dön",
    rollbackSub: "Kasa dosyasının güncellemeden önceki doğrulanmış kopyası; veriyi geri alır.",
    scanBackups: "Yedekleri tara",
    rollbackLocked: "Kasa kilitliyken yedek listesi okunamaz.",
    rollbackEmpty:
      "Kasa klasöründe güncelleme öncesi yedek yok. Bu yedek yalnız şema güncellemesi sırasında, doğrulanmış olarak yazılır.",
    rollbackConfirmLabel: "Onay için “{token}” yazın",
    rollbackSubmit: "Seçili yedeğe dön",
    rollbackDoneTitle: "Geri dönüş tamamlandı.",
    rollbackDoneBackup: "Yedek:",
    rollbackDoneSafety: "Güvenlik kopyası:",
    rollbackDoneHint: "Kasa açık; ekranlar tazelendiğinde geri dönen veri görünür.",
    backupTitle: "Yedek al",
    backupSub: "Tutarlı snapshot; yedek açılmadan önce doğrulanır.",
    backupPick: "Yedek klasörü seç",
    backupReady: "Yedek hazır:",
    backupSummary: "{count} dosya · {size} · manifest {hash}…",
    backupArchive: "Tek dosya:",
    backupLocked: "Kasa kilitliyken yedek alınamaz. Yedeği kasa dosyasından ayrı bir diskte saklayın.",
    autoTitle: "Otomatik yedek",
    autoSub: "Klasör seçilirse kasa açıkken süresi dolunca yedek alınır; sonuç kayda geçer.",
    autoPick: "Klasör seç",
    autoDir: "Klasör",
    autoDirNone: "seçilmedi",
    autoDays: "Aralık",
    autoDaysValue: "{days} gün",
    autoLast: "Son yedek",
    autoLastNone: "hiç",
    autoDisable: "Otomatik yedeği kapat",
    autoReset: "Sıfırla",
    autoResetTitle: "Bir sonraki kontrolde yeniden yedek alınır",
    gcTitle: "Kullanılmayan veriler",
    gcSub: "Hiçbir belge sürümüne veya türev sayfaya bağlı olmayan şifreli blob'lar.",
    gcScan: "Tara",
    gcClean: "Temizle",
    gcCandidates: "{count} aday · {size}",
    gcDeleted: "{count} kayıt silindi · {size} boşaldı",
    gcMissing: "Dosya uyarısı: {files}",
    gcHint: "Önce \"Tara\" ile adayları görün; silme yalnız referanssız kayıtlara dokunur.",
    restoreTitle: "Yedekten geri yükle",
    restoreSub: "Yeni bir kasaya kopyalar; mevcut kasanın üzerine yazmaz.",
    restoreDest: "Yeni kasa konumu",
    restoreDestPlaceholder: "~/Dosyalar/GeriYuklenenKasa",
    restorePick: "Yedek seç ve geri yükle",
    restoreSelected: "Seçilen yedek: {dir}",
    restoreDone: "Geri yükleme tamamlandı:",
    restoreVerifiedLead: "Kopyalamadan önce",
    restoreVerifiedTail: "dosya · {size} manifest sha256 ile doğrulandı ({ms} ms).",
    restoreUnlisted: "Yedekte sayılmayan {count} dosya kasaya kopyalanmadı:",
    restoreOpenHint: "Şimdi Kasa ekranından bu konumu ve parolanızı girerek açın.",
    passwordTitle: "Parola değiştir",
    passwordSub: "Yalnız parola yuvası yeniden sarılır; kasa içeriği ve kurtarma anahtarı değişmez.",
    passwordOld: "Mevcut parola",
    passwordNew: "Yeni parola (en az 10 karakter)",
    passwordNew2: "Yeni parola (tekrar)",
    passwordSubmit: "Parolayı değiştir",
    passwordDone: "Parola değiştirildi. Kurtarma anahtarınız hâlâ geçerli.",
    diagnosticsTitle: "Teşhis",
    diagnosticsBody:
      "Uygulama belge içeriğini günlüğe yazmaz; yalnız hata kodları ve sürüm bilgisi tutulur. Destek için hata kodu yeterlidir.",
  },
  rollback: {
    confirmToken: "GERİ DÖN",
    versionUnverified: "v{version} · doğrulanmadı",
    verifiedSchema: "doğrulandı · şema v{version}",
    verifiedMismatch: "doğrulandı · dosya v{verified}, ad v{claimed}",
    usable: "kullanılabilir",
    unusable: "kullanılamaz",
    unusableProblem: "kullanılamaz — {problem}",
    nameMismatch:
      "Dosya adı v{claimed} diyor, dosya v{verified} taşıyor; ad yeniden adlandırılmış olabilir.",
    consequence1: "Kasa kilitlenir; açık kasadaki tüm bağlantılar kapatılır.",
    consequence2:
      "Kasa dosyası seçilen güncelleme öncesi yedekle değiştirilir; güncelleme sonrası kayıtlar kasadan çıkar.",
    consequence3:
      "Değiştirilen dosya silinmez: aynı klasörde `vault.db.rollback-<zaman>.safety` güvenlik kopyası olarak kalır.",
    consequence4:
      "Kasa yeniden açılır ve bu uygulamanın şema sürümüne ilerletilir; ikili (uygulama) sürümü düşürülmez.",
    consequence5:
      "Geri dönüş ortasında süreç ölürse kasa dosyası yerinde kalır; yarım kalan geçici dosya bir sonraki denemede silinir.",
    consequence6:
      "Disk doluysa geri dönüş başlamaz (`E_DISK_FULL`); kasa ve yedek olduğu gibi kalır. Kasa başka bir süreçte açıksa (dolu `-wal`) da başlamaz.",
    noBackups: "Kasa klasöründe güncelleme öncesi yedek yok.",
    selectFirst: "Önce bir yedek seçin.",
    notVerifiedSelected: "Seçilen yedek doğrulanamadı; geri dönüş yapılmaz.",
    confirmRequired: "Onay için “{token}” yazın.",
    summary: "Veri v{restored} sürümüne döndü; kasa güncel şema v{resumed} olarak yeniden açıldı.",
  },
  notes: {
    title: "Notlar",
    cardSub: "{count} not · taslak ve düzenleme geçmişi kasa içinde tutulur",
    emptyBody: "Not metni boş olamaz.",
    notFound: "Not bulunamadı; liste yenilendi.",
    noAnchors: "Kaynak alıntı yok. Okuyucuda metin seçip “Seçimi kaynak olarak kaydet” deyin.",
    titleOptional: "Başlık (isteğe bağlı)",
    sourceAnchors: "Kaynak alıntılar",
    body: "Not",
    bodyPlaceholder: "Gözlem, eksik belge, duruşma notu…",
    lockedDraft: "kasa kilitliyse taslak yazılamaz",
    discardDraft: "Taslağı sil",
    add: "Notu ekle",
    untitled: "Başlıksız not",
    versionLabel: "sürüm {version}",
    edit: "Düzenle",
    delete: "Sil",
    editTitle: "Notu düzenle",
    editMeta: "sürüm {version} · revizyon {revision} · {time}",
    close: "Kapat",
    draftDiffers: "Kasadaki taslak bu nottan farklı ({time}).",
    draftStaleShort: "Taslak eski bir sürüme dayanıyor.",
    draftStaleLong:
      "Taslak daha eski bir sürüme dayanıyor; kaydederken çakışma ekranı açılabilir.",
    loadDraft: "Taslağı yükle",
    conflictTitle: "Sürüm çakışması",
    conflictHint:
      "Not başka bir pencerede değişmiş. Eski görünümün üzerine sessizce yazılmaz; farkı görüp seçim yapın.",
    conflictStats: "Benim metnim {mine} satırda farklı, kasadaki metin {theirs} satırda farklı.",
    keepMine: "Benim metnimi kaydet",
    keepCurrent: "Güncel metni koru",
    currentVersion: "Kasadaki güncel sürüm {version} · {time}",
    editTitleLabel: "Başlık",
    editBodyLabel: "Metin",
    unsaved: "Kaydedilmemiş değişiklik",
    saveFirst: "Önce kaydedin",
    undoTitle: "Son kaydı geri al",
    redoTitle: "Geri alınan kaydı ileri al",
    undo: "Geri al",
    redo: "İleri al",
    historyTitle: "Sürüm geçmişi",
    historyEmpty: "Kayıt yok.",
    footnote:
      "Notlar düz metindir; HTML çalıştırılmaz. Taslak yalnız açık kasaya yazılır, tarayıcı deposuna kaydedilmez. Geri/ileri al kasa içindeki gerçek revizyonları kullanır.",
    draftIdle: "Taslak yok",
    draftSaving: "Kaydediliyor…",
    draftSaved: "Taslak kasaya kaydedildi",
    draftFailed: "Taslak kaydedilemedi",
    originCreate: "Oluşturuldu",
    originEdit: "Düzenlendi",
    originUndo: "Geri alındı",
    originRedo: "İleri alındı",
    originUnknown: "Bilinmeyen kaynak",
  },
  vaultInput: {
    tooShort: "Parola en az {min} karakter olmalı.",
    common: "Bu parola çok yaygın; farklı bir parola seçin.",
    repeated: "Tek karakterin tekrarı parola olarak kullanılamaz.",
    digitsOnly: "Yalnız rakamlardan oluşan parola kullanılamaz.",
    mismatch: "Parolalar eşleşmiyor.",
  },
  cases: {
    lead:
      "Her dosya kendi belge, not ve kronolojisini taşır. Bir dosyanın verisi başka dosyanın sorgusuna örtük olarak eklenmez (INV-05).",
    listTitle: "İnceleme dosyaları",
    listSubtitle: "{count} kayıt · {scope}",
    scopeArchived: "arşiv dahil",
    scopeActive: "yalnız aktif",
    archiveToggle: "Arşiv",
    newCase: "Yeni dosya",
    cancel: "Vazgeç",
    close: "Kapat",
    fieldTitle: "Başlık",
    titlePlaceholder: "Örn. Kira alacağı — 2026",
    fieldKind: "Tür",
    fieldReference: "Esas / referans",
    referencePlaceholder: "2026/0000",
    fieldArea: "Alan",
    areaPlaceholder: "Kira, iş, aile…",
    creating: "Oluşturuluyor…",
    create: "Dosyayı oluştur",
    titleRequired: "Dosya başlığı zorunlu.",
    duplicateNotice: "Bu dosyada aynı içerik zaten var: “{name}”. Yeni kayıt oluşturulmadı.",
    imported: "“{name}” içe alındı (sürüm {version}).",
    emptyArchived: "Bu kasada kayıt yok.",
    emptyActive: "Henüz aktif dosya yok. “Yeni dosya” ile başlayın.",
    noReference: "referans yok",
    noArea: "alan belirtilmedi",
    versionLabel: "sürüm {version}",
    fieldJurisdiction: "Yargı yeri",
    fieldSensitivity: "Hassasiyet",
    fieldAiPolicy: "AI politikası",
    fieldCreated: "Oluşturma",
    fieldUpdated: "Son değişiklik",
    fieldId: "Kimlik",
    archive: "Arşivle",
    restore: "Geri al",
    delete: "Sil (geri dönüşümlü)",
    parseProgressAria: "Ayrıştırma ilerlemesi",
    parseSummary: "Ayrıştırma {status}: {pages} sayfa, {segments} metin bloğu",
    parseWarnings: "{count} uyarı",
    fileFilterDocuments: "Belgeler",
    anchorSaved: "Kaynak kaydedildi: “{quote}”",
    anchorRebound: "Kaynak güncel sürüme bağlandı.",
    noteCreated: "Kaynaklı not oluşturuldu: “{title}”",
    anchorMissing: "Kaynak kaydı bulunamadı; silinmiş olabilir.",
    anchorsTitle: "Kaynak alıntılar",
    anchorsSubtitle: "{count} bağ · alıntı segmentte doğrulanarak kaydedilir",
    anchorsEmpty: "Henüz kaynak yok. Okuyucuda metin seçip “Seçimi kaynağa bağla” deyin.",
    openAnchorTitle: "Alıntının sayfasını okuyucuda aç",
    openInReader: "Okuyucuda aç",
    documentsTitle: "Belgeler",
    documentsSubtitle: "{count} belge · özgün baytlar şifreli saklanır",
    parsePendingTitle: "Güncel sürümü ayrıştırılmamış belgeleri en fazla iki worker'la ayrıştırır",
    parsing: "Ayrıştırılıyor…",
    parsePending: "Bekleyenleri ayrıştır",
    addDocument: "Belge ekle",
    documentsEmpty: "Henüz belge yok. PDF, DOCX, TXT veya görüntü ekleyin.",
    noVersion: "sürüm yok",
    removeTagTitle: "“{tag}” etiketini kaldır",
    tagPlaceholder: "+ etiket (Enter)",
    tagAria: "{name} için etiket",
    openDocumentTitle: "Belgeyi okuyucuda aç",
    read: "Oku",
    parse: "Ayrıştır",
    ocr: "OCR",
    ocrTitle: "Taranmış sayfalar için OCR ({lang})",
    batchResult: "{succeeded} / {requested} belge ayrıştırıldı",
    batchDetail: "{pages} sayfa · eşzamanlılık tepe {peak} (en fazla 2)",
    batchFailed: "{count} başarısız",
    batchNonePending:
      "Bekleyen belge yok: her belgenin güncel sürümü için başarılı bir ayrıştırma var.",
    batchPages: "{count} sayfa",
    encodingLabel: "TXT kodlaması (düz metin belgeler)",
    encodingAuto: "Otomatik (BOM → UTF-8 → Türkçe)",
    encodingUtf8: "UTF-8",
    encodingWindows: "Windows-1254 (Türkçe)",
    encodingIso: "ISO-8859-9 (Latin-5)",
    encodingHint:
      "Otomatik seçim BOM'u, katı UTF-8'i ve Türkçe tek baytlık kodlamaları dener; hangisinin kullanıldığı ayrıştırma uyarısında yazılır. Metin bozuk görünürse kodlamayı açıkça seçip yeniden ayrıştırın.",
    pdfPasswordLabel: "Parolalı PDF parolası (yalnız bu oturum için)",
    pdfPasswordPlaceholder: "Parola gerekmiyorsa boş bırakın",
    footnote:
      "Aynı içerik ikinci kez eklenirse yeni belge oluşturulmaz; mevcut kayıt gösterilir. Parola kaydedilmez, yalnız ayrıştırma isteğinde worker'a iletilir. OCR taranmış sayfalar için çalışır ve dil paketi gerektirir.",
  },
  ocrLang: {
    tur: "Türkçe",
    eng: "İngilizce",
    deu: "Almanca",
    fra: "Fransızca",
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
