import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  toApiError,
  type AnchorRow,
  type ApiError,
  type CaseRow,
  type ClaimRelationRow,
  type ClaimRow,
  type ContradictionAssessmentRow,
  type DocumentRow,
  type EvidenceLinkRow,
  type InTextQuoteCheck,
  type InTextQuoteFinding,
  type ReviewDecisionRow,
  type ReviewItemRow,
} from "../lib/api";
import {
  bindHint,
  bindingInput,
  canBind,
  candidateLabel,
  oldVersionNote,
  positionLabel,
  quoteCheckSummary,
  QUOTE_STATE_LABEL,
  quoteStateTone,
  scopeLabel,
  skippedLabel,
  versionsLabel,
} from "../lib/intext";
import { formatTimestamp } from "../lib/format";
import { t } from "../lib/i18n";
import { Parties } from "../components/Parties";
import {
  buildReviewTarget,
  itemText,
  TARGET_KIND_LABEL,
  type ReviewTargetKind,
} from "../lib/reviewTarget";

const CLAIM_KIND: Record<string, string> = {
  allegation: t("review.claimKindAllegation"),
  defense: t("review.claimKindDefense"),
  admission: t("review.claimKindAdmission"),
  court_finding: t("review.claimKindCourtFinding"),
  unknown: t("review.claimKindUnknown"),
};

const RELATION: Record<string, string> = {
  submitted_for: t("review.relationSubmittedFor"),
  supports_candidate: t("review.relationSupportsCandidate"),
  contradicts_candidate: t("review.relationContradictsCandidate"),
  context: t("review.relationContext"),
};

const CLAIM_RELATION: Record<string, string> = {
  response_to: t("review.claimRelationResponseTo"),
  supports: t("review.claimRelationSupports"),
  contradicts: t("review.claimRelationContradicts"),
  context: t("review.claimRelationContext"),
};

const ITEM_KIND: Record<string, string> = {
  claim_candidate: t("review.itemClaim"),
  event_candidate: t("review.itemEvent"),
  note_candidate: t("review.itemNote"),
  task_candidate: t("review.itemTask"),
  contradiction_candidate: t("review.itemContradiction"),
};

const VERDICT: Record<string, string> = {
  conflict: t("review.verdictConflict"),
  explained: t("review.verdictExplained"),
  not_conflict: t("review.verdictNotConflict"),
  needs_more_evidence: t("review.verdictNeedsEvidence"),
};

const ITEM_STATE: Record<string, string> = {
  pending: t("review.statePending"),
  accepted: t("review.stateAccepted"),
  rejected: t("review.stateRejected"),
  edited: t("review.stateEdited"),
  needs_more_evidence: t("review.stateNeedsEvidence"),
  superseded: t("review.stateSuperseded"),
};

/**
 * Karar geçmişindeki karar kodları. Tanınmayan kod **ham** gösterilir: geçmiş
 * kaydı uydurma bir etikete çevrilmez (eski sürüm/elle yazım olabilir).
 */
const DECISION: Record<string, string> = {
  accept: t("review.decisionAccept"),
  reject: t("review.decisionReject"),
  edit: t("review.decisionEdit"),
  request_evidence: t("review.decisionRequestEvidence"),
};

export function Review() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [claims, setClaims] = useState<ClaimRow[]>([]);
  const [selectedClaim, setSelectedClaim] = useState<ClaimRow | null>(null);
  const [links, setLinks] = useState<EvidenceLinkRow[]>([]);
  const [items, setItems] = useState<ReviewItemRow[]>([]);
  const [anchors, setAnchors] = useState<AnchorRow[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [documents, setDocuments] = useState<DocumentRow[]>([]);
  const [quoteText, setQuoteText] = useState("");
  const [quoteScope, setQuoteScope] = useState("");
  const [quoteSource, setQuoteSource] = useState("");
  const [quoteCheck, setQuoteCheck] = useState<InTextQuoteCheck | null>(null);
  const [quoteBindings, setQuoteBindings] = useState<Record<number, string>>({});
  // Uçuştaki "Kaynağa bağla" **yazıldı** ama rozete bağlanamadı: alıntı girdisi
  // bu sırada değişti. Yazıldığı söylenmezse kullanıcı göremez ve yeniden
  // tıklayıp **ikinci** çapa üretir (çekirdekte iki kanıt, biri görünmez).
  const [quoteBindNotice, setQuoteBindNotice] = useState<string | null>(null);
  const [quoteBusy, setQuoteBusy] = useState(false);

  const [claimText, setClaimText] = useState("");
  const [claimKind, setClaimKind] = useState("");
  const [linkAnchor, setLinkAnchor] = useState("");
  const [linkRelation, setLinkRelation] = useState("");
  const [linkRationale, setLinkRationale] = useState("");
  const [relations, setRelations] = useState<ClaimRelationRow[]>([]);
  const [relationTarget, setRelationTarget] = useState("");
  const [relationKind, setRelationKind] = useState("");
  const [relationNote, setRelationNote] = useState("");
  const [draftKind, setDraftKind] = useState("note_candidate");
  const [draftAnchor, setDraftAnchor] = useState("");
  const [draftAnchor2, setDraftAnchor2] = useState("");
  const [draftText, setDraftText] = useState("");
  const [assessments, setAssessments] = useState<Record<string, ContradictionAssessmentRow[]>>({});
  const [assessmentVerdicts, setAssessmentVerdicts] = useState<Record<string, string>>({});
  const [assessmentNotes, setAssessmentNotes] = useState<Record<string, string>>({});
  const [editJson, setEditJson] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [supersedeTargets, setSupersedeTargets] = useState<Record<string, string>>({});
  // F08-T08: kabul, seçilen hedef kayıtla birlikte atomik yazılır.
  const [targetKinds, setTargetKinds] = useState<Record<string, ReviewTargetKind>>({});
  const [targetClaimKinds, setTargetClaimKinds] = useState<Record<string, string>>({});
  const [outboxPending, setOutboxPending] = useState(0);
  // Teslim edilmemiş **alan olayları** (bildirim değil): ayrı sayılır, ayrı
  // cümleyle yazılır. İkisini tek sayıda toplamak "bildirim bekliyor" yalanı
  // üretiyordu (olaylar hiçbir zaman bildirim olmayacak).
  const [outboxDomain, setOutboxDomain] = useState(0);
  // Karar geçmişi (F08-T04): kalem başına **istek üzerine** okunur — liste
  // açılışında her kalem için ayrı okuma yapılmaz.
  const [historyOpenId, setHistoryOpenId] = useState<string | null>(null);
  const [historyRows, setHistoryRows] = useState<Record<string, ReviewDecisionRow[]>>({});
  const [historyErrors, setHistoryErrors] = useState<Record<string, ApiError>>({});
  const [historyBusyId, setHistoryBusyId] = useState<string | null>(null);

  // Uçuştaki yanıtların kapısı: yanıt yalnız **hâlâ** aynı dosya, aynı iddia ve
  // aynı alıntı hükmü için yazılır (INV-05). Değerler render dışında okunduğu
  // için ref'te tutulur; sayaçlar aynı dosya için yarışan istekleri sıraya bağlar.
  const caseRef = useRef(caseId);
  const claimRef = useRef<ClaimRow | null>(selectedClaim);
  const quoteRef = useRef<InTextQuoteCheck | null>(quoteCheck);
  const editingRef = useRef<string | null>(editingId);
  const loadSeq = useRef(0);
  const linksSeq = useRef(0);
  const linksClaimRef = useRef<string | null>(null);
  const quoteSeq = useRef(0);
  // Alıntı **denetimi** nesli: metin, kapsam ya da kaynak iddia değişince
  // uçuştaki denetim geçersizdir ve yanıtı yazılmaz. `quoteSeq` yazma kilidini
  // (denetim + kaynağa bağlama) yönetir; ikisini tek sayaçta tutmak bir
  // bağlamanın uçuştaki denetimi sessizce düşürmesine yol açardı.
  const quoteCheckSeq = useRef(0);
  // Tek yazma kilidi: her yazma kendi **neslini** taşır. Dosya değişimi kilidi
  // devreder; devredilen isteğin kapanışı, yeni dosyada başlatılan yazmanın
  // kilidini erken açmaz.
  const busySeq = useRef(0);
  // Açık geçmiş paneli ve yarışan okumalar: geç yanıt yalnız **hâlâ** açık olan
  // kalemin panelinde görünür. Ref, `setState`den **önce** yazılır (yanıt
  // microtask'ta dönerse effect beklenmez).
  const historyOpenRef = useRef<string | null>(null);
  const historySeq = useRef(0);

  function beginWrite(): number {
    const seq = ++busySeq.current;
    setBusy(true);
    return seq;
  }

  function endWrite(seq: number) {
    if (busySeq.current === seq) setBusy(false);
  }

  useEffect(() => {
    caseRef.current = caseId;
  }, [caseId]);

  useEffect(() => {
    claimRef.current = selectedClaim;
  }, [selectedClaim]);

  useEffect(() => {
    quoteRef.current = quoteCheck;
  }, [quoteCheck]);

  useEffect(() => {
    editingRef.current = editingId;
  }, [editingId]);

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

  const load = useCallback(async (id: string) => {
    if (!id) return;
    const seq = ++loadSeq.current;
    // Yanıt yalnız hâlâ aynı dosya için ve **en son** istekse yazılır: hızlı
    // dosya değişiminde eski dosyanın geç yanıtı yeni dosyanın listesini ezmez.
    const fresh = () => caseRef.current === id && loadSeq.current === seq;
    // Bayat tazeleme (başka dosyanın geç okuması) yeni dosyanın kırmızı
    // bandını silmez: temizlik yalnız hâlâ geçerli istekte yapılır.
    if (fresh()) setError(null);
    try {
      const [claimRows, itemRows, anchorRows, documentRows] = await Promise.all([
        api.claimList(id),
        api.reviewItemList(id),
        api.anchorList(id),
        api.documentList(id),
      ]);
      if (!fresh()) return;
      setClaims(claimRows);
      setItems(itemRows);
      setAnchors(anchorRows);
      setDocuments(documentRows);
      const contradictionIds = itemRows
        .filter((i) => i.itemKind === "contradiction_candidate")
        .map((i) => i.id);
      const [outboxRows, pairs] = await Promise.all([
        api.reviewOutboxPending(id),
        Promise.all(
          contradictionIds.map(
            async (itemId) => [itemId, await api.contradictionAssessments(itemId)] as const,
          ),
        ),
      ]);
      if (!fresh()) return;
      // Sınıf ayrımı **okunan satırdan** yapılır: `task.reminder_due` bildirim,
      // diğerleri alan olayıdır. Arayüz tür listesini kendi uydurmaz; çekirdek
      // `event_type`ı zaten taşır.
      const notifications = outboxRows.filter((r) => r.eventType === "task.reminder_due").length;
      setOutboxPending(notifications);
      setOutboxDomain(outboxRows.length - notifications);
      setAssessments(Object.fromEntries(pairs));
      setSelectedClaim((prev) => (prev ? claimRows.find((c) => c.id === prev.id) ?? null : null));
    } catch (e) {
      if (!fresh()) return;
      setError(toApiError(e));
      // Okunamayan dosyanın yerine önceki dosyanın verisi gösterilmez: liste
      // "yeni dosyanın verisi" gibi kalamaz.
      setClaims([]);
      setItems([]);
      setAnchors([]);
      setDocuments([]);
      setOutboxPending(0);
      setOutboxDomain(0);
      setAssessments({});
      setSelectedClaim(null);
    }
  }, []);

  useEffect(() => {
    void load(caseId);
  }, [caseId, load]);

  /**
   * Karar geçmişini okur (F08-T04). Kapı: yanıt yalnız **aynı dosya** ve **en son**
   * istek için yazılır; dosya değişiminde ya da panel kapanıp yeniden açıldığında
   * (yeni istek) geç yanıt ekrana düşmez.
   */
  async function loadHistory(itemId: string) {
    const asked = caseId;
    const seq = ++historySeq.current;
    setHistoryBusyId(itemId);
    // Yeni denemede önceki hata satırı kalkar; liste tazelenene kadar korunur.
    setHistoryErrors((prev) => {
      if (!(itemId in prev)) return prev;
      const next = { ...prev };
      delete next[itemId];
      return next;
    });
    try {
      const rows = await api.reviewHistory(itemId);
      if (caseRef.current !== asked || historySeq.current !== seq) return;
      setHistoryRows((prev) => ({ ...prev, [itemId]: rows }));
    } catch (e) {
      if (caseRef.current !== asked || historySeq.current !== seq) return;
      // Okunamayan geçmiş "kayıt yok" diye gösterilmez: bayat liste düşer,
      // hata kendi satırında yazılır.
      setHistoryErrors((prev) => ({ ...prev, [itemId]: toApiError(e) }));
      setHistoryRows((prev) => {
        const next = { ...prev };
        delete next[itemId];
        return next;
      });
    } finally {
      if (historySeq.current === seq) setHistoryBusyId((cur) => (cur === itemId ? null : cur));
    }
  }

  function closeHistory() {
    historyOpenRef.current = null;
    setHistoryOpenId(null);
  }

  function toggleHistory(item: ReviewItemRow) {
    if (historyOpenRef.current === item.id) {
      closeHistory();
      return;
    }
    historyOpenRef.current = item.id;
    setHistoryOpenId(item.id);
    // Açılışta önceki okumanın satırları gösterilmez: panel "yükleniyor" ile
    // açılır, bayat liste yeni okuma gibi sunulmaz.
    setHistoryRows((prev) => {
      if (!(item.id in prev)) return prev;
      const next = { ...prev };
      delete next[item.id];
      return next;
    });
    void loadHistory(item.id);
  }

  // Dosya değişince önceki dosyanın alıntı **hükmü** ve **belge kapsamı**
  // temizlenir: bayat sonuç ekranda kalmaz ve kapsam yeni dosyaya taşınırsa
  // denetim başka dosyanın belgesine daraltılmış gitmez. Yazılan alıntı
  // metni kullanıcının girdisidir; silinmez, yeni dosyada açıkça çalıştırılır.
  // Kaynak seçimleri birer **çapa kimliğidir**: taşınırsa aday/bağ yanlış
  // dosyanın çapasına yazılır (INV-05), bu yüzden sıfırlanır. Açık düzenleyici
  // de önceki dosyanın öğesine aittir; metniyle birlikte kapanır.
  useEffect(() => {
    // Kilit devri: uçuştaki yazma **eski** dosyaya aittir. Yeni dosyanın
    // düğmeleri eski yanıtı beklemez (yanıt hiç gelmezse süresiz kapalı
    // kalırdı); nesil artırıldığı için devredilen isteğin kapanışı da yeni
    // yazmanın kilidini açmaz.
    busySeq.current += 1;
    setBusy(false);
    // Karar geçmişi paneli önceki dosyanın kalemine aittir: açık kimlik, liste,
    // hata ve uçuştaki okuma kimliği birlikte düşer (geç yanıt yazılmaz).
    historySeq.current += 1;
    historyOpenRef.current = null;
    setHistoryOpenId(null);
    setHistoryRows({});
    setHistoryErrors({});
    setHistoryBusyId(null);
    setQuoteCheck(null);
    setQuoteBindings({});
    setQuoteBindNotice(null);
    setQuoteSource("");
    setQuoteScope("");
    setDraftAnchor("");
    setDraftAnchor2("");
    setLinkAnchor("");
    setEditingId(null);
    setEditJson("");
  }, [caseId]);

  useEffect(() => {
    const seq = ++linksSeq.current;
    const claimId = selectedClaim?.id ?? null;
    // Başka iddiaya geçişte önceki iddianın bağları **hemen** kalkar; aynı
    // iddianın tazelenmesinde liste korunur (titreme yok).
    if (linksClaimRef.current !== claimId) {
      linksClaimRef.current = claimId;
      setLinks([]);
      setRelations([]);
    }
    if (!selectedClaim) return;
    void Promise.all([
      api.evidenceLinkList(selectedClaim.id),
      api.claimRelationList(selectedClaim.id),
    ])
      .then(([linkRows, relationRows]) => {
        // Geç gelen yanıt, o sırada seçilmiş **başka** iddianın panelini ezmez.
        if (linksSeq.current !== seq) return;
        setLinks(linkRows);
        setRelations(relationRows);
      })
      .catch((e) => {
        if (linksSeq.current !== seq) return;
        setError(toApiError(e));
      });
  }, [selectedClaim]);

  /**
   * Alıntı girdisi değişti: ekrandaki hüküm artık bu metne ait değil.
   *
   * Yalnız `setQuoteCheck(null)` yetmez — **uçuştaki** denetimin yanıtı da
   * düşürülmeli, yoksa eski metnin hükmü yeni metnin altında belirir ve
   * "Kaynağa bağla" o eski alıntının çapasını bağlar (INV-05: kanıt yanlış
   * metne işaret eder). Nesil burada artırılır; `runQuoteCheck` yanıt yazarken
   * kendi neslini karşılaştırır.
   */
  function invalidateQuoteCheck() {
    quoteCheckSeq.current += 1;
    setQuoteCheck(null);
    setQuoteBindings({});
    // `quoteBindNotice` **bilerek** burada silinmez: o cümle geçmişte kalmış
    // bir yazmayı anlatır (çapa çekirdekte), sonraki tuş vuruşuyla yok olursa
    // kullanıcı yazıldığını bir daha göremez ve ikinci çapa üretir.
  }

  async function runQuoteCheck() {
    if (!caseId || quoteText.trim().length === 0) return;
    const asked = caseId;
    const seq = ++quoteSeq.current;
    // Denetimin **girdisi** de nesil taşır: yanıt yalnız metin/kapsam hâlâ
    // gönderilen değerse yazılır. Dosya kapısı (`caseRef`) bunu kapsamaz —
    // aynı dosyada metin değişmiş olabilir.
    const checkSeq = ++quoteCheckSeq.current;
    setError(null);
    // Yeni denetim yeni hüküm demek: önceki bağlamanın bildirimi düşer.
    setQuoteBindNotice(null);
    setQuoteBusy(true);
    try {
      const report = await api.inTextQuoteCheck(asked, {
        text: quoteText,
        documentId: quoteScope === "" ? null : quoteScope,
      });
      // Dosya ya da denetlenen metin/kapsam değiştiyse hüküm ekrandakine ait değildir.
      if (caseRef.current !== asked || quoteCheckSeq.current !== checkSeq) return;
      setQuoteCheck(report);
      setQuoteBindings({});
    } catch (e) {
      if (caseRef.current !== asked || quoteCheckSeq.current !== checkSeq) return;
      setError(toApiError(e));
    } finally {
      if (quoteSeq.current === seq) setQuoteBusy(false);
    }
  }

  async function bindQuote(finding: InTextQuoteFinding) {
    const input = bindingInput(caseId, finding);
    if (!input) return;
    const asked = caseId;
    const captured = quoteCheck;
    const seq = ++quoteSeq.current;
    setError(null);
    setQuoteBusy(true);
    try {
      const anchor = await api.anchorCreate(input);
      // Hedef dosya değiştiyse bu bağ ekrandaki dosyaya ait değildir: hiçbir
      // liste yazılmaz (öteki yazma yollarıyla aynı kural).
      if (caseRef.current !== asked) return;
      // Çapa **çekirdekte yazıldı** ve dosyaya aittir: kaynak listesi onu
      // göstermek zorunda. Bu satır eskiden hüküm kapısının arkasındaydı →
      // girdi uçuşta değişince çapa yazılıyor, listede görünmüyordu; kullanıcı
      // yazıldığını göremeyip yeniden tıklıyor ve **ikinci** çapa üretiyordu.
      setAnchors((prev) => (prev.some((row) => row.id === anchor.id) ? prev : [anchor, ...prev]));
      if (quoteRef.current === captured) {
        setQuoteBindings((prev) => ({ ...prev, [finding.index]: anchor.id }));
        setQuoteBindNotice(null);
      } else {
        // Hüküm artık ekrandaki alıntı değil: rozet **yanlış alıntıya**
        // bağlanmaz (INV-05). Ama yazıldığı söylenir — sessiz başarı da yalan.
        setQuoteBindNotice(anchor.id);
      }
    } catch (e) {
      if (caseRef.current !== asked) return;
      setError(toApiError(e));
    } finally {
      if (quoteSeq.current === seq) setQuoteBusy(false);
    }
  }

  async function addClaim() {
    const text = claimText;
    const kind = claimKind;
    if (!caseId || text.trim().length === 0 || kind === "") return;
    const asked = caseId;
    const seq = beginWrite();
    try {
      await api.claimCreate(asked, {
        claimantActorId: null,
        claimText: text.trim(),
        claimKind: kind,
        sourceAnchorId: null,
      });
      // Girdi yalnız hâlâ aynı dosyada ve kullanıcı değiştirmediyse temizlenir:
      // uçuşta yazılan yeni metin sessizce silinmez.
      if (caseRef.current === asked) {
        setClaimText((cur) => (cur === text ? "" : cur));
        setClaimKind((cur) => (cur === kind ? "" : cur));
      }
      await load(asked);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function addLink() {
    if (!selectedClaim || !linkAnchor || linkRelation === "") return;
    const asked = caseId;
    const claimId = selectedClaim.id;
    const anchorId = linkAnchor;
    const rationale = linkRationale;
    const relation = linkRelation;
    setError(null);
    const seq = beginWrite();
    try {
      await api.evidenceLinkCreate(asked, {
        claimId,
        sourceAnchorId: anchorId,
        relation,
        rationale: rationale.trim(),
      });
      const rows = await api.evidenceLinkList(claimId);
      // Başka iddiaya/dosyaya geçildiyse eski iddianın bağı yeni panelin bağı
      // değildir.
      if (claimRef.current?.id !== claimId || caseRef.current !== asked) return;
      setLinkRationale((cur) => (cur === rationale ? "" : cur));
      setLinks(rows);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function addClaimRelation() {
    if (!selectedClaim || relationTarget === "" || relationKind === "") return;
    const asked = caseId;
    const claimId = selectedClaim.id;
    const target = relationTarget;
    const kind = relationKind;
    const note = relationNote;
    const seq = beginWrite();
    try {
      await api.claimRelationCreate(asked, claimId, {
        relatedClaimId: target,
        relation: kind,
        rationale: note.trim(),
      });
      const rows = await api.claimRelationList(claimId);
      if (claimRef.current?.id === claimId && caseRef.current === asked) {
        setRelationTarget((cur) => (cur === target ? "" : cur));
        setRelationKind((cur) => (cur === kind ? "" : cur));
        setRelationNote((cur) => (cur === note ? "" : cur));
        setRelations(rows);
      }
      await load(asked);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function addCandidate() {
    const anchorIds =
      draftKind === "contradiction_candidate"
        ? [draftAnchor, draftAnchor2].filter((id) => id !== "")
        : [draftAnchor];
    if (anchorIds.length === 0 || draftText.trim().length === 0) return;
    if (draftKind === "contradiction_candidate" && (anchorIds.length !== 2 || draftAnchor === draftAnchor2)) {
      setError({
        code: "E_FORM",
        message: t("review.errContradictionSources"),
        retryable: false,
      });
      return;
    }
    const asked = caseId;
    const seq = beginWrite();
    try {
      await api.reviewItemCreate(asked, {
        itemKind: draftKind,
        payload: { text: draftText.trim() },
        sourceAnchorIds: anchorIds,
        aiRunId: null,
      });
      // Uçuşta dosya değiştiyse ya da kullanıcı taslağı değiştirdiyse yeni
      // yazılan metin silinmez.
      if (caseRef.current === asked) {
        setDraftText((cur) => (cur === draftText ? "" : cur));
        setDraftAnchor2((cur) => (cur === draftAnchor2 ? "" : cur));
      }
      await load(asked);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function assess(item: ReviewItemRow) {
    const verdict = assessmentVerdicts[item.id];
    if (!verdict) return;
    const asked = caseId;
    const note = assessmentNotes[item.id] ?? "";
    const seq = beginWrite();
    setError(null);
    try {
      await api.contradictionAssess(asked, item.id, verdict, note.trim(), "local-user");
      const rows = await api.contradictionAssessments(item.id);
      setAssessments((prev) => ({ ...prev, [item.id]: rows }));
      // Uçuşta seçilen **yeni** hüküm/gerekçe silinmez: alan yalnız gönderilen
      // değer hâlâ yazılıysa temizlenir.
      setAssessmentVerdicts((prev) =>
        prev[item.id] === verdict ? { ...prev, [item.id]: "" } : prev,
      );
      setAssessmentNotes((prev) =>
        (prev[item.id] ?? "") === note ? { ...prev, [item.id]: "" } : prev,
      );
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  async function decide(item: ReviewItemRow, decision: "accept" | "reject" | "request_evidence" | "edit") {
    const asked = caseId;
    const seq = beginWrite();
    try {
      let edited: unknown = null;
      if (decision === "edit") {
        try {
          edited = JSON.parse(editJson);
        } catch {
          setError({ code: "E_JSON", message: t("review.errEditJson"), retryable: false });
          return;
        }
      }
      await api.reviewDecide(item.id, decision, decision === "edit" ? t("review.auditUserEdit") : "", edited ?? undefined);
      // Düzenleyici yalnız **hâlâ bu** öğeyi gösteriyorsa kapanır: bu sırada
      // başka öğe için açılan düzenleyici ve yazılan metin korunur.
      if (editingRef.current === item.id) {
        setEditingId(null);
        setEditJson("");
      }
      await load(asked);
      // Paneli açık kalemin geçmişi yeni kararla **tazelenir**: ekranda eski
      // liste kalmaz.
      if (historyOpenRef.current === item.id) void loadHistory(item.id);
    } catch (e) {
      // Uçuşta dosya değiştiyse eski dosyanın hatası yenisinin altında yazılmaz.
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  /**
   * F08-T08: kabul, seçilen hedef kayıtla birlikte tek işlemde yazılır.
   * Kayıt türü (iddiada ayrıca iddia türü) seçilmeden buton açılmaz.
   */
  async function commit(item: ReviewItemRow) {
    const kind = targetKinds[item.id] ?? "";
    const claimKind = targetClaimKinds[item.id] ?? "";
    const target = buildReviewTarget(kind, itemText(item.payload), claimKind);
    if (!target) {
      setError({
        code: "E_FORM",
        message: t("review.errTargetKind"),
        retryable: false,
      });
      return;
    }
    const asked = caseId;
    const seq = beginWrite();
    try {
      await api.reviewCommit(item.id, "accept", target, t("review.auditUserApproval"));
      // Uçuşta değiştirilen hedef seçimi silinmez: yalnız **gönderilen** tür
      // hâlâ seçiliyse temizlenir.
      setTargetKinds((prev) => (prev[item.id] === kind ? { ...prev, [item.id]: "" } : prev));
      setTargetClaimKinds((prev) =>
        (prev[item.id] ?? "") === claimKind ? { ...prev, [item.id]: "" } : prev,
      );
      await load(asked);
      if (historyOpenRef.current === item.id) void loadHistory(item.id);
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  const pending = items.filter((i) => i.state === "pending" || i.state === "needs_more_evidence");
  const decided = items.filter((i) => !(i.state === "pending" || i.state === "needs_more_evidence"));
  // F08-T09: hedef kaydı üretilmiş öğe "yerine geç" listesine girmez; onaylı
  // kayıt sessizce değiştirilemez (backend de reddeder).
  const replaceable = decided.filter(
    (i) => (i.state === "accepted" || i.state === "edited") && !i.targetEntityId,
  );

  async function supersede(item: ReviewItemRow, oldItemId: string) {
    if (!oldItemId) return;
    const asked = caseId;
    const seq = beginWrite();
    try {
      await api.reviewSupersede(oldItemId, item.id);
      // Uçuşta seçilen yeni hedef kayıt silinmez.
      setSupersedeTargets((prev) =>
        prev[item.id] === oldItemId ? { ...prev, [item.id]: "" } : prev,
      );
      await load(asked);
      // Yerine geçilen kalemin paneli açıksa geçmişi de tazelenir.
      const openId = historyOpenRef.current;
      if (openId !== null && (openId === oldItemId || openId === item.id)) {
        void loadHistory(openId);
      }
    } catch (e) {
      if (caseRef.current === asked) setError(toApiError(e));
    } finally {
      endWrite(seq);
    }
  }

  /** Karar geçmişi aç/kapa düğmesi (kimlik `hist-<kalem>`). */
  function historyToggle(item: ReviewItemRow) {
    const open = historyOpenId === item.id;
    return (
      <button
        id={`hist-${item.id}`}
        className="btn ghost"
        aria-expanded={open}
        onClick={() => toggleHistory(item)}
      >
        {open ? t("review.historyHide") : t("review.historyShow")}
      </button>
    );
  }

  /**
   * Açık geçmiş paneli. Dürüstlük kapıları: düşen okuma **"kayıt yok" demez**
   * (kendi hata satırını yazar), tanınmayan karar kodu **ham** gösterilir,
   * çözülemeyen düzenlenmiş içerik "yok" gibi gizlenmez.
   */
  function historyPanel(item: ReviewItemRow) {
    if (historyOpenId !== item.id) return null;
    const rows = historyRows[item.id];
    const failure = historyErrors[item.id];
    return (
      <div
        id={`hist-panel-${item.id}`}
        style={{ display: "flex", flexDirection: "column", gap: 6, paddingTop: 4 }}
      >
        {historyBusyId === item.id ? (
          <p className="hint" style={{ margin: 0 }}>{t("review.historyLoading")}</p>
        ) : null}
        {failure ? (
          <div className="notice danger" role="alert">
            {t("review.historyError", { code: failure.code })}
          </div>
        ) : rows === undefined ? null : rows.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>{t("review.historyEmpty")}</p>
        ) : (
          rows.map((decision) => (
            <div key={decision.id} className="card tight" style={{ background: "var(--bg)", margin: 0 }}>
              <div className="row" style={{ justifyContent: "space-between" }}>
                <span className="badge plain">{DECISION[decision.decision] ?? decision.decision}</span>
                <span className="muted small">{formatTimestamp(decision.decidedAt)}</span>
              </div>
              <span className="small">
                {t("review.historyReviewer", { name: decision.reviewerLabel })}
              </span>
              {decision.rationale ? (
                <p className="small" style={{ margin: 0 }}>
                  {t("review.rationaleDash", { text: decision.rationale })}
                </p>
              ) : null}
              {decision.editedPayloadInvalid ? (
                <p className="hint error" style={{ margin: 0 }}>{t("review.historyEditedInvalid")}</p>
              ) : decision.editedPayload === null || decision.editedPayload === undefined ? null : (
                <>
                  <span className="muted small">{t("review.historyEdited")}</span>
                  <pre
                    style={{
                      margin: 0,
                      maxHeight: 120,
                      overflow: "auto",
                      whiteSpace: "pre-wrap",
                      fontFamily: "var(--mono)",
                      fontSize: 12,
                    }}
                  >
                    {JSON.stringify(decision.editedPayload, null, 2)}
                  </pre>
                </>
              )}
            </div>
          ))
        )}
      </div>
    );
  }

  return (
    <div className="page" style={{ maxWidth: 1020 }}>
      <div className="page-head">
        <h1>{t("nav.review")}</h1>
        <p className="lead">{t("review.lead")}</p>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
          {error.code === "E_ROW_VERSION" ? (
            <button
              className="btn ghost"
              style={{ marginLeft: 8 }}
              onClick={() => {
                setError(null);
                void load(caseId);
              }}
            >
              {t("review.reloadCurrent")}
            </button>
          ) : null}
        </div>
      ) : null}

      <div className="card tight">
        <div className="row">
          <div className="field" style={{ flex: "1 1 260px" }}>
            <label htmlFor="rv-case">{t("common.case")}</label>
            <select id="rv-case" className="input" value={caseId} onChange={(e) => setCaseId(e.target.value)}>
              {cases.length === 0 ? <option value="">{t("common.noCase")}</option> : null}
              {cases.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}
            </select>
          </div>
          <div className="field" style={{ flex: "2 1 300px" }}>
            <label htmlFor="rv-claim">{t("review.newClaimLabel")}</label>
            <input
              id="rv-claim"
              className="input"
              value={claimText}
              onChange={(e) => setClaimText(e.target.value)}
              placeholder={t("review.claimPlaceholder")}
              onKeyDown={(e) => e.key === "Enter" && void addClaim()}
            />
          </div>
          <div className="field" style={{ flex: "0 1 170px" }}>
            <label htmlFor="rv-kind">{t("review.kindLabel")}</label>
            <select id="rv-kind" className="input" value={claimKind} onChange={(e) => setClaimKind(e.target.value)}>
              <option value="">{t("review.kindPlaceholder")}</option>
              {Object.entries(CLAIM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </div>
          <button className="btn primary" disabled={busy || claimKind === "" || claimText.trim().length === 0} onClick={() => void addClaim()}>{t("review.addAction")}</button>
        </div>
      </div>

      <Parties caseId={caseId} />

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-4)" }}>
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("review.claimsTitle")}</h2>
              <p className="card-sub">{t("review.claimsCount", { count: claims.length })}</p>
            </div>
          </div>
          {claims.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>{t("review.claimsEmpty")}</p></div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {claims.map((claim) => (
                <button
                  key={claim.id}
                  onClick={() => setSelectedClaim(claim)}
                  style={{
                    textAlign: "left",
                    padding: "8px 10px",
                    borderRadius: "var(--radius)",
                    border: "1px solid var(--line)",
                    background: selectedClaim?.id === claim.id ? "var(--surface)" : "var(--bg)",
                    cursor: "pointer",
                  }}
                >
                  <span style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
                    <strong style={{ fontWeight: 500 }}>{claim.claimText}</strong>
                    <span className="badge plain">{CLAIM_KIND[claim.claimKind] ?? claim.claimKind}</span>
                  </span>
                  <span className="muted small">{formatTimestamp(claim.createdAt)}</span>
                </button>
              ))}
            </div>
          )}

          {selectedClaim ? (
            <div className="card tight" style={{ background: "var(--surface)" }}>
              <h2>{t("review.evidenceTitle")}</h2>
              {links.length === 0 ? (
                <p className="hint" style={{ margin: 0 }}>{t("review.evidenceEmpty")}</p>
              ) : (
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {links.map((link) => (
                    <li key={link.id} className="small">
                      {RELATION[link.relation] ?? link.relation} ·{" "}
                      {anchors.find((a) => a.id === link.sourceAnchorId)?.exactQuote.slice(0, 60) ?? t("review.quoteFallback")}
                    </li>
                  ))}
                </ul>
              )}
              <div className="field">
                <label htmlFor="rv-anchor">{t("review.anchorLabel")}</label>
                <select id="rv-anchor" className="input" value={linkAnchor} onChange={(e) => setLinkAnchor(e.target.value)}>
                  <option value="">{t("common.select")}</option>
                  {anchors.map((a) => (
                    <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 70)}”</option>
                  ))}
                </select>
              </div>
              <div className="row">
                <div className="field">
                  <label htmlFor="rv-rel">{t("review.relationLabel")}</label>
                  <select id="rv-rel" className="input" value={linkRelation} onChange={(e) => setLinkRelation(e.target.value)}>
                    <option value="">{t("review.relationPlaceholder")}</option>
                    {Object.entries(RELATION).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 200px" }}>
                  <label htmlFor="rv-rat">{t("review.rationaleLabel")}</label>
                  <input id="rv-rat" className="input" value={linkRationale} onChange={(e) => setLinkRationale(e.target.value)} />
                </div>
                <button className="btn" disabled={busy || !linkAnchor || linkRelation === ""} onClick={() => void addLink()}>{t("review.bindAction")}</button>
              </div>

              <h2 style={{ marginTop: 12 }}>{t("review.claimRelationsTitle")}</h2>
              {relations.length === 0 ? (
                <p className="hint" style={{ margin: 0 }}>{t("review.claimRelationsEmpty")}</p>
              ) : (
                <ul style={{ margin: 0, paddingLeft: 18 }}>
                  {relations.map((rel) => {
                    const outgoing = rel.claimId === selectedClaim.id;
                    const otherId = outgoing ? rel.relatedClaimId : rel.claimId;
                    const other = claims.find((c) => c.id === otherId);
                    return (
                      <li key={rel.id} className="small">
                        {outgoing ? "→" : "←"} {CLAIM_RELATION[rel.relation] ?? rel.relation} ·{" "}
                        {other?.claimText ?? t("review.claimFallback")}
                        {rel.rationale ? ` · ${rel.rationale}` : ""}
                      </li>
                    );
                  })}
                </ul>
              )}
              <div className="row">
                <div className="field" style={{ flex: "1 1 180px" }}>
                  <label htmlFor="rv-reltarget">{t("review.relatedClaimLabel")}</label>
                  <select
                    id="rv-reltarget"
                    className="input"
                    value={relationTarget}
                    onChange={(e) => setRelationTarget(e.target.value)}
                  >
                    <option value="">{t("review.claimPlaceholderSelect")}</option>
                    {claims
                      .filter((c) => c.id !== selectedClaim.id)
                      .map((c) => (
                        <option key={c.id} value={c.id}>{c.claimText}</option>
                      ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "0 1 190px" }}>
                  <label htmlFor="rv-relkind">{t("review.relationKindLabel")}</label>
                  <select
                    id="rv-relkind"
                    className="input"
                    value={relationKind}
                    onChange={(e) => setRelationKind(e.target.value)}
                  >
                    <option value="">{t("review.kindPlaceholder")}</option>
                    {Object.entries(CLAIM_RELATION).map(([k, v]) => (
                      <option key={k} value={k}>{v}</option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ flex: "2 1 160px" }}>
                  <label htmlFor="rv-relnote">{t("review.rationaleLabel")}</label>
                  <input
                    id="rv-relnote"
                    className="input"
                    value={relationNote}
                    onChange={(e) => setRelationNote(e.target.value)}
                  />
                </div>
                <button
                  className="btn"
                  disabled={busy || relationTarget === "" || relationKind === ""}
                  onClick={() => void addClaimRelation()}
                >
                  {t("review.relateAction")}
                </button>
              </div>
              <p className="hint" style={{ margin: 0 }}>{t("review.relationHint")}</p>
            </div>
          ) : null}
        </div>

        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("review.queueTitle")}</h2>
              <p className="card-sub">
                {t("review.queueSummary", {
                  pending: pending.length,
                  decided: decided.length,
                  outbox: outboxPending,
                })}
              </p>
              {outboxDomain > 0 ? (
                <p className="muted small" style={{ margin: 0 }}>
                  {t("review.outboxDomainNote", { count: outboxDomain })}
                </p>
              ) : null}
            </div>
          </div>

          <div className="card tight" style={{ background: "var(--surface)" }}>
            <h2>{t("review.draftTitle")}</h2>
            <div className="row">
              <div className="field" style={{ flex: "1 1 160px" }}>
                <label htmlFor="rv-dkind">{t("review.kindLabel")}</label>
                <select id="rv-dkind" className="input" value={draftKind} onChange={(e) => setDraftKind(e.target.value)}>
                  {Object.entries(ITEM_KIND).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                </select>
              </div>
              <div className="field" style={{ flex: "2 1 180px" }}>
                <label htmlFor="rv-danchor">{t("review.sourceLabel")}</label>
                <select id="rv-danchor" className="input" value={draftAnchor} onChange={(e) => setDraftAnchor(e.target.value)}>
                  <option value="">{t("common.select")}</option>
                  {anchors.map((a) => (
                    <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 60)}”</option>
                  ))}
                </select>
              </div>
              {draftKind === "contradiction_candidate" ? (
                <div className="field" style={{ flex: "2 1 180px" }}>
                  <label htmlFor="rv-danchor2">{t("review.secondSourceLabel")}</label>
                  <select id="rv-danchor2" className="input" value={draftAnchor2} onChange={(e) => setDraftAnchor2(e.target.value)}>
                    <option value="">{t("common.select")}</option>
                    {anchors
                      .filter((a) => a.id !== draftAnchor)
                      .map((a) => (
                        <option key={a.id} value={a.id}>“{a.exactQuote.slice(0, 60)}”</option>
                      ))}
                  </select>
                </div>
              ) : null}
            </div>
            <div className="field">
              <label htmlFor="rv-dtext">{t("review.draftTextLabel")}</label>
              <input id="rv-dtext" className="input" value={draftText} onChange={(e) => setDraftText(e.target.value)} />
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn" disabled={busy || !draftAnchor || draftText.trim().length === 0 || (draftKind === "contradiction_candidate" && (!draftAnchor2 || draftAnchor2 === draftAnchor))} onClick={() => void addCandidate()}>
                {t("review.queueAdd")}
              </button>
            </div>
            <p className="hint" style={{ margin: 0 }}>{t("review.queueHint")}</p>
          </div>

          {pending.length === 0 ? (
            <div className="empty"><p style={{ margin: 0 }}>{t("review.queueEmpty")}</p></div>
          ) : (
            pending.map((item) => (
              <div key={item.id} className="card tight" style={{ background: "var(--bg)" }}>
                <div className="row" style={{ justifyContent: "space-between" }}>
                  <span className="badge plain">{ITEM_KIND[item.itemKind] ?? item.itemKind}</span>
                  <span className="muted small">{formatTimestamp(item.createdAt)}</span>
                </div>
                <pre style={{ margin: 0, whiteSpace: "pre-wrap", fontFamily: "var(--mono)", fontSize: 12 }}>
                  {JSON.stringify(item.payload, null, 2)}
                </pre>
                <div className="muted small">
                  {t("review.sourceQuotes", { count: item.sourceAnchorIds.length })}
                </div>
                {item.itemKind === "contradiction_candidate" ? (
                  <div>
                    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
                      {item.sourceAnchorIds.slice(0, 2).map((anchorId, idx) => {
                        const anchor = anchors.find((a) => a.id === anchorId);
                        return (
                          <div key={anchorId} className="card tight" style={{ background: "var(--bg)", margin: 0 }}>
                            <span className="badge plain">{idx === 0 ? t("review.sourceOne") : t("review.sourceTwo")}</span>
                            <p className="small" style={{ margin: 0 }}>
                              “{anchor?.exactQuote ?? t("review.quoteMissing")}”
                            </p>
                            <span className="muted small">
                              {anchor ? `${anchor.startOffset}–${anchor.endOffset}` : ""}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                    {(() => {
                      const list = assessments[item.id] ?? [];
                      const latest = list[list.length - 1];
                      return latest ? (
                        <p className="small" style={{ margin: 0 }}>
                          {t("review.latestAssessment")} <strong>{VERDICT[latest.verdict] ?? latest.verdict}</strong>
                          {latest.rationale ? ` ${t("review.rationaleDash", { text: latest.rationale })}` : ""}
                        </p>
                      ) : (
                        <p className="hint" style={{ margin: 0 }}>
                          {t("review.noAssessment")}
                        </p>
                      );
                    })()}
                    <div className="row">
                      <div className="field" style={{ flex: "0 1 200px" }}>
                        <label htmlFor={`verdict-${item.id}`}>{t("review.verdictLabel")}</label>
                        <select
                          id={`verdict-${item.id}`}
                          className="input"
                          value={assessmentVerdicts[item.id] ?? ""}
                          onChange={(e) =>
                            setAssessmentVerdicts((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        >
                          <option value="">{t("common.select")}</option>
                          {Object.entries(VERDICT).map(([k, v]) => (
                            <option key={k} value={k}>{v}</option>
                          ))}
                        </select>
                      </div>
                      <div className="field" style={{ flex: "2 1 180px" }}>
                        <label htmlFor={`verdict-note-${item.id}`}>{t("review.rationaleLabel")}</label>
                        <input
                          id={`verdict-note-${item.id}`}
                          className="input"
                          value={assessmentNotes[item.id] ?? ""}
                          onChange={(e) =>
                            setAssessmentNotes((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        />
                      </div>
                      <button
                        className="btn"
                        disabled={busy || !assessmentVerdicts[item.id]}
                        onClick={() => void assess(item)}
                      >
                        {t("review.assessAction")}
                      </button>
                    </div>
                  </div>
                ) : null}
                {/* Delil istenmiş kalem kuyrukta beklerken **neden** istendiği
                    görünür: karar geçmişi burada da açılır. */}
                {item.state === "needs_more_evidence" ? (
                  <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                    <div className="row" style={{ justifyContent: "flex-end" }}>
                      {historyToggle(item)}
                    </div>
                    {historyPanel(item)}
                  </div>
                ) : null}
                {editingId === item.id ? (
                  <div className="field">
                    <label htmlFor={`edit-${item.id}`}>{t("review.editJsonLabel")}</label>
                    <textarea
                      id={`edit-${item.id}`}
                      className="input"
                      style={{ height: 80, padding: 8, fontFamily: "var(--mono)", fontSize: 12 }}
                      value={editJson}
                      onChange={(e) => setEditJson(e.target.value)}
                    />
                  </div>
                ) : null}
                {editingId === item.id ? null : (
                  <div className="row" style={{ alignItems: "flex-end" }}>
                    <div className="field" style={{ flex: "1 1 170px" }}>
                      <label htmlFor={`rk-${item.id}`}>{t("review.targetKindLabel")}</label>
                      <select
                        id={`rk-${item.id}`}
                        className="input"
                        value={targetKinds[item.id] ?? ""}
                        onChange={(e) =>
                          setTargetKinds((prev) => ({
                            ...prev,
                            [item.id]: e.target.value as ReviewTargetKind,
                          }))
                        }
                      >
                        <option value="">{t("common.select")}</option>
                        {(Object.keys(TARGET_KIND_LABEL) as Array<Exclude<ReviewTargetKind, "">>).map(
                          (k) => (
                            <option key={k} value={k}>
                              {TARGET_KIND_LABEL[k]}
                            </option>
                          ),
                        )}
                      </select>
                    </div>
                    {targetKinds[item.id] === "claim" ? (
                      <div className="field" style={{ flex: "1 1 170px" }}>
                        <label htmlFor={`ck-${item.id}`}>{t("review.targetClaimKindLabel")}</label>
                        <select
                          id={`ck-${item.id}`}
                          className="input"
                          value={targetClaimKinds[item.id] ?? ""}
                          onChange={(e) =>
                            setTargetClaimKinds((prev) => ({ ...prev, [item.id]: e.target.value }))
                          }
                        >
                          <option value="">{t("common.select")}</option>
                          {Object.entries(CLAIM_KIND).map(([k, v]) => (
                            <option key={k} value={k}>
                              {v}
                            </option>
                          ))}
                        </select>
                      </div>
                    ) : null}
                    <span className="hint" style={{ flex: "2 1 220px", margin: 0 }}>
                      {t("review.commitHint")}
                    </span>
                  </div>
                )}
                <div className="row" style={{ justifyContent: "flex-end" }}>
                  {editingId === item.id ? (
                    <>
                      <button className="btn ghost" onClick={() => setEditingId(null)}>{t("review.cancelAction")}</button>
                      <button className="btn primary" disabled={busy} onClick={() => void decide(item, "edit")}>{t("common.save")}</button>
                    </>
                  ) : (
                    <>
                      <button
                        className="btn ghost"
                        onClick={() => {
                          setEditingId(item.id);
                          setEditJson(JSON.stringify(item.payload, null, 2));
                        }}
                      >
                        {t("review.editAction")}
                      </button>
                      <button className="btn ghost" disabled={busy} onClick={() => void decide(item, "request_evidence")}>
                        {t("review.requestEvidence")}
                      </button>
                      <button className="btn ghost" disabled={busy} onClick={() => void decide(item, "reject")}>{t("review.rejectAction")}</button>
                      <button
                        className="btn primary"
                        disabled={
                          busy ||
                          !(targetKinds[item.id] ?? "") ||
                          (targetKinds[item.id] === "claim" && !(targetClaimKinds[item.id] ?? ""))
                        }
                        onClick={() => void commit(item)}
                      >
                        {t("review.acceptCommit")}
                      </button>
                    </>
                  )}
                </div>
                {replaceable.length > 0 ? (
                  <div className="row" style={{ justifyContent: "flex-end" }}>
                    <div className="field" style={{ flex: "2 1 220px" }}>
                      <label htmlFor={`sup-${item.id}`}>{t("review.supersedeLabel")}</label>
                      <select
                        id={`sup-${item.id}`}
                        className="input"
                        value={supersedeTargets[item.id] ?? ""}
                        onChange={(e) =>
                          setSupersedeTargets((prev) => ({ ...prev, [item.id]: e.target.value }))
                        }
                      >
                        <option value="">{t("review.recordPlaceholder")}</option>
                        {replaceable.map((d) => (
                          <option key={d.id} value={d.id}>
                            {ITEM_STATE[d.state] ?? d.state} · {ITEM_KIND[d.itemKind] ?? d.itemKind} · {formatTimestamp(d.createdAt)}
                          </option>
                        ))}
                      </select>
                    </div>
                    <button
                      className="btn ghost"
                      disabled={busy || !supersedeTargets[item.id]}
                      onClick={() => void supersede(item, supersedeTargets[item.id] ?? "")}
                    >
                      {t("review.supersedeAction")}
                    </button>
                  </div>
                ) : null}
              </div>
            ))
          )}

          {decided.length > 0 ? (
            <div>
              <h2 style={{ marginTop: 8 }}>{t("review.decidedTitle")}</h2>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {decided.map((item) => (
                  <div
                    key={item.id}
                    style={{ border: "1px solid var(--line)", borderRadius: "var(--radius)" }}
                  >
                    <div className="row" style={{ justifyContent: "space-between", padding: "6px 8px" }}>
                      <span className="small">
                        {ITEM_KIND[item.itemKind] ?? item.itemKind}
                        {item.targetEntityId ? (
                          <span className="badge plain" style={{ marginLeft: 8 }}>
                            {t("review.targetBadge", { id: item.targetEntityId.slice(0, 8) })}
                          </span>
                        ) : null}
                      </span>
                      <div className="row" style={{ gap: 6, alignItems: "center" }}>
                        <span className={`badge ${item.state === "accepted" ? "ok" : item.state === "rejected" ? "danger" : "warn"}`}>
                          {ITEM_STATE[item.state] ?? item.state}
                        </span>
                        {historyToggle(item)}
                      </div>
                    </div>
                    <div style={{ padding: "0 8px 8px" }}>{historyPanel(item)}</div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("review.quoteTitle")}</h2>
            <p className="card-sub">{t("review.quoteLead")}</p>
          </div>
        </div>
        <div className="row">
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="rv-qclaim">{t("review.useClaimLabel")}</label>
            <select
              id="rv-qclaim"
              className="input"
              value={quoteSource}
              onChange={(e) => {
                const id = e.target.value;
                setQuoteSource(id);
                const claim = claims.find((c) => c.id === id);
                if (claim) {
                  setQuoteText(claim.claimText);
                  invalidateQuoteCheck();
                }
              }}
            >
              <option value="">{t("common.select")}</option>
              {claims.map((claim) => (
                <option key={claim.id} value={claim.id}>
                  {claim.claimText.slice(0, 90)}
                </option>
              ))}
            </select>
          </div>
          <div className="field" style={{ flex: "1 1 240px" }}>
            <label htmlFor="rv-qscope">{t("review.scopeLabel")}</label>
            <select
              id="rv-qscope"
              className="input"
              value={quoteScope}
              onChange={(e) => {
                setQuoteScope(e.target.value);
                invalidateQuoteCheck();
              }}
            >
              <option value="">{t("review.scopeAll")}</option>
              {documents.map((document) => (
                <option key={document.id} value={document.id}>
                  {document.displayName}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="field">
          <label htmlFor="rv-qtext">{t("review.quoteTextLabel")}</label>
          <textarea
            id="rv-qtext"
            className="input"
            rows={3}
            value={quoteText}
            onChange={(e) => {
              setQuoteText(e.target.value);
              invalidateQuoteCheck();
            }}
            placeholder={t("review.quotePlaceholder")}
          />
        </div>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <span className="hint" style={{ margin: 0, flex: "1 1 220px" }}>
            {t("review.quoteHint")}
          </span>
          <button
            className="btn primary"
            disabled={quoteBusy || quoteText.trim().length === 0}
            onClick={() => void runQuoteCheck()}
          >
            {quoteBusy ? t("review.quoteBusy") : t("review.quoteRun")}
          </button>
        </div>

        {quoteBindNotice ? (
          // Hüküm bloğunun **dışında**: çapa yazıldığında alıntı listesi
          // değişmiş olabilir, yani aşağıdaki blok ekranda olmayabilir.
          <p className="hint" style={{ margin: 0 }} role="status">
            {t("review.bindKeptNote", { id: quoteBindNotice.slice(0, 8) })}
          </p>
        ) : null}

        {quoteCheck ? (
          <div className="card tight" style={{ background: "var(--surface)" }}>
            <div className="row" style={{ justifyContent: "space-between" }}>
              <strong className="small">{quoteCheckSummary(quoteCheck)}</strong>
              <span className="muted small">
                {[
                  scopeLabel(quoteCheck),
                  t("intext.pagesScanned", { count: quoteCheck.pagesScanned }),
                  versionsLabel(quoteCheck),
                ]
                  .filter((part) => part !== "")
                  .join(" · ")}
              </span>
            </div>
            {skippedLabel(quoteCheck) ? (
              <p className="hint" style={{ margin: 0 }}>{skippedLabel(quoteCheck)}</p>
            ) : null}
            {quoteCheck.quotes.length === 0 ? (
              <p className="hint" style={{ margin: 0 }}>{t("review.quoteEmpty")}</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {quoteCheck.quotes.map((finding) => (
                  <div
                    key={finding.index}
                    style={{
                      border: "1px solid var(--line)",
                      borderRadius: "var(--radius)",
                      padding: "8px 10px",
                      background: "var(--bg)",
                    }}
                  >
                    <div className="row" style={{ justifyContent: "space-between" }}>
                      <strong className="small">“{finding.quote}”</strong>
                      <span className={`badge ${quoteStateTone(finding.state)}`}>
                        {QUOTE_STATE_LABEL[finding.state] ?? finding.state}
                      </span>
                    </div>
                    {finding.reason === "no_text" ? (
                      <p className="hint" style={{ margin: 0 }}>
                        {t("review.noText")}
                      </p>
                    ) : null}
                    {finding.position ? (
                      <p className="muted small" style={{ margin: 0 }}>
                        {[
                          positionLabel(finding),
                          t("review.sourcePrefix", {
                            text: finding.position.sourceText
                              .replace(/\s*\n\s*/g, " ⏎ ")
                              .slice(0, 140),
                          }),
                        ].join(" · ")}
                      </p>
                    ) : null}
                    {finding.position?.spansPages ? (
                      <p className="hint" style={{ margin: 0 }}>
                        {t("review.spanPagesHint", {
                          start: finding.position.pageIndex + 1,
                          end: finding.position.endPageIndex + 1,
                        })}
                      </p>
                    ) : null}
                    {oldVersionNote(finding) ? (
                      <p className="hint" style={{ margin: 0 }}>
                        <span className="badge warn" style={{ marginRight: 6 }}>
                          {t("review.oldVersionBadge")}
                        </span>
                        {oldVersionNote(finding)}
                      </p>
                    ) : null}
                    {finding.candidates.length > 0 ? (
                      <ul className="small" style={{ margin: "4px 0 0", paddingLeft: 18 }}>
                        {finding.candidates.map((candidate, candidateIndex) => (
                          <li key={`${candidate.documentId}-${candidate.pageIndex}-${candidate.charStart}-${candidateIndex}`}>
                            {candidateLabel(candidate)}
                          </li>
                        ))}
                      </ul>
                    ) : null}
                    <div className="row" style={{ justifyContent: "flex-end", marginTop: 4 }}>
                      {quoteBindings[finding.index] ? (
                        <span className="badge ok">
                          {t("review.boundBadge", {
                            id: quoteBindings[finding.index]?.slice(0, 8) ?? "",
                          })}
                        </span>
                      ) : canBind(finding) ? (
                        <button className="btn" disabled={quoteBusy} onClick={() => void bindQuote(finding)}>
                          {t("review.bindQuoteAction")}
                        </button>
                      ) : (
                        <span className="hint" style={{ margin: 0 }}>
                          {bindHint(finding)}
                        </span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
            {quoteCheck.notes.length > 0 ? (
              <details>
                <summary className="small">
                  {t("review.notesSummary", { count: quoteCheck.notes.length })}
                </summary>
                <ul className="small" style={{ margin: "4px 0 0", paddingLeft: 18 }}>
                  {quoteCheck.notes.map((note, noteIndex) => (
                    <li key={noteIndex}>{note}</li>
                  ))}
                </ul>
              </details>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
