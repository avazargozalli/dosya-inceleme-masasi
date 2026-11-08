import { useEffect, useRef, useState } from "react";
import {
  api,
  toApiError,
  type AiCandidateRow,
  type AiContextItemRow,
  type AiOverview,
  type ApiError,
  type CandidateReport,
  type CaseRow,
  type ConsentPreview,
  type TransferConsentListRow,
  type TransferConsentRow,
  type TransferReceiptRow,
  type TransferSegmentRow,
} from "../lib/api";
import { formatDateTime, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import { consentScopeLine, skippedSummary } from "../lib/transferScope";
import {
  CANDIDATE_KINDS,
  candidateKindLabel,
  candidateStateLabel,
  candidateStateTone,
  draftJsonError,
  errorCount,
  parseIssues,
  promoteBlockedReason,
  quoteLineStateLabel,
  quoteLineTone,
  quotePositionLabel,
  quoteStateLabel,
  quoteStateTone,
  quoteSummary,
  warningCount,
} from "../lib/candidate";

/**
 * Onay **amaca özgüdür** (F09-T09): çalıştırmanın görev türü, onaylanan amaca
 * göre koşar. `summarize` sabiti, "İddia çıkarımı" için onay alınmışken modele
 * "özetle" demek olurdu.
 */
const PURPOSE_TASKS: Record<string, string> = {
  ozet: "summarize",
  olay_cikarimi: "extract_events",
  iddia_cikarimi: "extract_claims",
};

function purposeTask(purpose: string): string {
  return PURPOSE_TASKS[purpose] ?? purpose;
}

function purposeLabel(purpose: string): string {
  switch (purpose) {
    case "ozet":
      return t("ai.purposeSummary");
    case "olay_cikarimi":
      return t("ai.purposeEvents");
    case "iddia_cikarimi":
      return t("ai.purposeClaims");
    default:
      return purpose;
  }
}

/**
 * Envanter durumu **çekirdeğin** `active`/`providerCurrent` değerlerinden
 * okunur; arayüz kendi saatine göre "süresi geçti" demez. İptal, süre
 * dolmasından ve sağlayıcı değişiminden önce gelir.
 *
 * "Etkin" ile "yürütülebilir" aynı şey değildir: sağlayıcının uç noktası/modeli
 * değiştiyse kayıt yürürlükte kalır ama hiçbir profille çalıştırılamaz —
 * ekran bunu satırda yazar, kullanıcı "Çalıştır"a basana kadar gizlemez.
 */
function consentStateLabel(row: TransferConsentListRow): string {
  if (row.revokedAt) return t("ai.consentRevoked");
  if (!row.active) return t("ai.consentExpired");
  return row.providerCurrent ? t("ai.consentActive") : t("ai.consentProviderChanged");
}

function consentStateTone(row: TransferConsentListRow): string {
  if (row.revokedAt) return "plain";
  if (!row.active) return "warn";
  return row.providerCurrent ? "ok" : "warn";
}

/**
 * Makbuz durumu. `attempted` **başarı değildir**: gönderim denendi, sonucu
 * yazılmadı (süreç çöktü ya da yanıt gelmedi). Bilinmeyen durum olduğu gibi
 * yazılır — ekran tanımadığı bir değeri "gönderildi" saymaz.
 */
function receiptStateLabel(status: string): string {
  if (status === "dispatched") return t("ai.receiptDispatched");
  if (status === "failed") return t("ai.receiptFailed");
  if (status === "attempted") return t("ai.receiptAttempted");
  return status;
}

function receiptStateTone(status: string): string {
  if (status === "dispatched") return "ok";
  if (status === "failed") return "danger";
  return "warn";
}

/**
 * Alıntının **gönderim siciliyle** ilişkisi. Bilinmeyen değer olduğu gibi
 * yazılır — ekran tanımadığı bir durumu "gönderilen metinde bulundu" saymaz.
 */
function outboundLabel(state: string): string {
  if (!state) return t("common.unknown");
  if (state === "found") return t("ai.outboundFound");
  if (state === "missing") return t("ai.outboundMissing");
  if (state === "document_not_sent") return t("ai.outboundDocumentNotSent");
  if (state === "changed") return t("ai.outboundChanged");
  if (state === "masked") return t("ai.outboundMasked");
  if (state === "not_recorded") return t("ai.outboundNotRecorded");
  return state;
}

export function Ai() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [overview, setOverview] = useState<AiOverview | null>(null);
  const [consent, setConsent] = useState<TransferConsentRow | null>(null);
  // Gönderim maskesi: satır başına bir terim. Terimler **kasaya yazılmaz**;
  // çekirdek maskeyi uygular, sicile yalnız terim sayısı düşer.
  const [maskText, setMaskText] = useState("");
  const [consents, setConsents] = useState<TransferConsentListRow[]>([]);
  const [consentListBusy, setConsentListBusy] = useState(false);
  // Makbuz envanteri de **çekirdekten** okunur: yazılan makbuzun okuma yolu
  // yoktu, avukat neyin çıktığını göremiyordu.
  const [receipts, setReceipts] = useState<TransferReceiptRow[]>([]);
  const [receiptListBusy, setReceiptListBusy] = useState(false);
  const [preview, setPreview] = useState<ConsentPreview | null>(null);
  const [candidates, setCandidates] = useState<AiCandidateRow[]>([]);
  // Gönderim sicili de **çekirdekten** okunur: `ai_context_items` her çalıştırmada
  // hangi belge sürümünün kaç baytının çıktığını yazıyordu ama okuma yolu yoktu
  // — avukat "bu çalıştırmada ne gitti" sorusunu yanıtlayamıyordu.
  const [contextItems, setContextItems] = useState<AiContextItemRow[]>([]);
  const [contextBusy, setContextBusy] = useState(false);
  // Okuma hatası **kendi satırında** yazılır: düşen okumadan sonra "kayıt yok"
  // demek, olmayan bir olguyu bildirmek olurdu.
  const [contextError, setContextError] = useState<ApiError | null>(null);
  // Segment sicili: kasadan çıkan **metnin** kendisi (ilk 120 karakter).
  // Gönderim sicilinin ikinci katmanı; aday alıntıları önce burada aranır.
  const [segments, setSegments] = useState<TransferSegmentRow[]>([]);
  const [segmentsBusy, setSegmentsBusy] = useState(false);
  const [segmentsError, setSegmentsError] = useState<ApiError | null>(null);
  const [purpose, setPurpose] = useState("ozet");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  // Gönderim ve iptal **kendi** uçuş bayraklarını taşır: biri askıdayken
  // diğeri açık kalır (onay iptali yeni onayı engellemez), ama aynı düğme
  // ikinci kez çekirdeğe yazmaz.
  const [promoteBusy, setPromoteBusy] = useState(false);
  const [revokeBusy, setRevokeBusy] = useState(false);
  // İptal **hedefi**: uçuştaki iptal hangi onaya ait? Yeni gönderim yalnız
  // iptal edilmekte olan onayla kapanır; bu sırada alınmış yeni onay açık kalır.
  const [revokeTarget, setRevokeTarget] = useState<string | null>(null);

  // F11-T05 — aday doğrulama paneli
  const [runId, setRunId] = useState("");
  const [kind, setKind] = useState("claim_candidate");
  const [draft, setDraft] = useState(
    CANDIDATE_KINDS.find((option) => option.value === "claim_candidate")!.template,
  );
  const [report, setReport] = useState<CandidateReport | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // Uçuştaki yanıtlar **hangi bağlama** ait olduğunu taşır: dosya değişimi
  // `caseGen`i, çalıştırma seçimi `runSeq`i artırır. Eski yanıt yeni bağlama
  // yazılmaz (INV-05); yalnız kullanıcının açıkça istediği çekirdek yazısı gider.
  const caseGen = useRef(0);
  const runSeq = useRef(0);
  // Uçuş kilidi **nesil** taşır: kapanış yalnız kendi neslinde kilidi bırakır.
  // Koşulsuz `setBusy(false)` çağırsaydı, devredilen isteğin kapanışı yeni
  // dosyada başlatılan yazmanın kilidini erken açardı.
  const busySeq = useRef(0);
  const promoteSeq = useRef(0);
  const revokeSeq = useRef(0);
  // Önizleme bir **okuma**dır: iki istek yarışırsa sonuç **son** isteğe aittir.
  const previewSeq = useRef(0);
  // Onay envanteri de bir **okuma**dır: dosya değişimi nesli artırır, geç yanıt
  // yeni dosyanın listesine yazılmaz.
  const consentListSeq = useRef(0);
  // Makbuz envanteri de bir **okuma**dır: son başlayan okuma kazanır; geç
  // yanıt yeni dosyanın listesine yazılmaz.
  const receiptListSeq = useRef(0);
  // Gönderim sicili de bir **okuma**dır: `runSeq` seçimi, `contextSeq` okumayı
  // damgalar — geç yanıt başka çalıştırmanın satırı olarak yazılmaz.
  const contextSeq = useRef(0);
  // Segment sicili de bağımsız bir **okuma**dır: kendi nesli vardır.
  const segmentsSeq = useRef(0);
  // Doğrulama raporu **gönderilen** aday türüne aittir; tür değişince geç
  // gelen rapor yeni türün altına yazılmaz.
  const kindRef = useRef(kind);

  function beginWrite(): number {
    busySeq.current += 1;
    setBusy(true);
    return busySeq.current;
  }

  function endWrite(seq: number): void {
    if (busySeq.current === seq) setBusy(false);
  }

  function beginPromote(): number {
    promoteSeq.current += 1;
    setPromoteBusy(true);
    return promoteSeq.current;
  }

  function endPromote(seq: number): void {
    if (promoteSeq.current === seq) setPromoteBusy(false);
  }

  function beginRevoke(target: string): number {
    revokeSeq.current += 1;
    setRevokeBusy(true);
    setRevokeTarget(target);
    return revokeSeq.current;
  }

  function endRevoke(seq: number): void {
    if (revokeSeq.current === seq) {
      setRevokeBusy(false);
      setRevokeTarget(null);
    }
  }

  /**
   * Onay envanteri bir **okuma**dır. Her yeni okuma sayacı artırır ve
   * **son başlayan okuma kazanır** (`consentListSeq`): dosya değişiminde yeni
   * okuma başlar, geç yanıt yeni dosyanın listesine yazılmaz.
   */
  async function loadConsents(forCase: string): Promise<void> {
    if (!forCase) return;
    consentListSeq.current += 1;
    const seq = consentListSeq.current;
    setConsentListBusy(true);
    try {
      const rows = await api.aiConsentList(forCase);
      if (consentListSeq.current !== seq) return;
      setConsents(rows);
    } catch (e) {
      if (consentListSeq.current === seq) setError(toApiError(e));
    } finally {
      if (consentListSeq.current === seq) setConsentListBusy(false);
    }
  }

  /**
   * Aktarım makbuzları bir **okuma**dır (onay envanteriyle aynı desen): her
   * yeni okuma sayacı artırır, **son başlayan kazanır**. Gönderimden sonra da
   * çağrılır — düşen deneme `failed` satırı bırakır, başarılı gönderim
   * `dispatched`; ikisi de ekranda görünür.
   */
  async function loadReceipts(forCase: string): Promise<void> {
    if (!forCase) return;
    receiptListSeq.current += 1;
    const seq = receiptListSeq.current;
    setReceiptListBusy(true);
    try {
      const rows = await api.aiReceiptList(forCase);
      if (receiptListSeq.current !== seq) return;
      setReceipts(rows);
    } catch (e) {
      if (receiptListSeq.current === seq) setError(toApiError(e));
    } finally {
      if (receiptListSeq.current === seq) setReceiptListBusy(false);
    }
  }

  useEffect(() => {
    (async () => {
      try {
        const rows = await api.caseList(false);
        setCases(rows);
        if (rows.length > 0) setCaseId((prev) => prev || rows[0]!.id);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, []);

  useEffect(() => {
    if (!caseId) {
      setConsents([]);
      setConsentListBusy(false);
      setReceipts([]);
      setReceiptListBusy(false);
      return;
    }
    caseGen.current += 1;
    const gen = caseGen.current;
    // Dosya değişimi türetilmiş durumu **devralmaz** (INV-05): onay başka dosyanın
    // belgelerine aitti, önizleme/adaylar/seçili çalıştırma ve doğrulama raporu da
    // önceki dosyanın. Hepsi istekten **önce** silinir; aksi hâlde yeni dosyanın
    // künyesi açıkken eski dosyanın onayıyla çalıştırma başlatılabilir.
    // Uçuş kilitleri **devredilir**: yeni dosyanın düğmeleri eski isteğin yanıtını
    // beklemez (yanıt hiç gelmezse süresiz kilitli kalırdı); devredilen isteğin
    // kapanışı yeni nesilde kilidi bırakmaz (`endWrite`/`endPromote`/`endRevoke`).
    busySeq.current += 1;
    promoteSeq.current += 1;
    revokeSeq.current += 1;
    setBusy(false);
    setPromoteBusy(false);
    setRevokeBusy(false);
    setRevokeTarget(null);
    setOverview(null);
    setConsent(null);
    setMaskText("");
    setConsents([]);
    setConsentListBusy(false);
    setReceipts([]);
    setReceiptListBusy(false);
    setPreview(null);
    setCandidates([]);
    setContextItems([]);
    setContextBusy(false);
    setContextError(null);
    contextSeq.current += 1;
    setSegments([]);
    setSegmentsBusy(false);
    setSegmentsError(null);
    segmentsSeq.current += 1;
    setRunId("");
    setReport(null);
    setNotice(null);
    setError(null);
    void api
      .aiOverview(caseId)
      .then((next) => {
        if (caseGen.current === gen) setOverview(next);
      })
      .catch((e) => {
        if (caseGen.current === gen) setError(toApiError(e));
      });
    // Envanter de çekirdekten okunur: sayfa yeniden açıldığında etkin onaylar
    // görünür ve iptal edilebilir (arayüz yalnız kendi açtığı onayı tutmaz).
    void loadConsents(caseId);
    void loadReceipts(caseId);
  }, [caseId]);

  const provider = overview?.providers.find((p) => p.providerType !== "off") ?? null;
  const offOnly = overview !== null && overview.providers.every((p) => p.providerType === "off");
  // Onay **kendi** sağlayıcısıyla koşar. Eski davranış ilk off-dışı profili
  // seçiyordu: onay başka bir profile bağlıysa çekirdek reddediyor, kullanıcı
  // nedenini göremiyordu. Eşleşme **çekirdeğin** kullandığı ölçüdür
  // (`provider_snapshot` eşitliği).
  const consentProvider =
    consent === null
      ? null
      : overview?.providers.find((p) => p.providerSnapshot === consent.providerSnapshot) ?? null;
  // Künye yüklüyse ve eşleşen profil yoksa onay **yürütülemez**: düğme kapanır
  // ve nedeni yazılır (sessiz `E_AI_CONSENT` yok).
  const consentStale = consent !== null && overview !== null && consentProvider === null;
  // İptali **uygulanmakta** olan onayla yeni gönderim başlamaz (F09-T09):
  // aksi hâlde içerik, iptal çekirdeğe ulaşmadan dışarı çıkabilir. Bu sırada
  // alınmış **yeni** onay (başka kimlik) engellenmez.
  const revokingConsent = consent !== null && revokeBusy && revokeTarget === consent.id;
  const selectedKind = CANDIDATE_KINDS.find((option) => option.value === kind) ?? CANDIDATE_KINDS[0]!;
  const draftError = draftJsonError(draft);

  // Ölçü **çekirdekle aynı**: kod noktası uzunluğu (JS `.length` değil) ve
  // 2–200 sınırı; geçersiz liste gönderilmez (çekirdek de reddeder).
  const maskTerms = maskText
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  const maskInvalid =
    maskTerms.length > 500 ||
    maskTerms.some((term) => [...term].length < 2 || [...term].length > 200);

  async function approve() {
    if (!provider) return;
    const gen = caseGen.current;
    const seq = beginWrite();
    setError(null);
    try {
      // Onay **somut belge kümesine** bağlanır: kullanıcı neyi onayladıysa
      // gönderim yalnız onu kapsar; sonradan belge eklenirse yeniden onay gerekir.
      const docs = await api.documentList(caseId);
      const ids = docs.map((doc) => doc.id);
      const nextPreview = await api.aiConsentPreview(caseId, ids);
      if (caseGen.current !== gen) return;
      setPreview(nextPreview);
      // Onay **sağlayıcı profiline** bağlanır: kimlik dizesini arayüz kurmaz,
      // çekirdek profilden türetir (uç nokta + model parmak izi dahil). Böylece
      // sağlayıcı değişince eski onay kendiliğinden geçersizleşir.
      const nextConsent = await api.aiConsentCreate(caseId, provider.id, purpose, ids);
      if (caseGen.current !== gen) return;
      setConsent(nextConsent);
      // Envanter tazelenir: yeni onay (ve aynı amaçla kapanan önceki) listede görünür.
      void loadConsents(caseId);
    } catch (e) {
      if (caseGen.current === gen) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function start() {
    // Onayın **kendi** sağlayıcısı yoksa hiç istek gitmez: yanlış profille
    // göndermek çekirdekte reddedilir, kullanıcı da boşa bekler.
    if (!consentProvider || !consent) return;
    const gen = caseGen.current;
    const seq = beginWrite();
    setError(null);
    try {
      // Gönderim kapsamı **onaydan** çözülür; burada belge listesi verilmez.
      // Görev türü de onaydan gelir: "İddia çıkarımı" onayı özet olarak koşmaz.
      // Maske **yalnız** girildiyse gönderilir: boş liste göndermek,
      // "maske istendi ama uygulanmadı" izlenimi verirdi.
      const candidate =
        maskTerms.length > 0
          ? await api.aiRunExecute(
              caseId,
              consentProvider.id,
              consent.id,
              purposeTask(consent.purpose),
              maskTerms,
            )
          : await api.aiRunExecute(
              caseId,
              consentProvider.id,
              consent.id,
              purposeTask(consent.purpose),
            );
      const next = await api.aiOverview(caseId);
      if (caseGen.current !== gen) return;
      setOverview(next);
      // Yeni çalıştırma **seçilir**: gönderim sicili (hangi sürüm, kaç bayt)
      // gönderimden hemen sonra ekranda — avukat sormak zorunda kalmaz.
      await focusRun(candidate.aiRunId);
    } catch (e) {
      const failure = toApiError(e);
      if (caseGen.current === gen) setError(failure);
      // Varış yeri değişmişse ekran **aynı olguyu** yazsın: künye ve envanter
      // tazelenir, "Çalıştır" kapanır. Tazeleme düşerse ilk hata görünür kalır.
      if (failure.code === "E_AI_PROVIDER_CHANGED" && caseGen.current === gen) {
        try {
          const next = await api.aiOverview(caseId);
          if (caseGen.current === gen) setOverview(next);
        } catch {
          // birincil hata yukarıda yazıldı
        }
        if (caseGen.current === gen) void loadConsents(caseId);
      }
    } finally {
      // Makbuz envanteri **her** denemeden sonra tazelenir: başarılı gönderim
      // `dispatched`, düşen gönderim `failed` satırı bırakır; ikisi de ekranda
      // görünür (eski davranışta düşen deneme sicilde hiç yoktu).
      if (caseGen.current === gen) void loadReceipts(caseId);
      endWrite(seq);
    }
  }

  async function loadCandidates(id: string) {
    const gen = caseGen.current;
    const seq = runSeq.current;
    try {
      const rows = await api.aiCandidatesList(id);
      if (caseGen.current !== gen || runSeq.current !== seq) return;
      setCandidates(rows);
    } catch (e) {
      if (caseGen.current === gen && runSeq.current === seq) setError(toApiError(e));
    }
  }

  /**
   * Gönderim sicili bir **okuma**dır (onay/makbuz deseniyle aynı): her yeni okuma
   * sayacı artırır, geç yanıt yeni seçimin altına yazılmaz. Liste **istekten
   * önce** temizlenir — eski çalıştırmanın satırı yeni çalıştırmanınki gibi
   * görünmesin.
   */
  async function loadContextItems(forRun: string): Promise<void> {
    if (!forRun) return;
    contextSeq.current += 1;
    const seq = contextSeq.current;
    setContextBusy(true);
    setContextError(null);
    try {
      const rows = await api.aiContextItems(forRun);
      if (contextSeq.current !== seq) return;
      setContextItems(rows);
    } catch (e) {
      if (contextSeq.current !== seq) return;
      // Satırlar **temiz kalır**: düşen okumada eski liste "bu çalıştırmanın
      // içeriği" gibi gösterilmez, "kayıt yok" da denmez.
      setContextItems([]);
      setContextError(toApiError(e));
    } finally {
      if (contextSeq.current === seq) setContextBusy(false);
    }
  }

  /**
   * Segment sicili de bir **okuma**dır: her yeni okuma sayacı artırır, geç
   * yanıt yeni seçimin altına yazılmaz. Düşen okumada liste boşaltılır ama
   * **"sicil yok" denmez** — hata kendi satırında yazılır.
   */
  async function loadSegments(forRun: string): Promise<void> {
    if (!forRun) return;
    segmentsSeq.current += 1;
    const seq = segmentsSeq.current;
    setSegmentsBusy(true);
    setSegmentsError(null);
    try {
      const rows = await api.aiTransferSegments(forRun);
      if (segmentsSeq.current !== seq) return;
      setSegments(rows);
    } catch (e) {
      if (segmentsSeq.current !== seq) return;
      setSegments([]);
      setSegmentsError(toApiError(e));
    } finally {
      if (segmentsSeq.current === seq) setSegmentsBusy(false);
    }
  }

  /**
   * Bir çalıştırmayı **ekranın odağına** alır: adaylar ve gönderim sicili o
   * çalıştırmaya aittir. Gönderimden sonra da çağrılır — avukat "ne gitti"
   * sorusunun yanıtını hemen görür.
   */
  async function focusRun(id: string): Promise<void> {
    runSeq.current += 1;
    const seq = runSeq.current;
    setRunId(id);
    // Önceki çalıştırmanın adayları/sicili seçili çalıştırmanınki gibi
    // okunmasın: liste istekten **önce** boşaltılır, düşerse boş kalır.
    setCandidates([]);
    setContextItems([]);
    setContextError(null);
    setSegments([]);
    setSegmentsError(null);
    await loadCandidates(id);
    if (runSeq.current !== seq) return;
    await loadContextItems(id);
    if (runSeq.current !== seq) return;
    await loadSegments(id);
  }

  async function selectRun(id: string) {
    setReport(null);
    setNotice(null);
    await focusRun(id);
  }

  async function validateDraft(persist: boolean) {
    if (!runId || draftError) return;
    const gen = caseGen.current;
    const seq = runSeq.current;
    const forRun = runId;
    // Rapor **gönderilen** türe aittir: tür değişirse geç gelen rapor yeni
    // türün altına yazılmaz.
    const forKind = kind;
    const fresh = () => caseGen.current === gen && runSeq.current === seq;
    const write = beginWrite();
    setError(null);
    setNotice(null);
    // Düşen doğrulama önceki raporu "güncel" gibi bırakmaz.
    setReport(null);
    try {
      const result = await api.aiCandidateValidate(forRun, kind, draft);
      if (fresh() && kindRef.current === forKind) setReport(result);
      if (persist) {
        // Kayıt kullanıcının açık isteğiydi: bağlam değişse de çekirdeğe gider.
        await api.aiCandidateRecord(forRun, kind, draft);
        // Bildirim **yanıt anındaki** bağlama bağlıdır: kayıt ya da liste
        // beklerken dosya/çalıştırma değiştiyse yeni bağlama yazılmaz.
        if (!fresh()) return;
        await loadCandidates(forRun);
        if (!fresh()) return;
        setNotice(
          result.state === "valid"
            ? t("ai.noticeValid")
            : result.state === "partial"
              ? t("ai.noticePartial")
              : t("ai.noticeInvalid"),
        );
      }
    } catch (e) {
      if (fresh()) setError(toApiError(e));
    } finally {
      endWrite(write);
    }
  }

  async function revalidate(candidateId: string) {
    const gen = caseGen.current;
    const seq = runSeq.current;
    const write = beginWrite();
    setError(null);
    try {
      const refreshed = await api.aiCandidateRevalidate(candidateId);
      setCandidates((rows) =>
        rows.map((row) => (row.id === refreshed.id ? refreshed : row)),
      );
    } catch (e) {
      if (caseGen.current === gen && runSeq.current === seq) setError(toApiError(e));
    } finally {
      endWrite(write);
    }
  }

  async function promote(candidateId: string) {
    const gen = caseGen.current;
    const seq = runSeq.current;
    const write = beginPromote();
    setError(null);
    setNotice(null);
    try {
      await api.aiCandidatePromote(candidateId, caseId);
      // Bildirim, **gönderilen** adayın çalıştırmasına aittir: uçuşta başka
      // çalıştırmaya geçildiyse yeni çalıştırmanın altına yazılmaz.
      if (caseGen.current !== gen || runSeq.current !== seq) return;
      setNotice(t("ai.noticePromoted"));
    } catch (e) {
      if (caseGen.current === gen && runSeq.current === seq) setError(toApiError(e));
    } finally {
      endPromote(write);
    }
  }

  async function showContent() {
    const gen = caseGen.current;
    previewSeq.current += 1;
    const seq = previewSeq.current;
    try {
      const docs = await api.documentList(caseId);
      const next = await api.aiConsentPreview(caseId, docs.map((doc) => doc.id));
      // İki okuma yarışırsa **son** istek kazanır; eskisinin geç yanıtı yeni
      // önizlemeyi ne çizer ne siler.
      if (caseGen.current !== gen || previewSeq.current !== seq) return;
      setPreview(next);
      setError(null);
    } catch (e) {
      // Sessiz yutma yok: istek düştüyse söylenir ve eski önizleme silinir.
      if (caseGen.current !== gen || previewSeq.current !== seq) return;
      setPreview(null);
      setError(toApiError(e));
    }
  }

  async function revokeConsent(target: string) {
    const gen = caseGen.current;
    const write = beginRevoke(target);
    setError(null);
    try {
      await api.aiConsentRevoke(target);
    } catch (e) {
      if (caseGen.current === gen) setError(toApiError(e));
      return;
    } finally {
      endRevoke(write);
    }
    // Geç dönen iptal, bu arada alınmış **yeni** onayı silmez.
    setConsent((cur) => (cur && cur.id === target ? null : cur));
    // Liste çekirdekten tazelenir: satır "iptal edildi" olur (iyimser silme yok).
    // Dosya değiştiyse eski dosyanın listesi yeni dosyaya yazılmaz.
    if (caseGen.current === gen) void loadConsents(caseId);
  }

  return (
    <div className="page" style={{ maxWidth: 1000 }}>
      <div className="page-head">
        <h1>{t("nav.aiReview")}</h1>
        <p className="lead">{t("ai.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      {notice ? <div className="notice ok">{notice}</div> : null}

      <div className="card">
        <div className="field" style={{ maxWidth: 420 }}>
          <label htmlFor="ai-case">{t("common.case")}</label>
          <select id="ai-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
            {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
          </select>
        </div>

        {offOnly ? (
          <div className="notice warn">
            <strong>{t("ai.offTitle")}</strong> {t("ai.offLead")} <code>off</code>.{" "}
            {t("ai.offTail")} <code>E_AI_OFF</code> {t("ai.offEnd")}
          </div>
        ) : null}

        <dl className="kv">
          <dt>{t("ai.provider")}</dt>
          <dd>{provider ? `${provider.adapterId} (${provider.providerType})` : t("ai.providerNone")}</dd>
          {/* Varış yeri yalnız biliniyorsa yazılır: uydurulmuş "bilinmiyor"
              satırı, sağlayıcı yokken bile bir hedef varmış gibi okunurdu. */}
          {provider ? (
            <>
              <dt>{t("ai.destination")}</dt>
              <dd>
                <code>{provider.providerSnapshot}</code>
              </dd>
            </>
          ) : null}
          <dt>{t("ai.coverage")}</dt>
          <dd>
            {overview
              ? t("ai.coverageLine", {
                  documents: overview.coverage.documents,
                  pages: overview.coverage.pages,
                  ocrPages: overview.coverage.ocrPages,
                  emptyPages: overview.coverage.emptyPages,
                  unparsed: overview.coverage.unparsedDocuments,
                })
              : t("common.unknown")}
          </dd>
        </dl>

        <div className="row">
          <div className="field" style={{ maxWidth: 220 }}>
            <label htmlFor="ai-purpose">{t("ai.purpose")}</label>
            <select
              id="ai-purpose"
              className="input"
              value={purpose}
              onChange={(e) => {
                setPurpose(e.target.value);
                // Onay **amaca özgüdür** (F09-T09): amaç değişince eski onay
                // kapsam dışı kalır; yeni gönderim yeniden onay ister.
                setConsent(null);
              }}
            >
              <option value="ozet">{t("ai.purposeSummary")}</option>
              <option value="olay_cikarimi">{t("ai.purposeEvents")}</option>
              <option value="iddia_cikarimi">{t("ai.purposeClaims")}</option>
            </select>
          </div>
          <div className="field" style={{ maxWidth: 320 }}>
            <label htmlFor="ai-mask">{t("ai.maskLabel")}</label>
            <textarea
              id="ai-mask"
              className="input"
              rows={2}
              value={maskText}
              placeholder={t("ai.maskPlaceholder")}
              onChange={(e) => setMaskText(e.target.value)}
            />
            <p className="hint" style={{ margin: 0 }}>
              {maskInvalid
                ? t("ai.maskInvalid")
                : maskTerms.length > 0
                  ? t("ai.maskCount", { count: maskTerms.length })
                  : t("ai.maskHint")}
            </p>
          </div>
          <div className="row" style={{ alignItems: "flex-end", gap: "var(--space-2)" }}>
            <button className="btn" disabled={!provider || busy} onClick={() => void approve()}>
              {t("ai.approve")}
            </button>
            <button className="btn ghost" onClick={() => void showContent()}>
              {t("ai.showContent")}
            </button>
            <button
              className="btn primary"
              disabled={!provider || !consent || busy || revokingConsent || consentStale || maskInvalid}
              onClick={() => void start()}
            >
              {t("ai.run")}
            </button>
          </div>
        </div>

        {preview ? (
          <div className="card" style={{ marginTop: 8 }}>
            <div className="card-head">
              <div>
                <h3>{t("ai.previewTitle")}</h3>
                <p className="card-sub">
                  {t("ai.previewSub", {
                    documents: preview.documents,
                    segments: preview.segments,
                    bytes: formatSize(preview.bytes),
                  })}
                </p>
              </div>
            </div>
            {skippedSummary(preview.skipped) ? (
              <p className="hint" style={{ margin: 0 }}>
                {skippedSummary(preview.skipped)} {t("ai.skippedSuffix")}
              </p>
            ) : null}
            <pre style={{ maxHeight: 140, overflow: "auto", whiteSpace: "pre-wrap", fontSize: 12 }}>
              {preview.sample || t("ai.previewEmpty")}
            </pre>
          </div>
        ) : null}
        {consent ? (
          <>
            <p className="hint" style={{ margin: 0 }}>
              {t("ai.consentPurpose", { purpose: purposeLabel(consent.purpose) })} ·{" "}
              {consentScopeLine(consent.selectedDocumentIds.length, preview)} ·{" "}
              {t("ai.consentTail", {
                id: consent.id.slice(0, 8),
                provider: consent.providerSnapshot,
              })}
              <button
                className="btn ghost"
                style={{ marginLeft: 8 }}
                disabled={revokeBusy}
                onClick={() => void revokeConsent(consent.id)}
              >
                {t("ai.revoke")}
              </button>
            </p>
            {consentStale ? (
              <div className="notice warn" role="status">
                {t("ai.consentStale", { provider: consent.providerSnapshot })}
              </div>
            ) : null}
          </>
        ) : null}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.consentsTitle")}</h2>
            <p className="card-sub">{t("ai.consentsSub")}</p>
          </div>
          <button
            className="btn ghost"
            disabled={!caseId || consentListBusy}
            onClick={() => void loadConsents(caseId)}
          >
            {consentListBusy ? t("ai.consentsLoading") : t("ai.consentsRefresh")}
          </button>
        </div>
        {consents.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {consentListBusy ? t("ai.consentsLoading") : t("ai.consentsEmpty")}
          </p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.consentsColPurpose")}</th>
                <th>{t("ai.consentsColProvider")}</th>
                <th>{t("ai.consentsColDocuments")}</th>
                <th>{t("ai.consentsColApproved")}</th>
                <th>{t("ai.consentsColExpires")}</th>
                <th>{t("ai.consentsColState")}</th>
                <th>{t("ai.consentsColAction")}</th>
              </tr>
            </thead>
            <tbody>
              {consents.map((row) => (
                <tr key={row.id}>
                  <td>{purposeLabel(row.purpose)}</td>
                  <td>
                    <code>{row.providerSnapshot}</code>
                  </td>
                  <td>{row.selectedDocumentIds.length}</td>
                  <td>{formatDateTime(row.approvedAt)}</td>
                  <td>{row.expiresAt ? formatDateTime(row.expiresAt) : t("ai.consentNoExpiry")}</td>
                  <td>
                    <span className={`badge ${consentStateTone(row)}`}>{consentStateLabel(row)}</span>
                  </td>
                  <td>
                    {row.active ? (
                      <button
                        className="btn ghost"
                        disabled={revokeBusy}
                        onClick={() => void revokeConsent(row.id)}
                      >
                        {t("ai.revoke")}
                      </button>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="hint" style={{ margin: 0 }}>
          {t("ai.consentsNote")}
        </p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.runsTitle")}</h2>
            <p className="card-sub">{t("ai.runsSub")}</p>
          </div>
        </div>
        {overview && overview.runs.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.colKind")}</th>
                <th>{t("ai.colState")}</th>
                <th>{t("ai.colStarted")}</th>
                <th>{t("ai.colCandidates")}</th>
              </tr>
            </thead>
            <tbody>
              {overview.runs.map((run) => (
                <tr key={run.id}>
                  <td>{run.taskType}</td>
                  <td><span className={`badge ${run.status === "succeeded" ? "ok" : ""}`}>{run.status}</span></td>
                  <td>{formatDateTime(run.createdAt)}</td>
                  <td>
                    <button className="btn ghost" onClick={() => void selectRun(run.id)}>
                      {runId === run.id ? t("ai.selectedRun") : t("ai.selectRun")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="hint" style={{ margin: 0 }}>{t("ai.runsEmpty")}</p>
        )}
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.contextTitle")}</h2>
            <p className="card-sub">{t("ai.contextSub")}</p>
          </div>
        </div>
        {!runId ? (
          <p className="hint" style={{ margin: 0 }}>{t("ai.contextNoRun")}</p>
        ) : contextError ? (
          // Düşen okuma "kayıt yok" sayılmaz: kendi satırında yazılır.
          <div className="notice danger" role="alert">
            <strong>{t("ai.contextError")}</strong> {contextError.code} — {contextError.message}
          </div>
        ) : contextItems.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {contextBusy ? t("ai.contextLoading") : t("ai.contextEmpty")}
          </p>
        ) : (
          <>
            <table className="table">
              <thead>
                <tr>
                  <th>{t("ai.contextColDocument")}</th>
                  <th>{t("ai.contextColVersion")}</th>
                  <th>{t("ai.contextColReason")}</th>
                  <th>{t("ai.contextColBytes")}</th>
                  <th>{t("ai.contextColSegments")}</th>
                </tr>
              </thead>
              <tbody>
                {contextItems.map((item) => (
                  <tr key={item.id}>
                    <td>
                      {item.documentTitle ?? (
                        <span className="hint">
                          {t("ai.contextMissingDocument", { id: item.documentVersionId })}
                        </span>
                      )}
                    </td>
                    <td>
                      {item.versionNo === null
                        ? t("ai.contextVersionUnknown")
                        : item.versionNo}
                    </td>
                    <td>{item.inclusionReason}</td>
                    <td>{item.bytesSent}</td>
                    <td>
                      {item.segmentCount === null
                        ? t("ai.contextSegmentUnknown")
                        : item.segmentCount}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="hint" style={{ margin: 0 }}>
              {t("ai.contextTotal", {
                documents: contextItems.length,
                bytes: formatSize(
                  contextItems.reduce((sum, item) => sum + item.bytesSent, 0),
                ),
              })}
            </p>
          </>
        )}
        <p className="hint" style={{ margin: 0 }}>{t("ai.contextNote")}</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.transferTitle")}</h2>
            <p className="card-sub">{t("ai.transferSub")}</p>
          </div>
        </div>
        {!runId ? (
          <p className="hint" style={{ margin: 0 }}>{t("ai.transferNoRun")}</p>
        ) : segmentsError ? (
          <div className="notice danger" role="alert">
            <strong>{t("ai.transferError")}</strong> {segmentsError.code} — {segmentsError.message}
          </div>
        ) : segments.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {segmentsBusy ? t("ai.transferLoading") : t("ai.transferEmpty")}
          </p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.transferColIndex")}</th>
                <th>{t("ai.transferColSegment")}</th>
                <th>{t("ai.transferColChars")}</th>
                <th>{t("ai.transferColHash")}</th>
                <th>{t("ai.transferColText")}</th>
              </tr>
            </thead>
            <tbody>
              {segments.map((segment, index) => (
                <tr key={segment.id}>
                  <td>{index + 1}</td>
                  <td>
                    <code title={segment.segmentId}>{segment.segmentId.slice(0, 8)}…</code>
                    {segment.transformKind === "identity" ? null : (
                      <span className="hint" style={{ display: "block" }}>
                        {segment.transformKind === "mask_terms"
                          ? t("ai.transferKindMasked", { count: segment.maskTermCount })
                          : t("ai.transferKindUnknown", { kind: segment.transformKind })}
                      </span>
                    )}
                  </td>
                  <td>{segment.charCount}</td>
                  <td><code title={segment.outboundHash}>{segment.outboundHash.slice(0, 12)}…</code></td>
                  <td style={{ maxWidth: 380 }}>{segment.textHead}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="hint" style={{ margin: 0 }}>{t("ai.transferNote")}</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.receiptsTitle")}</h2>
            <p className="card-sub">{t("ai.receiptsSub")}</p>
          </div>
          <button
            className="btn ghost"
            disabled={!caseId || receiptListBusy}
            onClick={() => void loadReceipts(caseId)}
          >
            {receiptListBusy ? t("ai.receiptsLoading") : t("ai.receiptsRefresh")}
          </button>
        </div>
        {receipts.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {receiptListBusy ? t("ai.receiptsLoading") : t("ai.receiptsEmpty")}
          </p>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.receiptsColTime")}</th>
                <th>{t("ai.receiptsColTask")}</th>
                <th>{t("ai.receiptsColDestination")}</th>
                <th>{t("ai.receiptsColItems")}</th>
                <th>{t("ai.receiptsColBytes")}</th>
                <th>{t("ai.receiptsColHash")}</th>
                <th>{t("ai.receiptsColState")}</th>
              </tr>
            </thead>
            <tbody>
              {receipts.map((row) => (
                <tr key={row.id}>
                  <td>{formatDateTime(row.dispatchedAt)}</td>
                  <td>{row.taskType}</td>
                  <td>
                    <code>{row.destinationLabel}</code>
                  </td>
                  <td>{row.itemCount}</td>
                  <td>{row.byteCount}</td>
                  <td
                    style={{
                      fontFamily: "var(--mono)",
                      fontSize: 11,
                      wordBreak: "break-all",
                    }}
                  >
                    {row.outboundHash}
                  </td>
                  <td>
                    <span className={`badge ${receiptStateTone(row.status)}`}>
                      {receiptStateLabel(row.status)}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="hint" style={{ margin: 0 }}>{t("ai.receiptsNote")}</p>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.validateTitle")}</h2>
            <p className="card-sub">{t("ai.validateSub")}</p>
          </div>
        </div>

        {runId ? (
          <p className="hint" style={{ marginTop: 0 }}>
            {t("ai.currentRun")} <code>{runId.slice(0, 8)}…</code>
          </p>
        ) : (
          <div className="notice warn">{t("ai.noRun")}</div>
        )}

        <div className="row">
          <div className="field" style={{ maxWidth: 260 }}>
            <label htmlFor="ai-kind">{t("ai.kindLabel")}</label>
            <select
              id="ai-kind"
              className="input"
              value={kind}
              onChange={(e) => {
                const next = e.target.value;
                kindRef.current = next;
                setKind(next);
                setReport(null);
                const option = CANDIDATE_KINDS.find((item) => item.value === next);
                if (option) setDraft(option.template);
              }}
            >
              {CANDIDATE_KINDS.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </div>
          <p className="hint" style={{ alignSelf: "flex-end", margin: 0 }}>{selectedKind.hint}</p>
        </div>

        <div className="field">
          <label htmlFor="ai-draft">{t("ai.draftLabel")}</label>
          <textarea
            id="ai-draft"
            className="input"
            style={{ minHeight: 180, fontFamily: "var(--mono)", fontSize: 12 }}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
        </div>

        {draftError ? <p className="hint" style={{ color: "var(--danger)" }}>{draftError}</p> : null}

        <div className="row">
          <button
            className="btn"
            disabled={!runId || busy || draftError !== null}
            onClick={() => void validateDraft(false)}
          >
            {t("ai.validate")}
          </button>
          <button
            className="btn primary"
            disabled={!runId || busy || draftError !== null}
            onClick={() => void validateDraft(true)}
          >
            {t("ai.validateAndSave")}
          </button>
        </div>

        {report ? (
          <div style={{ marginTop: "var(--space-3)" }}>
            <div className="row" style={{ gap: 8, alignItems: "center" }}>
              <span className={`badge ${candidateStateTone(report.state)}`}>
                {candidateStateLabel(report.state)}
              </span>
              <span className={`badge ${quoteStateTone(report.quoteState)}`}>
                {quoteStateLabel(report.quoteState)}
              </span>
              <span className="badge plain">{quoteSummary(report)}</span>
              <span className="badge plain">
                {t(
                  report.outboundState === "recorded"
                    ? "ai.outboundRecorded"
                    : "ai.outboundNotRecorded",
                )}
              </span>
              {errorCount(report.issues) > 0 ? (
                <span className="badge danger">
                  {t("ai.errorBadge", { count: errorCount(report.issues) })}
                </span>
              ) : null}
              {warningCount(report.issues) > 0 ? (
                <span className="badge warn">
                  {t("ai.warningBadge", { count: warningCount(report.issues) })}
                </span>
              ) : null}
            </div>

            {report.quotes.length > 0 ? (
              <table className="table" style={{ marginTop: "var(--space-2)" }}>
                <thead>
                  <tr>
                    <th>#</th>
                    <th>{t("ai.colQuote")}</th>
                    <th>{t("ai.colState")}</th>
                    <th>{t("ai.colPosition")}</th>
                    <th>{t("ai.colOccurrences")}</th>
                    <th>{t("ai.colOutbound")}</th>
                    <th>{t("ai.colText")}</th>
                  </tr>
                </thead>
                <tbody>
                  {report.quotes.map((quote) => (
                    <tr key={`${quote.documentVersionId}-${quote.index}`}>
                      <td>{quote.index + 1}</td>
                      <td style={{ maxWidth: 320 }}>“{quote.quote.slice(0, 120)}”</td>
                      <td>
                        <span className={`badge ${quoteLineTone(quote.state)}`}>
                          {quoteLineStateLabel(quote)}
                        </span>
                      </td>
                      <td>
                        {quotePositionLabel(quote)}
                        {quote.masked && quote.sourceCharStart != null ? (
                          <span className="hint" style={{ display: "block" }}>
                            {t("ai.maskedSourceRange", { start: quote.sourceCharStart })}
                          </span>
                        ) : null}
                      </td>
                      <td>{quote.occurrences}</td>
                      <td>{outboundLabel(quote.outboundState)}</td>
                      <td>
                        {quote.textOrigin ?? t("common.unknown")}
                        {quote.claimedPageIndex !== null && quote.pageIndex !== null
                          && quote.claimedPageIndex !== quote.pageIndex ? (
                          <span className="hint" style={{ display: "block" }}>
                            {t("ai.claimedPage", { page: quote.claimedPageIndex + 1 })}
                          </span>
                        ) : null}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : null}

            {report.issues.length > 0 ? (
              <details style={{ marginTop: "var(--space-2)" }} open>
                <summary>{t("ai.issuesSummary", { count: report.issues.length })}</summary>
                <ul className="hint" style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
                  {report.issues.map((issue, index) => (
                    <li key={`${issue.code}-${issue.field}-${index}`}>
                      <code>{issue.field}</code> · {issue.code} — {issue.detail}
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}

            {report.notes.length > 0 ? (
              <ul className="hint" style={{ margin: "var(--space-2) 0 0", paddingLeft: 18 }}>
                {report.notes.map((note, index) => <li key={index}>{note}</li>)}
              </ul>
            ) : null}

            <p className="hint" style={{ marginBottom: 0 }}>{t("ai.limits")}</p>
          </div>
        ) : null}
      </div>

      {candidates.length > 0 ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("ai.candidatesTitle")}</h2>
              <p className="card-sub">{t("ai.candidatesSub")}</p>
            </div>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.colKind")}</th>
                <th>{t("ai.colSchema")}</th>
                <th>{t("ai.colQuote")}</th>
                <th>{t("ai.colContent")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {candidates.map((candidate) => {
                const blocked = promoteBlockedReason(candidate);
                const issues = parseIssues(candidate.validationErrors);
                return (
                  <tr key={candidate.id}>
                    <td>
                      {candidateKindLabel(candidate.candidateType)}
                      <span className="hint" style={{ display: "block" }}>
                        {candidate.checkedAt
                          ? formatDateTime(candidate.checkedAt)
                          : t("ai.neverChecked")}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${candidateStateTone(candidate.schemaState)}`}>
                        {candidateStateLabel(candidate.schemaState)}
                      </span>
                      <span className={`badge ${candidateStateTone(candidate.validationState)}`} style={{ marginLeft: 4 }}>
                        {t("ai.candidateBadge", {
                          state: candidateStateLabel(candidate.validationState),
                        })}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${quoteStateTone(candidate.quoteState)}`}>
                        {quoteStateLabel(candidate.quoteState)}
                      </span>
                      {candidate.quoteReport.length > 0 ? (
                        <details style={{ marginTop: 4 }}>
                          <summary className="hint">
                            {t("ai.quotesToggle", { count: candidate.quoteReport.length })}
                          </summary>
                          <ul className="hint" style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                            {candidate.quoteReport.map((quote) => (
                              <li key={`${quote.documentVersionId}-${quote.index}`}>
                                <span className={`badge ${quoteLineTone(quote.state)}`}>
                                  {quoteLineStateLabel(quote)}
                                </span>{" "}
                                {quotePositionLabel(quote)} · “{quote.quote.slice(0, 48)}”
                              </li>
                            ))}
                          </ul>
                        </details>
                      ) : null}
                    </td>
                    <td style={{ fontFamily: "var(--mono)", fontSize: 11 }}>
                      {candidate.structuredPayload.slice(0, 80)}
                      {issues.length > 0 ? (
                        <details style={{ marginTop: 4 }}>
                          <summary className="hint">
                            {t("ai.issuesToggle", { count: issues.length })}
                          </summary>
                          <ul className="hint" style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                            {issues.map((issue, index) => (
                              <li key={`${issue.code}-${index}`}>
                                <code>{issue.field}</code> · {issue.code}
                              </li>
                            ))}
                          </ul>
                        </details>
                      ) : null}
                    </td>
                    <td>
                      <div className="row" style={{ gap: 6 }}>
                        <button className="btn ghost" disabled={busy} onClick={() => void revalidate(candidate.id)}>
                          {t("ai.revalidate")}
                        </button>
                        <button
                          className="btn ghost"
                          disabled={busy || promoteBusy || blocked !== null}
                          title={blocked ?? t("ai.promoteTitle")}
                          onClick={() => void promote(candidate.id)}
                        >
                          {t("ai.promote")}
                        </button>
                      </div>
                      {blocked ? <span className="hint">{blocked}</span> : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}
