//! F11-T12 — model/prompt sürümü bazında kalite ölçümü.
//!
//! Şartname §F11-T12: "Model/prompt sürümü bazında kaynak doğruluğu, öneri
//! isabeti ve düzeltme emeğini ölç." Bu modül ölçümü **kayıtlı kanıtlardan**
//! okur, tahmin etmez ve kalıcı damga yazmaz:
//!
//! - **Kaynak doğruluğu** — her adayın kayıtlı `quote_report`undaki alıntı
//!   hükümleri. `exact`/`normalized` doğrulanmış, `ambiguous`/`page_mismatch`
//!   zayıf, `not_found` bulunamamış sayılır. `unverifiable` (sürüm silinmiş,
//!   maske çözülememiş, belge gönderilmemiş) **paydaya girmez**: modelin
//!   uydurduğunun kanıtı değildir, ayrı sayılır.
//! - **Öneri isabeti** — kuyruğa taşınmış adayların **son** inceleme kararı:
//!   `accept` doğrudan kabul, `edit` düzeltmeyle kabul, `reject` ret,
//!   `request_evidence` ek kanıt. Yerine geçmiş (`superseded`) öğe isabet
//!   oranına girmez; ayrı sayılır (yeni aday onun yerini aldı).
//! - **Düzeltme emeği** — düzenleme kararı sayısı. Bu bir **vekil**
//!   göstergedir: harcanan süre ölçülmez, yalnız «kaç öneri düzeltme istedi»
//!   sayılır.
//!
//! Anahtar `ai_runs.model_snapshot` + `ai_runs.prompt_version`dur; aynı modelin
//! farklı istem sürümleri ayrı satırlarda görünür. Rapor her okumada bu üç
//! kaynaktan yeniden türetilir — `ai_candidates`/`review_items` satırları
//! silinirse sayı düşer, eski bir «doğruluk» iddiası kalmaz.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{Database, StorageError};

/// Ölçüme giren tek bir çalıştırmanın kimlik anahtarı ve durumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiQualityRun {
    pub id: String,
    pub model_snapshot: String,
    pub prompt_version: String,
    pub status: String,
}

/// Ölçüme giren aday: şema doğrulaması ve kayıtlı alıntı hükümleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiQualityCandidate {
    pub model_snapshot: String,
    pub prompt_version: String,
    pub validation_state: String,
    /// Adayın toplu alıntı durumu: `verified` | `partial` | `failed` |
    /// `no_quotes` | `not_checked` | `legacy`.
    pub quote_state: String,
    /// Alıntı **başına** hükümler (`quote_report[].state`), kayıt sırasıyla.
    pub quote_states: Vec<String>,
    /// `quote_report` JSON'u okunamadıysa `true`: hükümler bilinmiyor sayılır,
    /// «alıntı yok» veya «temiz» sanılmaz.
    pub quote_report_unreadable: bool,
}

/// Kuyruğa taşınmış öneri: son kararı ve yerine geçme durumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiQualityItem {
    pub model_snapshot: String,
    pub prompt_version: String,
    pub superseded: bool,
    /// Karar geçmişi **eskiden yeniye**; isabet hükmüne **son** karar girer.
    pub decisions: Vec<String>,
}

/// Tek (model, prompt sürümü) grubunun sayıları. Oran **hesaplanmaz**:
/// arayüz paydaları bu sayılardan kendisi kurar, böylece «ölçülemeyen»
/// (payda 0) durum sessizce %0'a dönüşmez.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQualityGroup {
    pub model_snapshot: String,
    pub prompt_version: String,
    pub runs: i64,
    pub runs_succeeded: i64,
    pub runs_failed: i64,
    /// `queued`/`running`/`cancelled`/`partial` — sonuç üretmemiş çalıştırmalar.
    pub runs_unfinished: i64,
    pub candidates: i64,
    pub candidates_valid: i64,
    pub candidates_partial: i64,
    pub candidates_invalid: i64,
    /// Bilinmeyen doğrulama durumu (beklenmez; sessizce geçerli sayılmaz).
    pub candidates_other: i64,
    /// Alıntısı olmayan aday (`quote_state = no_quotes`).
    pub candidates_no_quotes: i64,
    /// Alıntı doğrulaması yapılmamış/eski kayıt (`not_checked`/`legacy`) ya da
    /// raporu okunamayan aday — paydası bilinmeyen aday.
    pub candidates_unmeasured: i64,
    pub quotes_checked: i64,
    pub quotes_exact: i64,
    pub quotes_normalized: i64,
    pub quotes_ambiguous: i64,
    pub quotes_page_mismatch: i64,
    pub quotes_not_found: i64,
    /// Doğrulanamayan alıntı: `quotes_checked`e **dahil değildir**.
    pub quotes_unverifiable: i64,
    /// Bilinmeyen alıntı hükmü (beklenmez; sessizce doğrulanmış sayılmaz).
    pub quotes_other: i64,
    pub items_promoted: i64,
    pub items_superseded: i64,
    pub decided_accept: i64,
    pub decided_edit: i64,
    pub decided_reject: i64,
    pub decided_request_evidence: i64,
    /// Bilinmeyen karar türü (beklenmez).
    pub decided_other: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQualityReport {
    /// (model, prompt sürümü) sırasıyla **deterministik** gruplar.
    pub groups: Vec<AiQualityGroup>,
    /// Çalıştırması çözülemeyen kuyruk öğeleri (beklenmez; sayılmadan
    /// geçilmez, rapor bunu ayrıca yazar).
    pub unattributed_items: i64,
}

fn group<'a>(
    groups: &'a mut BTreeMap<(String, String), AiQualityGroup>,
    model_snapshot: &str,
    prompt_version: &str,
) -> &'a mut AiQualityGroup {
    groups
        .entry((model_snapshot.to_string(), prompt_version.to_string()))
        .or_insert_with(|| AiQualityGroup {
            model_snapshot: model_snapshot.to_string(),
            prompt_version: prompt_version.to_string(),
            ..AiQualityGroup::default()
        })
}

/// Saf toplama: girdi satırlarından rapor üretir. Girdi sırası sonucu
/// değiştirmez (gruplar `BTreeMap`te, kararlar verilen sırada).
pub fn build_ai_quality_report(
    runs: &[AiQualityRun],
    candidates: &[AiQualityCandidate],
    items: &[AiQualityItem],
) -> AiQualityReport {
    let mut groups: BTreeMap<(String, String), AiQualityGroup> = BTreeMap::new();

    for run in runs {
        let g = group(&mut groups, &run.model_snapshot, &run.prompt_version);
        g.runs += 1;
        match run.status.as_str() {
            "succeeded" => g.runs_succeeded += 1,
            "failed" => g.runs_failed += 1,
            _ => g.runs_unfinished += 1,
        }
    }

    for candidate in candidates {
        let g = group(
            &mut groups,
            &candidate.model_snapshot,
            &candidate.prompt_version,
        );
        g.candidates += 1;
        match candidate.validation_state.as_str() {
            "valid" => g.candidates_valid += 1,
            "partial" => g.candidates_partial += 1,
            "invalid" => g.candidates_invalid += 1,
            _ => g.candidates_other += 1,
        }
        if candidate.quote_report_unreadable {
            g.candidates_unmeasured += 1;
            continue;
        }
        match candidate.quote_state.as_str() {
            "no_quotes" => g.candidates_no_quotes += 1,
            "not_checked" | "legacy" => g.candidates_unmeasured += 1,
            _ => {}
        }
        for state in &candidate.quote_states {
            match state.as_str() {
                "exact" => {
                    g.quotes_exact += 1;
                    g.quotes_checked += 1;
                }
                "normalized" => {
                    g.quotes_normalized += 1;
                    g.quotes_checked += 1;
                }
                "ambiguous" => {
                    g.quotes_ambiguous += 1;
                    g.quotes_checked += 1;
                }
                "page_mismatch" => {
                    g.quotes_page_mismatch += 1;
                    g.quotes_checked += 1;
                }
                "not_found" => {
                    g.quotes_not_found += 1;
                    g.quotes_checked += 1;
                }
                "unverifiable" => g.quotes_unverifiable += 1,
                _ => g.quotes_other += 1,
            }
        }
    }

    for item in items {
        let g = group(&mut groups, &item.model_snapshot, &item.prompt_version);
        g.items_promoted += 1;
        if item.superseded {
            g.items_superseded += 1;
            continue;
        }
        match item.decisions.last().map(String::as_str) {
            Some("accept") => g.decided_accept += 1,
            Some("edit") => g.decided_edit += 1,
            Some("reject") => g.decided_reject += 1,
            Some("request_evidence") => g.decided_request_evidence += 1,
            Some(_) => g.decided_other += 1,
            None => {}
        }
    }

    AiQualityReport {
        groups: groups.into_values().collect(),
        unattributed_items: 0,
    }
}

impl Database {
    /// F11-T12 — dosya kapsamında model/prompt sürümü bazında kalite raporu.
    ///
    /// Kaynaklar `ai_runs` (model/prompt/durum), `ai_candidates`
    /// (doğrulama + `quote_report`) ve `review_items`/`review_decisions`
    /// (son karar). Okuma anında hesaplanır; hiçbir satıra damga yazılmaz.
    pub fn ai_quality_report(&self, case_id: &str) -> Result<AiQualityReport, StorageError> {
        let mut run_stmt = self.conn().prepare(
            "SELECT id,model_snapshot,prompt_version,status FROM ai_runs WHERE case_id=?1",
        )?;
        let mut runs = Vec::new();
        let mut run_keys: HashMap<String, (String, String)> = HashMap::new();
        let run_rows = run_stmt.query_map([case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in run_rows {
            let (id, model_snapshot, prompt_version, status) = row?;
            run_keys.insert(id.clone(), (model_snapshot.clone(), prompt_version.clone()));
            runs.push(AiQualityRun {
                id,
                model_snapshot,
                prompt_version,
                status,
            });
        }

        let mut candidate_stmt = self.conn().prepare(
            "SELECT r.model_snapshot,r.prompt_version,c.validation_state,c.quote_state,c.quote_report
             FROM ai_candidates c JOIN ai_runs r ON r.id=c.ai_run_id
             WHERE r.case_id=?1 ORDER BY c.rowid",
        )?;
        let mut candidates = Vec::new();
        let candidate_rows = candidate_stmt.query_map([case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for row in candidate_rows {
            let (model_snapshot, prompt_version, validation_state, quote_state, report_json) = row?;
            // Raporun **tam** şekline bağlanma: kayıtlı satırdan yalnız alıntı
            // başına hüküm (`state`) okunur. JSON çözülemezse «alıntı yok»
            // denmez; aday **ölçülemedi** sayılır.
            let (quote_states, quote_report_unreadable) =
                match serde_json::from_str::<Vec<serde_json::Value>>(&report_json) {
                    Ok(report) => (
                        report
                            .iter()
                            .filter_map(|quote| quote.get("state").and_then(|state| state.as_str()))
                            .map(str::to_string)
                            .collect(),
                        false,
                    ),
                    Err(_) => (Vec::new(), true),
                };
            candidates.push(AiQualityCandidate {
                model_snapshot,
                prompt_version,
                validation_state,
                quote_state,
                quote_states,
                quote_report_unreadable,
            });
        }

        // Karar geçmişi **eskiden yeniye** sıralanır; hükme son karar girer.
        let mut item_stmt = self.conn().prepare(
            "SELECT i.id,i.ai_run_id,(i.superseded_at IS NOT NULL),d.decision
             FROM review_items i
             LEFT JOIN review_decisions d ON d.review_item_id=i.id
             WHERE i.case_id=?1 AND i.ai_run_id IS NOT NULL
             ORDER BY i.id, d.decided_at, d.id",
        )?;
        let mut items: Vec<AiQualityItem> = Vec::new();
        let mut unattributed_items = 0i64;
        let mut current: Option<(String, AiQualityItem)> = None;
        let item_rows = item_stmt.query_map([case_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        for row in item_rows {
            let (item_id, run_id, superseded, decision) = row?;
            let Some((model_snapshot, prompt_version)) = run_keys.get(&run_id) else {
                unattributed_items += 1;
                continue;
            };
            let flush = match &mut current {
                Some((open_id, _)) if open_id == &item_id => false,
                Some(_) => true,
                None => false,
            };
            if flush {
                if let Some((_, item)) = current.take() {
                    items.push(item);
                }
            }
            let entry = current.get_or_insert_with(|| {
                (
                    item_id.clone(),
                    AiQualityItem {
                        model_snapshot: model_snapshot.clone(),
                        prompt_version: prompt_version.clone(),
                        superseded,
                        decisions: Vec::new(),
                    },
                )
            });
            if let Some(decision) = decision {
                entry.1.decisions.push(decision);
            }
        }
        if let Some((_, item)) = current.take() {
            items.push(item);
        }

        let mut report = build_ai_quality_report(&runs, &candidates, &items);
        report.unattributed_items = unattributed_items;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, NewCase};
    use crate::{DatabaseKey, Sensitivity};

    fn run(model: &str, prompt: &str, status: &str) -> AiQualityRun {
        AiQualityRun {
            id: format!("{model}-{prompt}-{status}"),
            model_snapshot: model.into(),
            prompt_version: prompt.into(),
            status: status.into(),
        }
    }

    fn candidate(
        model: &str,
        prompt: &str,
        validation_state: &str,
        quote_state: &str,
        quote_states: &[&str],
    ) -> AiQualityCandidate {
        AiQualityCandidate {
            model_snapshot: model.into(),
            prompt_version: prompt.into(),
            validation_state: validation_state.into(),
            quote_state: quote_state.into(),
            quote_states: quote_states.iter().map(|state| (*state).into()).collect(),
            quote_report_unreadable: false,
        }
    }

    fn item(model: &str, prompt: &str, superseded: bool, decisions: &[&str]) -> AiQualityItem {
        AiQualityItem {
            model_snapshot: model.into(),
            prompt_version: prompt.into(),
            superseded,
            decisions: decisions
                .iter()
                .map(|decision| (*decision).into())
                .collect(),
        }
    }

    /// Kaynak doğruluğu alıntı **başına** hükümden sayılır; `unverifiable`
    /// paydaya girmez, bilinmeyen hüküm ayrı kovada kalır.
    #[test]
    fn quote_outcomes_are_counted_per_quote_and_unverifiable_stays_out_of_the_denominator() {
        let report = build_ai_quality_report(
            &[run("m1", "p1", "succeeded")],
            &[
                candidate(
                    "m1",
                    "p1",
                    "valid",
                    "partial",
                    &[
                        "exact",
                        "normalized",
                        "ambiguous",
                        "page_mismatch",
                        "not_found",
                    ],
                ),
                candidate("m1", "p1", "partial", "failed", &["unverifiable", "weird"]),
            ],
            &[],
        );
        let g = &report.groups[0];
        assert_eq!(g.candidates, 2);
        assert_eq!(g.candidates_valid, 1);
        assert_eq!(g.candidates_partial, 1);
        assert_eq!(g.quotes_exact, 1);
        assert_eq!(g.quotes_normalized, 1);
        assert_eq!(g.quotes_ambiguous, 1);
        assert_eq!(g.quotes_page_mismatch, 1);
        assert_eq!(g.quotes_not_found, 1);
        assert_eq!(g.quotes_unverifiable, 1);
        assert_eq!(g.quotes_other, 1);
        assert_eq!(
            g.quotes_checked, 5,
            "unverifiable ve bilinmeyen hüküm doğrulama paydasına girmez"
        );
    }

    /// Ölçülemeyen adaylar (eski `legacy`, `not_checked`, okunamayan rapor)
    /// «alıntı yok» veya «temiz» sayılmaz; ayrı sayılır.
    #[test]
    fn unmeasured_candidates_are_separated_from_no_quote_candidates() {
        let mut unreadable = candidate("m1", "p1", "valid", "verified", &["exact"]);
        unreadable.quote_report_unreadable = true;
        let report = build_ai_quality_report(
            &[],
            &[
                candidate("m1", "p1", "valid", "no_quotes", &[]),
                candidate("m1", "p1", "valid", "legacy", &[]),
                candidate("m1", "p1", "valid", "not_checked", &[]),
                unreadable,
            ],
            &[],
        );
        let g = &report.groups[0];
        assert_eq!(g.candidates, 4);
        assert_eq!(g.candidates_no_quotes, 1);
        assert_eq!(g.candidates_unmeasured, 3);
        assert_eq!(g.quotes_checked, 0, "okunamayan raporun hükümleri sayılmaz");
    }

    /// Yerine geçmiş öğe isabet oranına girmez; karar geçmişinde **son** karar
    /// hükme girer (ek kanıt isteyip sonra düzenleme = tek düzenleme).
    #[test]
    fn superseded_items_are_counted_but_excluded_from_hit_rate_and_the_last_decision_wins() {
        let report = build_ai_quality_report(
            &[],
            &[],
            &[
                item("m1", "p1", false, &["request_evidence", "edit"]),
                item("m1", "p1", false, &["accept"]),
                item("m1", "p1", false, &["reject"]),
                item("m1", "p1", false, &[]),
                item("m1", "p1", true, &["accept"]),
            ],
        );
        let g = &report.groups[0];
        assert_eq!(g.items_promoted, 5);
        assert_eq!(g.items_superseded, 1);
        assert_eq!(
            g.decided_edit, 1,
            "son karar düzenleme; ilk ek-kanıt isteği sayılmaz"
        );
        assert_eq!(g.decided_accept, 1, "yerine geçmiş öğenin kabulü sayılmaz");
        assert_eq!(g.decided_reject, 1);
        assert_eq!(g.decided_request_evidence, 0);
    }

    /// Farklı model/prompt çiftleri ayrı gruplarda toplanır ve sıra girdi
    /// sırasından bağımsızdır; boş girdi boş rapor verir.
    #[test]
    fn groups_are_split_by_model_and_prompt_and_sorted_deterministically() {
        let report = build_ai_quality_report(
            &[
                run("mB", "p2", "failed"),
                run("mA", "p1", "succeeded"),
                run("mA", "p1", "running"),
                run("mA", "p2", "cancelled"),
            ],
            &[],
            &[],
        );
        let keys: Vec<(&str, &str)> = report
            .groups
            .iter()
            .map(|g| (g.model_snapshot.as_str(), g.prompt_version.as_str()))
            .collect();
        assert_eq!(keys, vec![("mA", "p1"), ("mA", "p2"), ("mB", "p2")]);
        assert_eq!(report.groups[0].runs, 2);
        assert_eq!(report.groups[0].runs_succeeded, 1);
        assert_eq!(report.groups[0].runs_unfinished, 1);
        assert_eq!(report.groups[1].runs_unfinished, 1);

        let empty = build_ai_quality_report(&[], &[], &[]);
        assert!(empty.groups.is_empty());
        assert_eq!(empty.unattributed_items, 0);
    }

    fn fixture() -> (Database, String) {
        let db = Database::open_in_memory(&DatabaseKey::new([91u8; 32])).expect("db");
        let case = db
            .create_case(&NewCase {
                title: "Kira".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("case");
        (db, case.id)
    }

    fn insert_run(db: &Database, case_id: &str, id: &str, model: &str, prompt: &str, status: &str) {
        db.conn()
            .execute(
                "INSERT INTO ai_runs (id,case_id,task_type,provider_id,model_snapshot,input_manifest_hash,prompt_version,output_schema_version,status,usage_report,created_at,completed_at)
                 VALUES (?1,?2,'summary',NULL,?3,'h',?4,'1',?5,'{\"billing\":\"unknown\"}','2026-11-22T00:00:00.000000Z',NULL)",
                rusqlite::params![id, case_id, model, prompt, status],
            )
            .expect("run");
    }

    fn insert_candidate(
        db: &Database,
        run_id: &str,
        id: &str,
        validation_state: &str,
        quote_state: &str,
        quote_report: &str,
    ) {
        db.conn()
            .execute(
                "INSERT INTO ai_candidates (id,ai_run_id,type,structured_payload,validation_state,validation_errors,schema_state,quote_state,quote_report,checked_at)
                 VALUES (?1,?2,'summary','{}',?3,'[]','valid',?4,?5,NULL)",
                rusqlite::params![id, run_id, validation_state, quote_state, quote_report],
            )
            .expect("candidate");
    }

    fn insert_item(db: &Database, case_id: &str, id: &str, run_id: &str, superseded: bool) {
        db.conn()
            .execute(
                "INSERT INTO review_items (id,case_id,item_kind,payload_schema_version,payload,source_anchor_ids,ai_run_id,state,target_entity_id,created_at,updated_at,row_version,superseded_at)
                 VALUES (?1,?2,'summary_candidate',1,'{}','[]',?3,'pending',NULL,'2026-11-22T00:00:00.000000Z','2026-11-22T00:00:00.000000Z',1,?4)",
                rusqlite::params![id, case_id, run_id, if superseded { Some("2026-11-22T01:00:00.000000Z") } else { None }],
            )
            .expect("item");
    }

    fn insert_decision(db: &Database, id: &str, item_id: &str, decision: &str, at: &str) {
        db.conn()
            .execute(
                "INSERT INTO review_decisions (id,review_item_id,decision,edited_payload,reviewer_label,rationale,decided_at)
                 VALUES (?1,?2,?3,?4,'avukat','',?5)",
                rusqlite::params![
                    id,
                    item_id,
                    decision,
                    if decision == "edit" { Some("{}") } else { None },
                    at
                ],
            )
            .expect("decision");
    }

    /// Uçtan uca: veritabanından okunan satırlar rapora dönüşür; başka dosya
    /// sayılmaz, karar geçmişinde son karar kazanır ve yerine geçmiş öğe
    /// isabet oranına girmez.
    #[test]
    fn report_reads_the_case_scope_and_latest_decisions_from_the_vault() {
        let (db, case_id) = fixture();
        insert_run(&db, &case_id, "r1", "model-a", "p1", "succeeded");
        insert_run(&db, &case_id, "r2", "model-a", "p1", "failed");
        insert_run(&db, &case_id, "r3", "model-b", "p2", "succeeded");
        insert_candidate(
            &db,
            "r1",
            "c1",
            "valid",
            "verified",
            r#"[{"index":0,"documentVersionId":"d","quote":"x","state":"exact"}]"#,
        );
        insert_candidate(&db, "r1", "c2", "invalid", "legacy", "[]");
        insert_candidate(
            &db,
            "r3",
            "c3",
            "partial",
            "partial",
            r#"[{"index":0,"documentVersionId":"d","quote":"x","state":"ambiguous"}]"#,
        );
        insert_item(&db, &case_id, "i1", "r1", false);
        insert_item(&db, &case_id, "i2", "r1", false);
        // `i3` önce **açık** doğar, kararı yazılır, sonra yerine geçmiş
        // işaretlenir: v62 kapısı yerine geçmiş öğeye karar yazılmasını
        // yasakladığı için gerçek sıra budur (önce karar, sonra yerine geçme).
        insert_item(&db, &case_id, "i3", "r3", false);
        insert_decision(
            &db,
            "d1",
            "i1",
            "request_evidence",
            "2026-11-22T01:00:00.000000Z",
        );
        insert_decision(&db, "d2", "i1", "edit", "2026-11-22T02:00:00.000000Z");
        insert_decision(&db, "d3", "i2", "reject", "2026-11-22T03:00:00.000000Z");
        insert_decision(&db, "d4", "i3", "accept", "2026-11-22T04:00:00.000000Z");
        // Karardan **sonra** yerine geçmiş işaretlenir. Yalnız
        // `superseded_at` yazılır: 0041 kapısı bağı (varsa) im ister, imi
        // bağsız kabul eder — yerine geçen öğe silinmiş olabilir.
        db.conn()
            .execute(
                "UPDATE review_items SET superseded_at='2026-11-22T05:00:00.000000Z' WHERE id='i3'",
                [],
            )
            .expect("supersede");

        // Başka dosyanın satırları rapora sızmaz.
        let other = db
            .create_case(&NewCase {
                title: "Diğer".into(),
                case_kind: CaseKind::Review,
                reference_code: None,
                jurisdiction_label: None,
                practice_area: None,
                sensitivity: Sensitivity::Normal,
                default_ai_policy: crate::AiPolicy::Off,
            })
            .expect("other case");
        insert_run(&db, &other.id, "r9", "model-a", "p1", "succeeded");

        let report = db.ai_quality_report(&case_id).expect("rapor");
        assert_eq!(report.unattributed_items, 0);
        assert_eq!(report.groups.len(), 2, "iki model/prompt grubu");
        let a = &report.groups[0];
        assert_eq!(
            (a.model_snapshot.as_str(), a.prompt_version.as_str()),
            ("model-a", "p1")
        );
        assert_eq!(a.runs, 2);
        assert_eq!(a.runs_succeeded, 1);
        assert_eq!(a.runs_failed, 1);
        assert_eq!(a.candidates, 2);
        assert_eq!(a.candidates_valid, 1);
        assert_eq!(a.candidates_invalid, 1);
        assert_eq!(a.candidates_unmeasured, 1, "legacy aday ölçülmedi");
        assert_eq!(a.quotes_checked, 1);
        assert_eq!(a.quotes_exact, 1);
        assert_eq!(a.items_promoted, 2);
        assert_eq!(a.decided_edit, 1, "son karar düzenleme");
        assert_eq!(a.decided_reject, 1);
        assert_eq!(a.decided_request_evidence, 0);

        let b = &report.groups[1];
        assert_eq!(
            (b.model_snapshot.as_str(), b.prompt_version.as_str()),
            ("model-b", "p2")
        );
        assert_eq!(b.candidates_partial, 1);
        assert_eq!(b.quotes_ambiguous, 1);
        assert_eq!(b.items_promoted, 1);
        assert_eq!(b.items_superseded, 1);
        assert_eq!(b.decided_accept, 0, "yerine geçmiş öğe isabete girmez");

        let empty = db.ai_quality_report(&other.id).expect("diğer dosya");
        assert_eq!(empty.groups.len(), 1);
        assert_eq!(empty.groups[0].runs, 1);
        assert_eq!(empty.groups[0].candidates, 0);
    }
}
