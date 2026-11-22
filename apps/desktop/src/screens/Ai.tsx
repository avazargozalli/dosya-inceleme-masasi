import { useEffect, useRef, useState } from "react";
import {
  api,
  toApiError,
  type AiAnswerBasisRow,
  type AiCandidateRow,
  type AiContextItemRow,
  type AiMessageRow,
  type AiOverview,
  type AiQualityReport,
  type AiRunRow,
  type ApiError,
  type AttackAssessment,
  type CandidateReport,
  type CaseRow,
  type ConsentPreview,
  type ContradictionBasis,
  type QuoteMatchReport,
  type RedactionPlanRow,
  type SentenceAttribution,
  type TransferConsentListRow,
  type TransferConsentRow,
  type TransferReceiptRow,
  type TransferSegmentRow,
} from "../lib/api";
import {
  candidateSummary,
  directAcceptShare,
  quoteAccuracy,
  quoteSummary as qualityQuoteSummary,
  reviewSummary,
  usableShare,
} from "../lib/ai-quality";
import { formatDateTime, formatPercent, formatSize } from "../lib/format";
import { t } from "../lib/i18n";
import { consentScopeLine, skippedSummary } from "../lib/transferScope";
import {
  CANDIDATE_KINDS,
  answerBasisReasonLabel,
  answerBasisStateLabel,
  answerBasisTone,
  attributionStateLabel,
  attributionStateTone,
  attributionSummary,
  candidateKindLabel,
  candidateStateLabel,
  candidateStateTone,
  contradictionDateRoleLabel,
  contradictionReasonLabel,
  contradictionSideStateLabel,
  contradictionStateLabel,
  contradictionStateTone,
  draftJsonError,
  errorCount,
  instructionCodeLabel,
  parseIssues,
  promoteBlockedReason,
  quoteLineStateLabel,
  quoteLineTone,
  quotePositionLabel,
  quoteStateLabel,
  quoteStateTone,
  quoteSummary,
  sentenceQuoteRefs,
  sentenceStateLabel,
  sentenceStateTone,
  warningCount,
} from "../lib/candidate";

// Onay **amaca özgüdür** (F09-T09): amaç onay kaydına yazılır, çalıştırmanın
// görev türünü **çekirdek** ondan türetir. Arayüz görev türü üretmez —
// buradaki eşlemenin ikinci bir kopyası olsaydı ikisi ayrışabilirdi.

function purposeLabel(purpose: string): string {
  switch (purpose) {
    case "ozet":
      return t("ai.purposeSummary");
    case "olay_cikarimi":
      return t("ai.purposeEvents");
    case "iddia_cikarimi":
      return t("ai.purposeClaims");
    case "soru":
      return t("ai.purposeAnswer");
    case "celiski":
      return t("ai.purposeContradictions");
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
function consentMaskLabel(row: TransferConsentListRow): string {
  // Maske **onayın parçasıdır** (şartname §763): envanter "hangi maske
  // onaylandı" sorusunu yanıtlar. Terimlerin kendisi saklanmaz — yalnız sayı.
  if (row.outboundTransformId === null) return t("ai.consentMaskUnbound");
  return row.maskTermCount > 0
    ? t("ai.consentMaskTerms", { terms: row.maskTermCount })
    : t("ai.consentMaskNone");
}

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
 * yazılmadı (yanıt gelmedi ya da süreç sonucu yazamadan kapandı). `interrupted`
 * ise açılış uzlaştırmasının yazdığı **kesin** durumdur: sonucu yazacak çağrı
 * artık yok; içerik çıkmış olabilir. Bilinmeyen durum olduğu gibi yazılır —
 * ekran tanımadığı bir değeri "gönderildi" saymaz.
 */
function receiptStateLabel(status: string): string {
  if (status === "dispatched") return t("ai.receiptDispatched");
  if (status === "failed") return t("ai.receiptFailed");
  if (status === "attempted") return t("ai.receiptAttempted");
  if (status === "interrupted") return t("ai.receiptInterrupted");
  return status;
}

function receiptStateTone(status: string): string {
  if (status === "dispatched") return "ok";
  if (status === "failed") return "danger";
  return "warn";
}

/**
 * Çalıştırma durumu. Açılış uzlaştırması kesilmiş satırı `failed` yazar ama
 * ayrımı `usageReport.error=interrupted` taşır: ekran bunu "başarısız" diye
 * okumaz — istek çıkmış olabilir, sonucu yazacak çağrı artık yoktur ve
 * "sürüyor" da denmez. Bilinmeyen durum olduğu gibi yazılır.
 */
function runInterrupted(run: AiRunRow): boolean {
  if (run.status !== "failed") return false;
  try {
    const report: unknown = JSON.parse(run.usageReport);
    return (
      typeof report === "object" &&
      report !== null &&
      (report as { error?: unknown }).error === "interrupted"
    );
  } catch {
    return false;
  }
}

function runStateLabel(run: AiRunRow): string {
  if (runInterrupted(run)) return t("ai.runInterrupted");
  if (run.status === "succeeded") return t("ai.runSucceeded");
  if (run.status === "failed") return t("ai.runFailed");
  if (run.status === "running") return t("ai.runRunning");
  if (run.status === "partial") return t("ai.runPartial");
  if (run.status === "cancelled") return t("ai.runCancelled");
  return run.status;
}

function runStateTone(run: AiRunRow): string {
  if (runInterrupted(run)) return "warn";
  if (run.status === "succeeded") return "ok";
  if (run.status === "failed") return "danger";
  if (run.status === "partial") return "warn";
  return "";
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

/**
 * Çekirdek, onayın **isteği kapsamadığını** söylediğinde (maske/amaç/bağ/kapsam)
 * aynı onayla ikinci deneme de reddedilir: karar `start_ai_run` içinde ve
 * durumsuzdur (`consent.outbound_transform_id` / `purpose` / belge canlılığı).
 * Bu yüzden ekran onayı **çalıştırılabilir** tutamaz; yenileme yolu tektir:
 * yeni onay (`Onayla`) — önizleme de onunla birlikte tazelenir.
 */
const CONSENT_RENEWAL_CODES: readonly string[] = [
  "E_AI_CONSENT",
  "E_AI_CONSENT_UNBOUND",
  "E_AI_MASK_CHANGED",
  "E_AI_PURPOSE",
];

function consentNeedsRenewal(code: string): boolean {
  return CONSENT_RENEWAL_CODES.includes(code);
}

/**
 * F11-T07 — «hangi cümle hangi alıntıya dayanıyor?».
 *
 * Cümleler **çekirdekten** gelir (deterministik bölme); bağ yalnız modelin
 * `sentences` bildiriminden kurulur. Kaynaksız cümleler ayrı blokta gösterilir
 * — yorum, kaynaklı cümleyle aynı görünmemeli. Bağ hiç bildirilmediyse ekran
 * "kaynaksız" **demez**: ayrımın yapılamadığını yazar (uydurma damga yok).
 */
function SentenceAttributionView({
  attribution,
  compact = false,
}: {
  attribution: SentenceAttribution;
  compact?: boolean;
}) {
  const sourced = attribution.sentences.filter((sentence) => sentence.state !== "unsourced");
  const unsourced = attribution.sentences.filter((sentence) => sentence.state === "unsourced");
  return (
    <div style={{ marginTop: compact ? 0 : "var(--space-2)" }}>
      {compact ? null : (
        <div className="row" style={{ gap: 8, alignItems: "center" }}>
          <strong>{t("candidate.attribution.title")}</strong>
          <span className={`badge ${attributionStateTone(attribution.state)}`}>
            {attributionStateLabel(attribution.state)}
          </span>
          {attribution.state !== "unlinked" ? (
            <span className="badge plain">{attributionSummary(attribution)}</span>
          ) : null}
        </div>
      )}
      {attribution.state === "unlinked" ? (
        <p className="hint" style={{ marginBottom: 0 }}>
          {t("candidate.attribution.unlinkedHint")}
        </p>
      ) : (
        <>
          {sourced.length > 0 ? (
            <ol style={{ margin: "var(--space-1) 0 0", paddingLeft: 18 }}>
              {sourced.map((sentence) => (
                <li key={sentence.index} style={{ marginBottom: 4 }}>
                  <span className={`badge ${sentenceStateTone(sentence.state)}`}>
                    {sentenceStateLabel(sentence.state)}
                  </span>{" "}
                  {sentence.text}
                  {sentence.quoteIndexes.length > 0 ? (
                    <span className="hint"> ({sentenceQuoteRefs(sentence.quoteIndexes)})</span>
                  ) : null}
                </li>
              ))}
            </ol>
          ) : null}
          {attribution.unmatched > 0 ? (
            <p className="hint">
              {t("candidate.attribution.unmatched", { count: attribution.unmatched })}
            </p>
          ) : null}
          {unsourced.length > 0 ? (
            <div
              style={{
                marginTop: "var(--space-1)",
                paddingLeft: "var(--space-2)",
                borderLeft: "3px solid var(--danger)",
              }}
            >
              <strong className="hint">{t("candidate.attribution.unsourcedTitle")}</strong>
              <ul className="hint" style={{ margin: "4px 0 0", paddingLeft: 16 }}>
                {unsourced.map((sentence) => (
                  <li key={sentence.index}>{sentence.text}</li>
                ))}
              </ul>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}

/**
 * F11-T09 — «çelişki iki kaynakla mı sunuluyor?».
 *
 * İki taraf **yan yana** gösterilir: her tarafın alıntısı, konumu ve tarih
 * rolü. `presentable` «çelişki var» demek **değildir**; iki kaynağın birebir
 * doğrulandığını söyler — hüküm avukatındır. Tarih rolleri farklıysa ekran
 * bunu açıkça yazar: belge tarihi ile tebliğ tarihi farkı çelişki değildir.
 */
function ContradictionBasisView({
  basis,
  quotes,
  compact = false,
}: {
  basis: ContradictionBasis;
  quotes: QuoteMatchReport[];
  compact?: boolean;
}) {
  const roleDifference = basis.state === "role_difference";
  return (
    <div style={{ marginTop: compact ? 0 : "var(--space-2)" }}>
      {compact ? null : (
        <div className="row" style={{ gap: 8, alignItems: "center" }}>
          <strong>{t("candidate.contradiction.title")}</strong>
          <span className={`badge ${contradictionStateTone(basis.state)}`}>
            {contradictionStateLabel(basis.state)}
          </span>
          <span className="badge plain">
            {t("candidate.contradiction.documentCount", { count: basis.distinctDocuments })}
          </span>
        </div>
      )}
      {basis.sides.length === 0 ? (
        <p className="hint" style={{ marginBottom: 0 }}>
          {t("candidate.contradiction.unlinkedHint")}
        </p>
      ) : (
        <div style={{ display: "grid", gap: "var(--space-2)", marginTop: "var(--space-1)" }}>
          {basis.sides.map((side) => {
            const quote = quotes.find((item) => item.index === side.quoteIndex);
            const role = contradictionDateRoleLabel(side.dateKind);
            return (
              <div
                key={side.quoteIndex}
                style={{ paddingLeft: "var(--space-2)", borderLeft: "3px solid var(--line)" }}
              >
                <div className="row" style={{ gap: 6, alignItems: "center" }}>
                  <strong>{t("candidate.contradiction.sideLabel", { side: side.side })}</strong>
                  <span className={`badge ${quoteLineTone(side.quoteState)}`}>
                    {contradictionSideStateLabel(side.quoteState)}
                  </span>
                  <span className="hint">
                    {side.pageIndex === null
                      ? t("candidate.position.missing")
                      : t("candidate.position.page", { page: side.pageIndex + 1 })}
                  </span>
                  {side.label ? <span className="hint">· {side.label}</span> : null}
                </div>
                <p style={{ margin: "4px 0" }}>
                  “{quote ? quote.quote : t("candidate.contradiction.quoteMissing")}”
                </p>
                <p className="hint" style={{ margin: 0 }}>
                  {t("candidate.contradiction.dateRole", {
                    role: role ?? t("candidate.contradiction.dateRoleMissing"),
                    text: side.dateText ?? "—",
                  })}
                </p>
              </div>
            );
          })}
        </div>
      )}
      {roleDifference ? (
        <p className="hint" style={{ marginBottom: 0 }}>
          {t("candidate.contradiction.roleDifferenceHint")}
        </p>
      ) : null}
      {basis.reasons.length > 0 ? (
        <p className="hint" style={{ marginBottom: 0 }}>
          {t("candidate.contradiction.reasons", {
            list: basis.reasons.map(contradictionReasonLabel).join(", "),
          })}
        </p>
      ) : null}
    </div>
  );
}

// F11-T12 — model/prompt sürümü bazında kalite ölçümü. Sayımlar çekirdekten
// **okuma anında** gelir; oranlar `lib/ai-quality`te kurulur, paydası olmayan
// hüküm «%0» diye yazılmaz. Kart yalnız bu dosyanın ölçümüdür; kasadaki başka
// dosyaların çalıştırmaları karışmaz.
function AiQualityCard({ caseId }: { caseId: string }) {
  const [report, setReport] = useState<AiQualityReport | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  // Nesil sayacı: dosya hızlı değişirse eski yanıt yeni dosyanın tablosuna
  // yazılmaz (geç yanıt kapısı).
  const gen = useRef(0);

  useEffect(() => {
    const seq = ++gen.current;
    setReport(null);
    setError(null);
    setBusy(true);
    api
      .aiQualityReport(caseId)
      .then((next) => {
        if (gen.current !== seq) return;
        setReport(next);
      })
      .catch((e) => {
        if (gen.current !== seq) return;
        setError(toApiError(e));
      })
      .finally(() => {
        if (gen.current === seq) setBusy(false);
      });
    return () => {
      gen.current += 1;
    };
  }, [caseId]);

  return (
    <div className="card">
      <div className="card-head">
        <div>
          <h2>{t("ai.qualityTitle")}</h2>
          <p className="card-sub">{t("ai.qualitySub")}</p>
        </div>
      </div>
      {error ? (
        // Düşen okuma «kayıt yok» sayılmaz: kendi satırında yazılır.
        <div className="notice danger" role="alert">
          <strong>{t("ai.qualityError")}</strong> {error.code} — {error.message}
        </div>
      ) : report === null ? (
        <p className="hint" style={{ margin: 0 }}>
          {busy ? t("ai.qualityLoading") : t("ai.qualityEmpty")}
        </p>
      ) : report.groups.length === 0 ? (
        <p className="hint" style={{ margin: 0 }}>{t("ai.qualityEmpty")}</p>
      ) : (
        <>
          <table className="table">
            <thead>
              <tr>
                <th>{t("ai.qualityColModel")}</th>
                <th>{t("ai.qualityColRuns")}</th>
                <th>{t("ai.qualityColCandidates")}</th>
                <th>{t("ai.qualityColQuotes")}</th>
                <th>{t("ai.qualityColReview")}</th>
              </tr>
            </thead>
            <tbody>
              {report.groups.map((group) => {
                const quotes = qualityQuoteSummary(group);
                const review = reviewSummary(group);
                const accuracy = quoteAccuracy(group);
                const candidates = candidateSummary(group);
                return (
                  <tr key={`${group.modelSnapshot}\u0000${group.promptVersion}`}>
                    <td>
                      <code>{group.modelSnapshot}</code>
                      <div className="hint">
                        {t("ai.qualityPrompt", { version: group.promptVersion })}
                      </div>
                    </td>
                    <td>
                      {t("ai.qualityRunsCell", {
                        runs: group.runs,
                        succeeded: group.runsSucceeded,
                        failed: group.runsFailed,
                      })}
                      {group.runsUnfinished > 0 ? (
                        <div className="hint">
                          {t("ai.qualityRunsUnfinished", { count: group.runsUnfinished })}
                        </div>
                      ) : null}
                    </td>
                    <td>
                      {t("ai.qualityCandidatesCell", {
                        total: candidates.total,
                        valid: candidates.valid,
                        partial: candidates.partial,
                        invalid: candidates.invalid,
                      })}
                      {candidates.other > 0 ? (
                        <div className="hint">
                          {t("ai.qualityCandidatesOther", { count: candidates.other })}
                        </div>
                      ) : null}
                      {quotes.noQuotes > 0 ? (
                        <div className="hint">
                          {t("ai.qualityCandidatesNoQuotes", { count: quotes.noQuotes })}
                        </div>
                      ) : null}
                      {quotes.unmeasuredCandidates > 0 ? (
                        <div className="hint">
                          {t("ai.qualityCandidatesUnmeasured", {
                            count: quotes.unmeasuredCandidates,
                          })}
                        </div>
                      ) : null}
                    </td>
                    <td>
                      {quotes.checked > 0 ? (
                        <>
                          <div>
                            {t("ai.qualityQuotesGrounded", {
                              checked: quotes.checked,
                              exact: group.quotesExact,
                              normalized: group.quotesNormalized,
                            })}
                          </div>
                          <div className="hint">
                            {t("ai.qualityQuotesWeak", {
                              count: quotes.weak,
                              notFound: quotes.notFound,
                            })}
                          </div>
                          <div className="hint">
                            {t("ai.qualityAccuracy", {
                              percent: formatPercent(accuracy),
                            })}
                          </div>
                        </>
                      ) : (
                        <span className="hint">{t("ai.qualityAccuracyUnmeasured")}</span>
                      )}
                      {quotes.unverifiable > 0 ? (
                        <div className="hint">
                          {t("ai.qualityQuotesUnverifiable", { count: quotes.unverifiable })}
                        </div>
                      ) : null}
                    </td>
                    <td>
                      {t("ai.qualityReviewCell", {
                        promoted: review.promoted,
                        accept: review.accept,
                        edit: review.edit,
                        reject: review.reject,
                        needsEvidence: review.needsEvidence,
                      })}
                      {review.pending > 0 || review.superseded > 0 ? (
                        <div className="hint">
                          {t("ai.qualityReviewPending", {
                            count: review.pending,
                            superseded: review.superseded,
                          })}
                        </div>
                      ) : null}
                      {review.verdicts > 0 ? (
                        <div className="hint">
                          {t("ai.qualityDirectAccept", {
                            percent: formatPercent(directAcceptShare(group)),
                          })}
                          {" · "}
                          {t("ai.qualityUsable", {
                            percent: formatPercent(usableShare(group)),
                          })}
                        </div>
                      ) : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <p className="hint" style={{ margin: 0 }}>{t("ai.qualityNote")}</p>
        </>
      )}
    </div>
  );
}

export function Ai() {
  const [cases, setCases] = useState<CaseRow[]>([]);
  const [caseId, setCaseId] = useState("");
  const [overview, setOverview] = useState<AiOverview | null>(null);
  const [consent, setConsent] = useState<TransferConsentRow | null>(null);
  // Gönderim maskesi: satır başına bir terim. Terimler **kasaya yazılmaz**;
  // çekirdek maskeyi uygular, sicile yalnız terim sayısı düşer.
  const [maskText, setMaskText] = useState("");
  // Karartma planı **kimlikle** seçilir: terimler çekirdekte türetilir ve
  // arayüze dönmez (planda yalnız bölge geometrisi var).
  const [maskPlanId, setMaskPlanId] = useState("");
  const [maskPlans, setMaskPlans] = useState<RedactionPlanRow[]>([]);
  const [maskPlansBusy, setMaskPlansBusy] = useState(false);
  // Okuma hatası **kendi satırında** tutulur: düşen okumadan sonra "plan yok"
  // demek, olmayan bir olguyu bildirmek olurdu.
  const [maskPlansError, setMaskPlansError] = useState<ApiError | null>(null);
  const [consents, setConsents] = useState<TransferConsentListRow[]>([]);
  const [consentListBusy, setConsentListBusy] = useState(false);
  // Makbuz envanteri de **çekirdekten** okunur: yazılan makbuzun okuma yolu
  // yoktu, avukat neyin çıktığını göremiyordu.
  const [receipts, setReceipts] = useState<TransferReceiptRow[]>([]);
  const [receiptListBusy, setReceiptListBusy] = useState(false);
  const [preview, setPreview] = useState<ConsentPreview | null>(null);
  // Önizlemenin hangi maske **imzasıyla** alındığı. Maske onaydan/önizlemeden
  // sonra değişirse gönderim farklı metinle çıkardı → "Çalıştır" kapanır.
  const [previewMaskSig, setPreviewMaskSig] = useState<string | null>(null);
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
  // Görünen model yanıtı (şartname §06.3 `ai_messages`): sağlayıcının
  // kullanıcıya görünen metni — yapısal şema hatasında da saklanır.
  const [messages, setMessages] = useState<AiMessageRow[]>([]);
  const [messagesBusy, setMessagesBusy] = useState(false);
  const [messagesError, setMessagesError] = useState<ApiError | null>(null);
  // F11-T08 — soru metni + yanıtın **kapsam/dayanak** ölçümü. Ölçüm çekirdekten
  // okunur (`ai_answer_basis`); arayüz "sınırlı" işaretini tahmin etmez.
  const [question, setQuestion] = useState("");
  const [answerBasis, setAnswerBasis] = useState<AiAnswerBasisRow | null>(null);
  const [answerBasisBusy, setAnswerBasisBusy] = useState(false);
  const [answerBasisError, setAnswerBasisError] = useState<ApiError | null>(null);
  // F11-T11 — saldırı değerlendirmesi (talimat taraması, sahte kaynak, şema
  // taşması) çalıştırma seçilince çekirdekten okunur.
  const [attack, setAttack] = useState<AttackAssessment | null>(null);
  const [attackBusy, setAttackBusy] = useState(false);
  const [attackError, setAttackError] = useState<ApiError | null>(null);
  const [purpose, setPurpose] = useState("ozet");
  const [error, setError] = useState<ApiError | null>(null);
  // Onay **reddedildi**: çekirdek “bu onay isteği kapsamıyor” dedi. Onay
  // ekranda tutulmaz (Çalıştır yalnız aynı hatayı yinelerdi) ama kullanıcı
  // nedenini ve tek yenileme yolunu görür — sessiz kapanan düğme yok.
  const [renewNeeded, setRenewNeeded] = useState(false);
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
  // Karartma planı listesi de bir **okuma**dır: son başlayan kazanır; dosya
  // değişiminde geç yanıt yeni dosyanın listesine yazılmaz.
  const maskPlanSeq = useRef(0);
  // Gönderim sicili de bir **okuma**dır: `runSeq` seçimi, `contextSeq` okumayı
  // damgalar — geç yanıt başka çalıştırmanın satırı olarak yazılmaz.
  const contextSeq = useRef(0);
  // Segment sicili de bağımsız bir **okuma**dır: kendi nesli vardır.
  const segmentsSeq = useRef(0);
  // Model yanıtı da bağımsız bir **okuma**dır: kendi nesli vardır.
  const messagesSeq = useRef(0);
  // Kapsam/dayanak ölçümü de bağımsız bir **okuma**dır.
  const answerBasisSeq = useRef(0);
  // Saldırı değerlendirmesi de bağımsız bir **okuma**dır.
  const attackSeq = useRef(0);
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

  /**
   * Dosyanın karartma planları — **okuma**. Plan satırı yalnız kimlik, belge
   * adı, sürüm ve bölge sayısı taşır; maskelenecek terimler planda yoktur.
   * Liste düşerse sessizce "plan yok" denmez: hata yazılır ve seçim temizlenir.
   */
  async function loadMaskPlans(forCase: string): Promise<void> {
    if (!forCase) return;
    maskPlanSeq.current += 1;
    const seq = maskPlanSeq.current;
    setMaskPlansBusy(true);
    setMaskPlansError(null);
    try {
      const rows = await api.redactionPlanList(forCase);
      if (maskPlanSeq.current !== seq) return;
      setMaskPlans(rows);
    } catch (e) {
      if (maskPlanSeq.current !== seq) return;
      setMaskPlans([]);
      setMaskPlansError(toApiError(e));
      setError(toApiError(e));
    } finally {
      if (maskPlanSeq.current === seq) setMaskPlansBusy(false);
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
      setMaskPlans([]);
      setMaskPlansBusy(false);
      setMaskPlansError(null);
      setMaskPlanId("");
      maskPlanSeq.current += 1;
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
    setRenewNeeded(false);
    setMaskText("");
    setConsents([]);
    setConsentListBusy(false);
    setReceipts([]);
    setReceiptListBusy(false);
    setPreview(null);
    setPreviewMaskSig(null);
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
    // Soru metni dosyaya bağlıdır: önceki dosyaya yazılmış soru yeni dosyada
    // sessizce gönderilmez (INV-05).
    setQuestion("");
    setAnswerBasis(null);
    setAnswerBasisBusy(false);
    setAnswerBasisError(null);
    answerBasisSeq.current += 1;
    // Saldırı değerlendirmesi de çalıştırmaya bağlıdır.
    setAttack(null);
    setAttackBusy(false);
    setAttackError(null);
    attackSeq.current += 1;
    // Plan seçimi dosyaya bağlıdır: önceki dosyanın planıyla yeni dosyanın
    // metni maskelenemez (INV-05).
    setMaskPlanId("");
    setMaskPlansError(null);
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
    void loadMaskPlans(caseId);
  }, [caseId]);

  // F11-T08 — kapsam/dayanak ölçümü **soru-cevap** çalıştırmalarına aittir ve
  // seçili çalıştırmayla birlikte okunur. Başka görevde istek atılmaz; seçim
  // düşünce ölçüm de temizlenir (bayat "dayanak doğrulandı" kalmaz).
  useEffect(() => {
    const selected = overview?.runs.find((run) => run.id === runId) ?? null;
    if (!runId || selected?.taskType !== "answer") {
      answerBasisSeq.current += 1;
      setAnswerBasis(null);
      setAnswerBasisBusy(false);
      setAnswerBasisError(null);
      return;
    }
    void loadAnswerBasis(runId);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- okuma yardımcısı
    // render'lar arasında kararlıdır; nesil sayacı geç yanıtı zaten keser.
  }, [runId, overview]);

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
  // F11-T08 — seçili çalıştırmanın görev türü: kapsam/dayanak ölçümü yalnız
  // soru-cevap çalıştırmalarında çizilir (özet/çıkarımda "yanıt" yoktur).
  const selectedRun = overview?.runs.find((run) => run.id === runId) ?? null;
  const selectedIsAnswer = selectedRun?.taskType === "answer";
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
  // Soru **çekirdekle aynı** ölçüyle denetlenir: kırpılmış kod noktası sayısı
  // 3–500. Geçersizse istek hiç gitmez (çekirdek de `E_AI_QUESTION` verir).
  const questionText = question.trim();
  const questionLength = [...questionText].length;
  const questionInvalid =
    purpose === "soru" && (questionLength < 3 || questionLength > 500);
  // "Model yanıtı" kartı **modelin** satırlarını çizer; kullanıcının sorusu
  // `answerQuestionLine` ile ayrı yazılır (soru satırına «şemaya çözüldü»
  // durumu basılmaz).
  const modelMessages = messages.filter((message) => message.role !== "user");
  // Maske imzası: önizleme/onay **hangi maske** üzerinden alındı. Gönderim
  // aynı maskeyi kullanmıyorsa "Çalıştır" kapanır — onaylanan metin ile çıkan
  // metin ayrışmaz (şartname §11.7: maskeleme durumu önizlemenin parçasıdır).
  const maskSig = `${maskPlanId}\u0000${maskTerms.join("\u0000")}`;
  const maskChangedSincePreview =
    preview !== null && previewMaskSig !== null && previewMaskSig !== maskSig;

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
      const nextPreview = await api.aiConsentPreview(
        caseId,
        ids,
        maskTerms.length > 0 ? maskTerms : null,
        maskPlanId || null,
      );
      if (caseGen.current !== gen) return;
      setPreview(nextPreview);
      setPreviewMaskSig(maskSig);
      // Onay **sağlayıcı profiline** bağlanır: kimlik dizesini arayüz kurmaz,
      // çekirdek profilden türetir (uç nokta + model parmak izi dahil). Böylece
      // sağlayıcı değişince eski onay kendiliğinden geçersizleşir.
      const nextConsent = await api.aiConsentCreate(
        caseId,
        provider.id,
        purpose,
        ids,
        // Maske **onayın parçasıdır**: onaylanan terimler/plan kayda bağlanır;
        // çalıştırma aynı maskeyi getirmezse çekirdek reddeder (şartname §763:
        // maskenin kaldırılması izni genişletir).
        maskTerms.length > 0 ? maskTerms : null,
        maskPlanId || null,
      );
      if (caseGen.current !== gen) return;
      setConsent(nextConsent);
      // Yeni onay **yenileme**dir: reddedilen onayın yerine geçen künye geldi,
      // “onayı yenile” uyarısı artık geçerli değil.
      setRenewNeeded(false);
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
      // Görev türü de onaydan gelir: "İddia çıkarımı" onayı özet olarak
      // koşmaz — arayüz görev türü **göndermez**, çekirdek onayın amacından
      // türetir. Maske **yalnız** girildiyse/seçildiyse gönderilir: boş liste
      // göndermek, "maske istendi ama uygulanmadı" izlenimi verirdi. Plan
      // seçiliyse terimler çekirdekte türetilir; arayüz yalnız plan kimliğini
      // taşır.
      const candidate = await api.aiRunExecute(
        caseId,
        consentProvider.id,
        consent.id,
        maskTerms.length > 0 ? maskTerms : null,
        maskPlanId || null,
        // F11-T08 — soru yalnız `soru` amacında gönderilir; çekirdek başka
        // görevde soru görürse isteği reddeder (`E_AI_QUESTION`).
        purpose === "soru" ? questionText : null,
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
      // Onay **artık isteği kapsamıyor** (maske tanımı değişti, amaç tanınmıyor,
      // maske bağı yok ya da kapsam daraldı): çekirdek kararı durumsuz verir,
      // aynı onayla ikinci deneme de düşer. Onay ve ondan türeyen önizleme
      // ekranda tutulmaz — Çalıştır yalnız aynı hatayı yinelerdi; yenileme yolu
      // tek: “Onayla” (onay + önizleme birlikte tazelenir).
      if (consentNeedsRenewal(failure.code) && caseGen.current === gen) {
        setConsent(null);
        setPreview(null);
        setPreviewMaskSig(null);
        setRenewNeeded(true);
        // Envanter çekirdekten tazelenir: hangi onayın hâlâ etkin olduğu
        // ekranın varsayımı değil, kasadaki satır olsun.
        void loadConsents(caseId);
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
   * Görünen model yanıtı da bir **okuma**dır: her yeni okuma sayacı artırır,
   * geç yanıt yeni seçimin altına yazılmaz. Düşen okumada satırlar temizlenir
   * ama **"yanıt yok" denmez** — hata kendi satırında yazılır.
   */
  async function loadMessages(forRun: string): Promise<void> {
    if (!forRun) return;
    messagesSeq.current += 1;
    const seq = messagesSeq.current;
    setMessagesBusy(true);
    setMessagesError(null);
    try {
      const rows = await api.aiMessages(forRun);
      if (messagesSeq.current !== seq) return;
      setMessages(rows);
    } catch (e) {
      if (messagesSeq.current !== seq) return;
      setMessages([]);
      setMessagesError(toApiError(e));
    } finally {
      if (messagesSeq.current === seq) setMessagesBusy(false);
    }
  }

  /**
   * F11-T08 — incelenen kapsam ve dayanak da bir **okuma**dır: geç yanıt yeni
   * seçimin altına yazılmaz; düşen okumada "dayanak yok" denmez, hata kendi
   * satırında yazılır. Ölçüm **çekirdekte** yapılır (gönderilen segmentler +
   * alıntı raporu); arayüz tahmin etmez.
   */
  async function loadAnswerBasis(forRun: string): Promise<void> {
    if (!forRun) return;
    answerBasisSeq.current += 1;
    const seq = answerBasisSeq.current;
    setAnswerBasisBusy(true);
    setAnswerBasisError(null);
    try {
      const basis = await api.aiAnswerBasis(forRun);
      if (answerBasisSeq.current !== seq) return;
      setAnswerBasis(basis);
    } catch (e) {
      if (answerBasisSeq.current !== seq) return;
      setAnswerBasis(null);
      setAnswerBasisError(toApiError(e));
    } finally {
      if (answerBasisSeq.current === seq) setAnswerBasisBusy(false);
    }
  }

  /**
   * F11-T11 — **saldırı değerlendirmesi**: talimat taraması gönderilen
   * metinden, sahte kaynak ve şema taşması aday kanıtlarından okunur.
   * Üçü de okuma anında yeniden sayılır; hiçbiri gönderimi engellemez.
   */
  async function loadAttackAssessment(forRun: string): Promise<void> {
    if (!forRun) return;
    attackSeq.current += 1;
    const seq = attackSeq.current;
    setAttackBusy(true);
    setAttackError(null);
    try {
      const report = await api.aiAttackAssessment(forRun);
      if (attackSeq.current !== seq) return;
      setAttack(report);
    } catch (e) {
      if (attackSeq.current !== seq) return;
      setAttack(null);
      setAttackError(toApiError(e));
    } finally {
      if (attackSeq.current === seq) setAttackBusy(false);
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
    // Önceki çalıştırmanın değerlendirmesi yeni seçimin altında okunmasın.
    setAttack(null);
    setAttackError(null);
    await loadCandidates(id);
    if (runSeq.current !== seq) return;
    await loadContextItems(id);
    if (runSeq.current !== seq) return;
    await loadSegments(id);
    if (runSeq.current !== seq) return;
    await loadMessages(id);
    if (runSeq.current !== seq) return;
    await loadAttackAssessment(id);
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
      const next = await api.aiConsentPreview(
        caseId,
        docs.map((doc) => doc.id),
        maskTerms.length > 0 ? maskTerms : null,
        maskPlanId || null,
      );
      // İki okuma yarışırsa **son** istek kazanır; eskisinin geç yanıtı yeni
      // önizlemeyi ne çizer ne siler.
      if (caseGen.current !== gen || previewSeq.current !== seq) return;
      setPreview(next);
      setPreviewMaskSig(maskSig);
      setError(null);
    } catch (e) {
      // Sessiz yutma yok: istek düştüyse söylenir ve eski önizleme silinir.
      if (caseGen.current !== gen || previewSeq.current !== seq) return;
      setPreview(null);
      setPreviewMaskSig(null);
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
                  noText: overview.coverage.documentsWithoutText,
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
              <option value="celiski">{t("ai.purposeContradictions")}</option>
              <option value="soru">{t("ai.purposeAnswer")}</option>
            </select>
          </div>
          {purpose === "soru" ? (
            <div className="field" style={{ maxWidth: 420 }}>
              <label htmlFor="ai-question">{t("ai.questionLabel")}</label>
              <textarea
                id="ai-question"
                className="input"
                rows={2}
                value={question}
                placeholder={t("ai.questionPlaceholder")}
                onChange={(e) => {
                  // Soru değişikliği onayı kapsam dışı bırakmaz (onay amaca
                  // bağlıdır) ama **soruyu** değiştirir: geçersiz soruyla
                  // istek gitmez.
                  setQuestion(e.target.value);
                }}
              />
              <p className="hint" style={{ margin: 0 }}>
                {questionInvalid
                  ? t("ai.questionInvalid")
                  : t("ai.questionHint", { count: questionLength })}
              </p>
            </div>
          ) : null}
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
          <div className="field" style={{ maxWidth: 320 }}>
            <label htmlFor="ai-mask-plan">{t("ai.maskPlanLabel")}</label>
            <select
              id="ai-mask-plan"
              value={maskPlanId}
              onChange={(e) => setMaskPlanId(e.target.value)}
            >
              <option value="">{t("ai.maskPlanNone")}</option>
              {maskPlans.map((plan) => (
                <option key={plan.id} value={plan.id}>
                  {t("ai.maskPlanOption", {
                    title: plan.documentTitle ?? t("common.unknown"),
                    version: plan.versionNo ?? "?",
                    regions: plan.regionCount,
                  })}
                </option>
              ))}
            </select>
            <p className="hint" style={{ margin: 0 }}>
              {maskPlansBusy
                ? t("ai.maskPlanLoading")
                : maskPlansError
                  ? t("ai.maskPlanUnreadable")
                  : maskPlans.length === 0
                    ? t("ai.maskPlanEmpty")
                    : maskPlanId
                      ? t("ai.maskPlanHint")
                      : t("ai.maskPlanHintIdle")}
            </p>
          </div>
          <div className="row" style={{ alignItems: "flex-end", gap: "var(--space-2)" }}>
            <button
              className="btn"
              disabled={!provider || busy || questionInvalid}
              onClick={() => void approve()}
            >
              {t("ai.approve")}
            </button>
            <button className="btn ghost" onClick={() => void showContent()}>
              {t("ai.showContent")}
            </button>
            <button
              className="btn primary"
              disabled={
                !provider ||
                !consent ||
                busy ||
                revokingConsent ||
                consentStale ||
                maskInvalid ||
                questionInvalid ||
                maskChangedSincePreview
              }
              onClick={() => void start()}
            >
              {t("ai.run")}
            </button>
          </div>
          {maskChangedSincePreview ? (
            <div className="notice warn" role="status">
              {t("ai.maskChangedSincePreview")}
            </div>
          ) : null}
          {renewNeeded ? (
            <div className="notice warn" role="status">
              {t("ai.consentRenewNeeded")}
            </div>
          ) : null}
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
            {preview.mask ? (
              <p className="hint" style={{ margin: 0 }}>
                {preview.mask.sampleMasked
                  ? t("ai.previewMaskApplied", {
                      terms: preview.mask.termCount,
                      runs: preview.mask.maskedRuns,
                    })
                  : t("ai.previewMaskNoMatch", { terms: preview.mask.termCount })}
                {" · "}
                {preview.mask.bytesExact
                  ? t("ai.previewMaskBytes", {
                      masked: formatSize(preview.bytes),
                      raw: formatSize(preview.bytesUnmasked),
                    })
                  : t("ai.previewMaskBytesUpper", { raw: formatSize(preview.bytesUnmasked) })}
                {preview.mask.planId ? ` · ${t("ai.previewMaskPlan")}` : ""}
              </p>
            ) : (
              <p className="hint" style={{ margin: 0 }}>{t("ai.previewNoMask")}</p>
            )}
            {preview.instruction ? (
              // F11-T11 — onaydan **önce** görünür: belgeye gömülü talimat
              // benzeri ifadeler. Hiçbir şeyi engellemez.
              <p className={`hint`} style={{ margin: 0 }}>
                {preview.instruction.total > 0
                  ? t("ai.instructionFound", {
                      total: preview.instruction.total,
                      segments: preview.instruction.segments,
                    })
                  : t("ai.instructionClean", { segments: preview.instruction.segments })}
                {preview.instruction.truncated ? ` · ${t("ai.instructionTruncated")}` : ""}
                {preview.instruction.basis === "pre_mask" ? ` · ${t("ai.instructionPreMask")}` : ""}
                {preview.instruction.signals.length > 0
                  ? ` · ${preview.instruction.signals
                      .slice(0, 3)
                      .map((signal) => instructionCodeLabel(signal.code))
                      .join(" · ")}`
                  : ""}
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
                <th>{t("ai.consentsColMask")}</th>
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
                  <td>{consentMaskLabel(row)}</td>
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
                  <td><span className={`badge ${runStateTone(run)}`}>{runStateLabel(run)}</span></td>
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

      <AiQualityCard caseId={caseId} />

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

      {/* F11-T11 — saldırı değerlendirmesi: üç bölüm de okuma anında
          kanıtlarından sayılır; hiçbiri engel değildir. */}
      {runId ? (
        <div className="card">
          <div className="card-head">
            <div>
              <h2>{t("ai.attackTitle")}</h2>
              <p className="card-sub">{t("ai.attackSub")}</p>
            </div>
          </div>
          {attackError ? (
            <div className="notice danger" role="alert">
              <strong>{t("ai.attackError")}</strong> {attackError.code} — {attackError.message}
            </div>
          ) : attack ? (
            <>
              <p style={{ margin: "4px 0" }}>
                <span className={`badge ${attack.instruction.total > 0 ? "warn" : "ok"}`}>
                  {attack.instruction.total > 0
                    ? t("ai.attackInstructionLine", {
                        total: attack.instruction.total,
                        segments: attack.instruction.segments,
                      })
                    : t("ai.attackInstructionClean", {
                        segments: attack.instruction.segments,
                      })}
                </span>
                {attack.instruction.truncated ? (
                  <span className="hint"> · {t("ai.attackInstructionTruncated")}</span>
                ) : null}
              </p>
              {attack.instruction.signals.length > 0 ? (
                <ul className="hint" style={{ margin: "0 0 6px 16px", padding: 0 }}>
                  {attack.instruction.signals.slice(0, 6).map((signal) => (
                    <li key={`${signal.segmentId}-${signal.code}`}>
                      {instructionCodeLabel(signal.code)} ·{" "}
                      {t("ai.instructionSegment", {
                        segment: signal.segmentId,
                        count: signal.occurrences,
                        preview: signal.preview,
                      })}
                    </li>
                  ))}
                </ul>
              ) : null}
              <p style={{ margin: "4px 0" }}>
                <span
                  className={`badge ${
                    attack.fakeSource.notFound > 0
                      ? "danger"
                      : attack.fakeSource.quotes > 0 &&
                          attack.fakeSource.verified === attack.fakeSource.quotes
                        ? "ok"
                        : "plain"
                  }`}
                >
                  {attack.fakeSource.notFound > 0
                    ? t("ai.attackFakeSourceLine", {
                        notFound: attack.fakeSource.notFound,
                        verified: attack.fakeSource.verified,
                        quotes: attack.fakeSource.quotes,
                      })
                    : attack.fakeSource.quotes > 0 &&
                        attack.fakeSource.verified === attack.fakeSource.quotes
                      ? t("ai.attackFakeSourceClean", { quotes: attack.fakeSource.quotes })
                      : t("ai.attackNoCandidates")}
                </span>
                {attack.fakeSource.unverifiable > 0 ? (
                  <span className="hint">
                    {" "}
                    · {t("ai.attackFakeUnverifiable", { count: attack.fakeSource.unverifiable })}
                  </span>
                ) : null}
              </p>
              <p style={{ margin: "4px 0" }}>
                <span className={`badge ${attack.schemaOverflow.overflow > 0 ? "warn" : "plain"}`}>
                  {attack.schemaOverflow.overflow > 0
                    ? t("ai.attackSchemaLine", {
                        overflow: attack.schemaOverflow.overflow,
                        codes: attack.schemaOverflow.codes
                          .map((entry) => `${entry.code} ×${entry.count}`)
                          .join(", "),
                      })
                    : t("ai.attackSchemaClean", { candidates: attack.schemaOverflow.candidates })}
                </span>
              </p>
            </>
          ) : attackBusy ? (
            <p className="hint" style={{ margin: 0 }}>{t("ai.attackLoading")}</p>
          ) : null}
        </div>
      ) : null}

      <div className="card">
        <div className="card-head">
          <div>
            <h2>{t("ai.answerTitle")}</h2>
            <p className="card-sub">{t("ai.answerSub")}</p>
          </div>
        </div>
        {runId && selectedIsAnswer ? (
          // F11-T08 — **incelenen kapsam** ve **dayanak** çekirdekten okunur.
          // Sınırlı yanıt rozeti tahmin değildir: kapsam fiilen gönderilen
          // segmentlerden, dayanak alıntı raporundan gelir.
          <div style={{ marginBottom: "var(--space-2)" }}>
            {answerBasisError ? (
              <div className="notice danger" role="alert">
                <strong>{t("ai.answerBasisError")}</strong> {answerBasisError.code} —{" "}
                {answerBasisError.message}
              </div>
            ) : answerBasis ? (
              <>
                <p className="hint" style={{ margin: 0 }}>
                  {answerBasis.question
                    ? t("ai.answerQuestionLine", { question: answerBasis.question })
                    : t("ai.answerQuestionMissing")}
                </p>
                <p style={{ margin: "4px 0" }}>
                  <span className={`badge ${answerBasisTone(answerBasis.state)}`}>
                    {answerBasisStateLabel(answerBasis.state)}
                  </span>{" "}
                  <span className="hint">
                    {answerBasis.scopeSegments === 0
                      ? t("ai.answerScopeEmpty")
                      : t("ai.answerScopeLine", {
                          documents: answerBasis.scopeDocuments.length,
                          segments: answerBasis.scopeSegments,
                          bytes: formatSize(answerBasis.scopeBytes),
                        })}
                  </span>
                </p>
                {answerBasis.reasons.length > 0 ? (
                  <p className="hint" style={{ margin: 0 }}>
                    {t("ai.answerBasisReasons", {
                      reasons: answerBasis.reasons.map(answerBasisReasonLabel).join(" · "),
                    })}
                  </p>
                ) : null}
                <p className="hint" style={{ margin: 0 }}>
                  {t("ai.answerBasisCounts", {
                    found: answerBasis.quotesFound,
                    total: answerBasis.quotesTotal,
                    verified: answerBasis.quotesVerified,
                  })}
                </p>
              </>
            ) : answerBasisBusy ? (
              <p className="hint" style={{ margin: 0 }}>{t("ai.answerBasisLoading")}</p>
            ) : null}
          </div>
        ) : null}
        {!runId ? (
          <p className="hint" style={{ margin: 0 }}>{t("ai.answerNoRun")}</p>
        ) : messagesError ? (
          // Düşen okuma "yanıt yok" sayılmaz: kendi satırında yazılır.
          <div className="notice danger" role="alert">
            <strong>{t("ai.answerError")}</strong> {messagesError.code} — {messagesError.message}
          </div>
        ) : modelMessages.length === 0 ? (
          <p className="hint" style={{ margin: 0 }}>
            {messagesBusy ? t("ai.answerLoading") : t("ai.answerEmpty")}
          </p>
        ) : (
          <>
            {modelMessages.map((message) => (
              <div key={message.id} style={{ marginBottom: "var(--space-2)" }}>
                <p className="hint" style={{ margin: 0 }}>
                  {t("ai.answerMeta", {
                    role:
                      message.role === "assistant"
                        ? t("ai.answerRoleAssistant")
                        : message.role,
                    state:
                      message.state === "visible"
                        ? t("ai.answerStateVisible")
                        : message.state === "unparsed"
                          ? t("ai.answerStateUnparsed")
                          : message.state === "truncated"
                            ? t("ai.answerStateTruncated")
                            : message.state,
                    refs: message.sourceRefs.length,
                  })}
                </p>
                <pre
                  style={{
                    maxHeight: 220,
                    overflow: "auto",
                    whiteSpace: "pre-wrap",
                    fontSize: 12,
                  }}
                >
                  {message.visibleContent}
                </pre>
              </div>
            ))}
            <p className="hint" style={{ margin: 0 }}>{t("ai.answerNote")}</p>
          </>
        )}
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

            {report.sentenceAttribution ? (
              <SentenceAttributionView attribution={report.sentenceAttribution} />
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
                const answerLimited =
                  selectedIsAnswer === true &&
                  candidate.candidateType === "answer_candidate" &&
                  answerBasis !== null &&
                  answerBasis.state !== "sufficient";
                const blocked = promoteBlockedReason(candidate, answerLimited);
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
                      {candidate.sentenceAttribution ? (
                        <details style={{ marginTop: 4 }}>
                          <summary className="hint">
                            {candidate.sentenceAttribution.state === "unlinked"
                              ? `${t("candidate.attribution.title")} · ${attributionStateLabel("unlinked")}`
                              : `${t("candidate.attribution.title")} · ${attributionSummary(candidate.sentenceAttribution)}`}
                          </summary>
                          <SentenceAttributionView
                            attribution={candidate.sentenceAttribution}
                            compact
                          />
                        </details>
                      ) : null}
                      {candidate.contradictionBasis ? (
                        <details
                          style={{ marginTop: 4 }}
                          open={candidate.contradictionBasis.state !== "presentable"}
                        >
                          <summary className="hint">
                            {`${t("candidate.contradiction.title")} · ${contradictionStateLabel(candidate.contradictionBasis.state)}`}
                          </summary>
                          <ContradictionBasisView
                            basis={candidate.contradictionBasis}
                            quotes={candidate.quoteReport}
                            compact
                          />
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
