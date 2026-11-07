import { useCallback, useEffect, useRef, useState } from "react";

import { api, toApiError, type ApiError, type SystemInfo } from "../lib/api";
import {
  SUPPORTED_PROTOCOL,
  parseSystemInfo,
  platformLabel,
  protocolDrift,
  versionDrift,
} from "../lib/diagnostics";
import { t } from "../lib/i18n";

/**
 * Teşhis kartı (REQ-033): sürüm **çekirdekten** okunur.
 *
 * Ölçülen kusur (2026-11-07): `system_info` komutu kayıtlıydı ama hiçbir yerden
 * çağrılmıyordu (ölü yüzey). Ekranda görünen sürüm yalnız `package.json`dan
 * geliyordu — yani **paketlenen arayüzün** sürümü. Çalışan ikilinin sürümü hiç
 * okunmadığı için, ikili ile arayüz ayrışan bir kurulumda (elle kopyalama, yanlış
 * paket, geri alınmış güncelleme) ekran farkında olmadan **yanlış sürüm**
 * yazıyordu ve bunu görecek bir satır yoktu.
 *
 * Sözleşme:
 * - Sürüm/protokol/platform **çekirdekten** (`system_info`) gelir; arayüz sürümü
 *   ayrı satırda durur, ikisi ayrışırsa uyarı basılır. Ayrışma gizlenmez.
 * - Okuma düşerse sürüm **uydurulmaz**: satır "okunamadı" yazar, hata kodu ayrı
 *   satırda görünür (`role="alert"`).
 * - Geç yanıt kapısı (`seqRef`): `main.tsx` StrictMode ile açılıyor, geliştirmede
 *   effect iki kez koşar; yavaş dönen eski okuma yeni değeri ezmemeli.
 * - Kart **kasa durumuna bağlı değildir**: `system_info` kasa açmadan yanıt verir
 *   ve "hangi sürümü kurmuşum" sorusu tam olarak kasa açılamadığında sorulur.
 * - Gizlilik: yalnız sürüm, protokol ve platform; dosya yolu, kasa kimliği,
 *   kullanıcı adı yazılmaz (çekirdek de döndürmez).
 */
export function DiagnosticsCard({ uiVersion }: { uiVersion: string }) {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const seqRef = useRef(0);

  const load = useCallback(async () => {
    const seq = ++seqRef.current;
    setBusy(true);
    setError(null);
    try {
      const next = parseSystemInfo(await api.systemInfo());
      if (seq !== seqRef.current) return;
      if (next === null) {
        // Sözleşme dışı gövde: eksik alanı "sürüm" diye yazmak uydurmak olur.
        setInfo(null);
        setError({
          code: "E_SYSTEM_INFO",
          message: t("settings.diagnosticsBadPayload"),
          retryable: false,
        });
        return;
      }
      setInfo(next);
    } catch (e) {
      if (seq === seqRef.current) {
        // Sürüm uydurulmaz: eski değer de bırakılmaz (hangi okumanın düştüğü
        // belirsizken "son bilinen sürüm" göstermek yanıltıcı olurdu).
        setInfo(null);
        setError(toApiError(e));
      }
    } finally {
      if (seq === seqRef.current) setBusy(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const pending = busy ? t("settings.diagnosticsLoading") : t("settings.diagnosticsNotRead");
  const coreVersion = info === null ? pending : info.appVersion;
  const protocol = info === null ? pending : String(info.protocolVersion);
  const platform = info === null ? pending : platformLabel(info.platform);
  const drifted = info !== null && versionDrift(uiVersion, info.appVersion);
  const protocolMismatch = info !== null && protocolDrift(info.protocolVersion);

  return (
    <div className="card">
      <div className="card-head">
        <div>
          <h2>{t("settings.diagnosticsTitle")}</h2>
          <p className="card-sub">{t("settings.diagnosticsBody")}</p>
        </div>
        <button className="btn ghost" disabled={busy} onClick={() => void load()}>
          {busy ? t("settings.diagnosticsLoading") : t("settings.diagnosticsRefresh")}
        </button>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{t("settings.diagnosticsUnreadable")}</strong>{" "}
          <code>{error.code}</code> — {error.message}
        </div>
      ) : null}

      <dl className="kv">
        <dt>{t("settings.diagnosticsCoreVersion")}</dt>
        <dd style={{ fontFamily: "var(--mono)" }}>{coreVersion}</dd>
        <dt>{t("settings.diagnosticsUiVersion")}</dt>
        <dd style={{ fontFamily: "var(--mono)" }}>{uiVersion}</dd>
        <dt>{t("settings.diagnosticsProtocol")}</dt>
        <dd>{protocol}</dd>
        <dt>{t("settings.diagnosticsPlatform")}</dt>
        <dd>{platform}</dd>
      </dl>

      {drifted ? (
        <div className="notice danger" role="alert">
          {t("settings.diagnosticsDrift")}
        </div>
      ) : null}
      {protocolMismatch && info ? (
        <div className="notice danger" role="alert">
          {t("settings.diagnosticsProtocolMismatch", {
            ui: SUPPORTED_PROTOCOL,
            core: info.protocolVersion,
          })}
        </div>
      ) : null}

      <p className="hint" style={{ margin: 0 }}>{t("settings.diagnosticsPrivacy")}</p>
    </div>
  );
}
