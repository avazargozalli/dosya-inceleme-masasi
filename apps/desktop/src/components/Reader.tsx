import { useEffect, useState } from "react";
import { api, toApiError, type ApiError, type DocumentPages } from "../lib/api";

/** Belge okuyucu: sayfa metnini kaynak sırasıyla gösterir. Görüntü render'ı
 *  F07 devamında; bu sürüm metin katmanını gösterir ve uyarıları açık yazar. */
export function Reader({ versionId, title, onClose }: { versionId: string; title: string; onClose: () => void }) {
  const [data, setData] = useState<DocumentPages | null>(null);
  const [page, setPage] = useState(0);
  const [error, setError] = useState<ApiError | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const result = await api.documentPages(versionId);
        setData(result);
      } catch (e) {
        setError(toApiError(e));
      }
    })();
  }, [versionId]);

  const current = data?.pages[page];

  return (
    <div className="card" style={{ borderColor: "var(--line-strong)" }}>
      <div className="card-head">
        <div>
          <h2>{title}</h2>
          <p className="card-sub">
            {data ? `${data.pages.length} sayfa · ${data.parseRun.engine} ${data.parseRun.engineVersion}` : "yükleniyor…"}
          </p>
        </div>
        <button className="btn ghost" onClick={onClose}>Kapat</button>
      </div>

      {error ? (
        <div className="notice danger" role="alert">
          <strong>{error.code}</strong> — {error.message}
        </div>
      ) : null}

      {!data ? (
        <div className="empty"><p style={{ margin: 0 }}>Ayrıştırılmış metin yok. Önce “Ayrıştır” çalıştırın.</p></div>
      ) : (
        <>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            {data.pages.map((p, index) => (
              <button
                key={p.id}
                className={index === page ? "btn primary" : "btn"}
                style={{ height: 28, minWidth: 38, padding: "0 8px" }}
                onClick={() => setPage(index)}
                title={p.textOrigin === "none" ? "metin katmanı yok" : "metin var"}
              >
                {p.pageIndex + 1}
              </button>
            ))}
          </div>

          {current ? (
            <div>
              <div className="row" style={{ justifyContent: "space-between", marginBottom: 8 }}>
                <span className="badge plain">sayfa {current.pageIndex + 1}</span>
                <span className={`badge ${current.textOrigin === "native" ? "ok" : "warn"}`}>
                  {current.textOrigin === "native" ? "metin katmanı" : "metin yok / OCR gerekli"}
                </span>
              </div>
              <div
                style={{
                  background: "var(--surface)",
                  border: "1px solid var(--line)",
                  borderRadius: "var(--radius)",
                  padding: "var(--space-4)",
                  whiteSpace: "pre-wrap",
                  fontFamily: "var(--mono)",
                  fontSize: 12.5,
                  lineHeight: 1.7,
                  maxHeight: 420,
                  overflow: "auto",
                }}
              >
                {current.text.trim() || "(bu sayfada metin katmanı yok)"}
              </div>
            </div>
          ) : null}

          <p className="hint" style={{ margin: 0 }}>
            Sayfa numarası fiziksel sayfa sırasıdır; belgenin basılı numarası farklı olabilir.
            Metin çıkarımı özgün belgenin yerine geçmez.
          </p>
        </>
      )}
    </div>
  );
}
