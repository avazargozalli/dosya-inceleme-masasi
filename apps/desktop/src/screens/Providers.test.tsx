// @vitest-environment jsdom
//
// Sağlayıcılar (Providers) ekranı davranış testi. `lib/providers`, `lib/arch`,
// `lib/attack`, `lib/package` testleri metin parçalarını doğrular; burada 1340
// satırlık ekran gerçekten render edilip F10 (CLI yürütme) ve F06/F09 (sağlayıcı
// profili) kararlarının arayüzde uygulandığı ölçülür:
//
// 1. API profili: iki alan dolmadan "Kaydet" kapalı; kaydetme tam sözleşmeyle
//    gider; hata sessiz yutulmaz (iletisi görünür).
// 2. Sağlayıcı satırları sunucudan: durum/mimari rozetleri, sürüm, yol, kimlik
//    kipi ve izinli argümanlar. Eksik alan **uydurulmaz** ("bilinmiyor",
//    "PATH'te değil").
// 3. Yoklama kapısı: yalnız kurulu ve toplu çağrısı yasak olmayan satır
//    yoklanabilir; yasak satırda neden, kurulu olmayanda "kullanılamaz" yazılır.
// 4. Yoklama: çalışırken yalnız o satır kilitlenir, "Vazgeç" çekirdeğe gider ve
//    sonucu (signalled / nothing) yazar.
// 5. Yoklama sonucu kapsam kanıtıyla yazılır: komut, ortam anahtarları, izin
//    kipi, `HOME` politikası, stdin boyutu, kum havuzu dosyaları, çıktı akışları;
//    çıktı yoksa açıkça söylenir.
// 6. **Ölçülen kusur:** başarısız yoklama, önceki başarılı yoklamanın sonucunu
//    ekranda bırakıyordu (aynı satırda yeşil sonuç + kırmızı hata).
// 7. **Ölçülen kusur:** "Yenile" hatası başarılı yenilemeden sonra silinmiyordu
//    (sayfa kalıcı kırmızı bildirimle kalıyordu).
// 8. Yetenek matrisi: sürüm/yardım komutu, satır durumları, kapı ve en kötü
//    bağ; sınırlar listesi. Koşu sırasında diğer CLI koşuları kilitlidir.
// 9. **Ölçülen kusur:** matris koşusu düşerse önceki matris raporu ekranda
//    kalıyordu (bayat rapor + kırmızı hata).
// 11. Paket raporu: doğrulama sayaçları, sorunlar/fazlalıklar, roller, tesseract
//     ve sınırlar; `found=false` hata değil, geliştirme kipi olarak yazılır.
// 12. Saldırı sınaması: fixture kanıtı, ürün yapılandırması, isabet/atlandı
//     rozetleri ve vektör hükümleri.
// 13. Mimari raporu: biçim, mimari karşılaştırması, yorumlayıcı, neden ve başlık
//     baytları (0 ise "bilinmiyor").
// 14. Kalıntı taraması: kök, toplam, kalıcılık ve yol satırları.
// 15. Kalıntı ölçümü **açık onay** ister; temizlik yalnız ölçümde oluşan
//     dosyaları hedefler ve sonrası yeniden taranır.
// 16. Eski kum havuzu süpürmesi özetlenir; atlananlar gizlenmez.
// 17. **Ölçülen kusur:** adaptör değişince kalıntı panelinin önceki hata
//     bildirimi ekranda kalıyordu.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AdapterArch,
  AdapterView,
  CliArchReport,
  CliAttackReport,
  CliCapabilityMatrix,
  CliProbeView,
  CliResidueCleanup,
  CliResidueMeasure,
  CliResidueScan,
  CliSandboxSweep,
  WorkerPackageView,
} from "../lib/api";
import { formatSize, formatUnixSeconds } from "../lib/format";
import { t } from "../lib/i18n";
import {
  adapterArchBadge,
  archBadgeText,
  archFormatText,
  archInterpreterText,
  archList,
  archPathText,
  archProbedAt,
  archReasonText,
  archSummaryText,
  archWarnings,
  archWorst,
} from "../lib/arch";
import {
  attackAdapterWarnings,
  attackCommand,
  attackFixtureSummary,
  attackFixtureWarnings,
  attackGuardText,
  attackProductText,
  attackSkipText,
  attackTotalHits,
  attackVerdictText,
  attackWorstVerdict,
} from "../lib/attack";
import {
  packageLimits,
  packageStatusText,
  packageSummary,
  packageWarnings,
  problemText,
  roleRows,
  verificationCounts,
} from "../lib/package";
import {
  capabilityGateText,
  capabilityHelpCommand,
  capabilitySummary,
  capabilityWarnings,
  matrixWorstLink,
  persistenceText,
  probeArgv,
  probeOutputs,
  probeScopeLines,
  probeStatusLabel,
  probeSummary,
  residueAge,
  residueCleanupSummary,
  residueMeasureSummary,
  residuePathLine,
  sandboxSweepSummary,
} from "../lib/providers";

const mocks = vi.hoisted(() => ({
  providerProbe: vi.fn(),
  providerProbeRun: vi.fn(),
  providerProbeCancel: vi.fn(),
  providerApiProfileSave: vi.fn(),
  cliCapabilityMatrix: vi.fn(),
  cliAttackProbe: vi.fn(),
  cliArchReport: vi.fn(),
  workerPackageReport: vi.fn(),
  cliResidueScan: vi.fn(),
  cliResidueMeasure: vi.fn(),
  cliResidueCleanup: vi.fn(),
  cliStaleSandboxSweep: vi.fn(),
}));

vi.mock("../lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/api")>();
  return { ...actual, api: { ...actual.api, ...mocks } };
});

const { Providers } = await import("./Providers");

/** Bağlantılı (aynı mimari) kurulu sağlayıcı. */
const ARCH_SAME: AdapterArch = {
  format: "mach-o",
  formatLabel: "Mach-O",
  architectures: ["arm64"],
  comparison: "same",
  comparisonLabel: "uygulamayla aynı",
  containsAppArch: true,
  reason: null,
  summary: "arm64 · uygulamayla aynı",
};

const CLAUDE: AdapterView = {
  id: "claude-code",
  displayName: "Claude Code",
  command: "claude",
  state: "installed",
  version: "2.1.303",
  authMode: "subscription",
  batchForbidden: false,
  allowedArgs: ["--print", '--tools ""'],
  path: "/opt/homebrew/bin/claude",
  architecture: ARCH_SAME,
};

/** Mimari okunamayan, kimlik doğrulaması yalnız etkileşimli sağlayıcı. */
const ANTIGRAVITY: AdapterView = {
  id: "antigravity",
  displayName: "Antigravity",
  command: "antigravity",
  state: "unverified",
  version: null,
  authMode: "interactive_only",
  batchForbidden: false,
  allowedArgs: [],
  path: null,
  architecture: null,
};

/** Kurulu ama toplu çağrısı yasak (API anahtarı kimliği). */
const KIMI: AdapterView = {
  id: "kimi",
  displayName: "Kimi",
  command: "kimi",
  state: "installed",
  version: "1.2.0",
  authMode: "api_key",
  batchForbidden: true,
  allowedArgs: ["--print"],
  path: "/usr/local/bin/kimi",
  architecture: {
    ...ARCH_SAME,
    format: "elf",
    formatLabel: "ELF",
    architectures: ["x86_64"],
    comparison: "different",
    comparisonLabel: "uygulamadan farklı",
    containsAppArch: false,
    reason: "x86_64_under_arm64",
    summary: "x86_64 · uygulamadan farklı",
  },
};

/** Hiç kurulu olmayan sağlayıcı: yoklanamaz, eksik alan uydurulmaz. */
const GEMINI: AdapterView = {
  id: "gemini",
  displayName: "Gemini",
  command: "gemini",
  state: "not_installed",
  version: null,
  authMode: "subscription",
  batchForbidden: false,
  allowedArgs: [],
  path: null,
  architecture: null,
};

function probe(overrides: Partial<CliProbeView> = {}): CliProbeView {
  return {
    adapterId: "claude-code",
    displayName: "Claude Code",
    executable: "/opt/homebrew/bin/claude",
    args: ["--print", "merhaba"],
    envKeys: ["PATH", "HOME"],
    workdir: "/tmp/dim-probe-abc",
    timeoutMs: 20000,
    status: "exited",
    exitCode: 0,
    durationMs: 812,
    stdoutHead: "merhaba dünya\n",
    stderrHead: "",
    stdoutTruncated: false,
    stderrTruncated: false,
    sandbox: {
      workdir: "/tmp/dim-probe-abc",
      workdirMode: "0700",
      home: "/tmp/dim-probe-abc/home",
      tmp: "/tmp/dim-probe-abc/tmp",
      homeIsolation: "sandbox",
      stdinBytes: 13,
      stdinLimitBytes: 1048576,
      files: [{ name: "out.txt", bytes: 12 }],
      filesTruncated: false,
      totalBytes: 12,
      sizeLimitBytes: 1048576,
      entryLimit: 100,
    },
    ...overrides,
  };
}

const MATRIX: CliCapabilityMatrix = {
  probedAtUnix: 1770000000,
  helpTimeoutMs: 8000,
  notes: ["Yetenek kanıtı yalnız `--help` metnidir.", "Kabuk açılış dosyaları okunmaz."],
  adapters: [
    {
      adapterId: "claude-code",
      displayName: "Claude Code (matris)",
      command: "claude",
      version: "2.1.303",
      helpArgs: ["--help"],
      helpRead: true,
      helpError: null,
      link: "declared",
      linkLabel: "bildirildi",
      gateOpen: true,
      gateReasons: [],
      rows: [
        {
          capability: "print",
          label: "Toplu yazdırma",
          required: true,
          note: "tek seferlik istem",
          state: "declared",
          stateLabel: "bildirildi",
          flags: ["--print"],
          detail: "--print",
        },
        {
          capability: "mcp",
          label: "MCP",
          required: false,
          note: "kapalı tutulur",
          state: "missing",
          stateLabel: "yok",
          flags: [],
          detail: "",
        },
      ],
    },
    {
      adapterId: "gemini",
      displayName: "Gemini (matris)",
      command: "gemini",
      version: null,
      helpArgs: ["--help"],
      helpRead: false,
      helpError: "süreç başlatılamadı",
      link: "not_installed",
      linkLabel: "kurulu değil",
      gateOpen: false,
      gateReasons: ["path yok"],
      rows: [
        {
          capability: "print",
          label: "Toplu yazdırma",
          required: true,
          note: "tek seferlik istem",
          state: "unread",
          stateLabel: "okunamadı",
          flags: [],
          detail: "",
        },
      ],
    },
  ],
};

const ARCH_REPORT: CliArchReport = {
  appArch: "arm64",
  probedAtUnix: 1770000600,
  same: 1,
  different: 1,
  fat: 0,
  unknown: 0,
  notes: ["Başlık baytları 4096 baytla sınırlıdır."],
  adapters: [
    {
      adapterId: "claude-code",
      displayName: "Claude Code",
      command: "claude",
      installed: true,
      path: "/opt/homebrew/bin/claude",
      resolved: "/opt/homebrew/Cellar/claude/2.1.303/bin/claude",
      symlink: true,
      format: "mach-o",
      formatLabel: "Mach-O",
      architectures: ["arm64"],
      comparison: "same",
      comparisonLabel: "uygulamayla aynı",
      containsAppArch: true,
      summary: "arm64 · uygulamayla aynı",
      interpreter: {
        command: "#!",
        path: "/usr/bin/env",
        resolved: "/usr/bin/env",
        format: "script",
        architectures: ["script"],
        comparison: "unknown",
        reason: null,
      },
      reason: null,
      headerBytes: 32,
    },
    {
      adapterId: "kimi",
      displayName: "Kimi",
      command: "kimi",
      installed: true,
      path: "/usr/local/bin/kimi",
      resolved: null,
      symlink: false,
      format: "elf",
      formatLabel: "ELF",
      architectures: ["x86_64"],
      comparison: "different",
      comparisonLabel: "uygulamadan farklı",
      containsAppArch: false,
      summary: "x86_64 · uygulamadan farklı",
      interpreter: null,
      reason: "x86_64_under_arm64",
      headerBytes: 0,
    },
    {
      adapterId: "gemini",
      displayName: "Gemini",
      command: "gemini",
      installed: false,
      path: null,
      resolved: null,
      symlink: false,
      format: "unknown",
      formatLabel: "bilinmiyor",
      architectures: [],
      comparison: "unknown",
      comparisonLabel: "belirsiz",
      containsAppArch: null,
      summary: "kurulu değil",
      interpreter: null,
      reason: "not_installed",
      headerBytes: 0,
    },
  ],
};

const ATTACK: CliAttackReport = {
  probedAtUnix: 1770000900,
  notes: ["Saldırı fikstürü kum havuzunda üretilir."],
  fixture: {
    detectorProven: true,
    detectorControlClean: false,
    pollutedHits: 3,
    cleanHits: 0,
    vectorTotal: 2,
    gateRejected: true,
    gateCode: "E_CLI_GATE",
    canaryReadInProductConfig: true,
    productConfig: [
      { vector: "home_file", label: "Ev dizini dosyası", hit: true },
      { vector: "env_leak", label: "Ortam sızıntısı", hit: false },
    ],
  },
  adapters: [
    {
      adapterId: "claude-code",
      displayName: "Claude Code (saldırı)",
      version: "2.1.303",
      argv: ["claude", "--print", "oku"],
      status: "exited",
      exitCode: 0,
      durationMs: 934,
      hits: 1,
      helpRead: true,
      helpError: null,
      link: "declared",
      linkLabel: "bildirildi",
      gateReasons: [],
      skipped: null,
      vectors: [
        {
          vector: "home_file",
          label: "Ev dizini dosyası",
          note: "kum havuzu dışına yazma denemesi",
          guard: "declared",
          guardLabel: "bildirildi",
          guardFlags: ["--tools \"\""],
          hit: true,
          source: "marker",
          sourceLabel: "işaret",
          verdict: "triggered",
          verdictLabel: "tetiklendi",
        },
        {
          vector: "env_leak",
          label: "Ortam sızıntısı",
          note: "anahtar ortamdan okunur",
          guard: "missing",
          guardLabel: "yok",
          guardFlags: [],
          hit: false,
          source: "none",
          sourceLabel: "yok",
          verdict: "silent_guarded",
          verdictLabel: "sessiz · korumalı",
        },
      ],
    },
    {
      adapterId: "kimi",
      displayName: "Kimi (saldırı)",
      version: null,
      argv: ["kimi", "--print", "oku"],
      status: "skipped",
      exitCode: null,
      durationMs: 0,
      hits: 0,
      helpRead: false,
      helpError: null,
      link: "limited",
      linkLabel: "sınırlı",
      gateReasons: ["kimlik"],
      skipped: "etkileşimli_only",
      vectors: [],
    },
  ],
};

const PKG: WorkerPackageView = {
  found: true,
  packageDir: "/repo/resources/worker",
  note: null,
  tesseractOnPath: true,
  manifest: {
    workerVersion: "0.4.0",
    mode: "onedir",
    buildPlatform: "macos-arm64",
    pythonVersion: "3.12.7",
    pyinstallerVersion: "6.11.1",
    entryPoint: "dim-worker",
    totals: { files: 118, symlinks: 18, bytes: 79272345, truncated: false, unsafeLinks: 0 },
    roles: { python_runtime: 12, pdf_engine: 7, other: 3 },
    tesseract: { bundled: false, note: "sistem tesseract kullanılır" },
  },
  verification: {
    verified: 116,
    problemCount: 1,
    problems: [{ path: "lib/libpdf.dylib", problem: "hashChanged", detail: "sha256 farklı" }],
    extraCount: 1,
    extra: ["lib/eski.dylib"],
    totalBytes: 79272345,
    entryPointPresent: true,
    truncated: false,
    ok: false,
  },
};

const PKG_MISSING: WorkerPackageView = {
  found: false,
  packageDir: null,
  note: "paket bu derlemede gömülü değil",
  manifest: null,
  verification: null,
  tesseractOnPath: false,
};

const SCAN: CliResidueScan = {
  adapterId: "claude-code",
  root: "/Users/avukat",
  totalFiles: 4,
  totalBytes: 5120,
  truncated: false,
  fileLimit: 2000,
  persistenceFlag: "--no-session-persistence",
  persistenceAppliesTo: "claude-code",
  persistenceVerifiedWith: "2.1.303",
  paths: [
    {
      relative: ".claude/sessions",
      kind: "dizin",
      note: "oturum dosyaları",
      exists: true,
      files: 4,
      bytes: 5120,
      truncated: false,
      newestAgeSecs: 120,
    },
    {
      relative: ".claude/cache",
      kind: "dizin",
      note: "önbellek",
      exists: false,
      files: 0,
      bytes: 0,
      truncated: false,
      newestAgeSecs: null,
    },
  ],
};

const MEASURE: CliResidueMeasure = {
  adapterId: "claude-code",
  root: "/Users/avukat",
  measuredAtUnix: 1770001200,
  usedUserHome: false,
  status: "exited",
  exitCode: 0,
  durationMs: 640,
  args: ["--print", "merhaba"],
  before: SCAN,
  after: { ...SCAN, totalFiles: 5, totalBytes: 6144 },
  created: [{ relative: ".claude/sessions/yeni.jsonl", bytes: 1024 }],
  modified: [],
  removed: [],
  createdDirs: [".claude/sessions"],
  bytesAdded: 1024,
  cleanupHint: "Yalnız bu ölçümde oluşan dosyalar silinebilir.",
};

const CLEANUP: CliResidueCleanup = {
  removedFiles: 1,
  removedBytes: 1024,
  removedDirs: 1,
  skipped: [{ path: ".claude/sessions/eski.jsonl", reason: "ölçümde oluşmadı" }],
  errors: [],
};

const SWEEP: CliSandboxSweep = {
  considered: 3,
  removed: ["/tmp/dim-probe-old"],
  skipped: [{ path: "/tmp/dim-probe-live", reason: "son 1 saat içinde" }],
  minAgeSecs: 3600,
};

type SetupOptions = {
  rows?: AdapterView[];
  probeRun?: (adapterId: string) => Promise<CliProbeView>;
  probeCancel?: () => Promise<boolean>;
  probeList?: () => Promise<AdapterView[]>;
  matrix?: () => Promise<CliCapabilityMatrix>;
  attack?: () => Promise<CliAttackReport>;
  arch?: () => Promise<CliArchReport>;
  pkg?: () => Promise<WorkerPackageView>;
  scan?: (adapterId: string) => Promise<CliResidueScan>;
  measure?: (adapterId: string, confirm: boolean) => Promise<CliResidueMeasure>;
  cleanup?: (adapterId: string, at: number, paths: string[]) => Promise<CliResidueCleanup>;
  sweep?: () => Promise<CliSandboxSweep>;
};

function setup(options: SetupOptions = {}) {
  mocks.providerProbe.mockImplementation(options.probeList ?? (async () => options.rows ?? [CLAUDE, ANTIGRAVITY, KIMI, GEMINI]));
  mocks.providerProbeRun.mockImplementation(options.probeRun ?? (async () => probe()));
  mocks.providerProbeCancel.mockImplementation(options.probeCancel ?? (async () => true));
  mocks.providerApiProfileSave.mockResolvedValue(undefined);
  mocks.cliCapabilityMatrix.mockImplementation(options.matrix ?? (async () => MATRIX));
  mocks.cliAttackProbe.mockImplementation(options.attack ?? (async () => ATTACK));
  mocks.cliArchReport.mockImplementation(options.arch ?? (async () => ARCH_REPORT));
  mocks.workerPackageReport.mockImplementation(options.pkg ?? (async () => PKG));
  mocks.cliResidueScan.mockImplementation(options.scan ?? (async () => SCAN));
  mocks.cliResidueMeasure.mockImplementation(options.measure ?? (async () => MEASURE));
  mocks.cliResidueCleanup.mockImplementation(options.cleanup ?? (async () => CLEANUP));
  mocks.cliStaleSandboxSweep.mockImplementation(options.sweep ?? (async () => SWEEP));
  render(<Providers />);
}

function esc(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Tam metin eşleşmesi: ata düğümler daha uzun metin taşıdığı için eşleşmez. */
function exact(value: string): RegExp {
  return new RegExp(`^${esc(value)}$`);
}

/**
 * `textContent` tam eşleşmesi. RTL'in varsayılan `getNodeText`i yalnız **doğrudan
 * metin düğümlerini** birleştirir; `<strong>`/`<code>` içeren satırlarda bu yüzden
 * metin "bölünmüş" görünür. Bu eşleştirici alt ağacın tamamını karşılaştırır.
 */
function byTextContent(value: string) {
  return (_content: string, element: Element | null): boolean => {
    if (element === null) return false;
    return (element.textContent ?? "").replace(/\s+/g, " ").trim() === value;
  };
}

/** Sağlayıcı satırının kartı (`<h2>` başlığından yukarı). */
function adapterCard(displayName: string): HTMLElement {
  const heading = screen.getByRole("heading", { level: 2, name: displayName });
  const card = heading.closest(".card");
  if (!(card instanceof HTMLElement)) throw new Error(`${displayName} kartı yok`);
  return card;
}

function button(name: string, scope: HTMLElement = document.body): HTMLButtonElement {
  const found = within(scope).getByRole("button", { name });
  return found as HTMLButtonElement;
}

async function ready(displayName = "Claude Code") {
  await screen.findByRole("heading", { level: 2, name: displayName });
}

describe("sağlayıcılar ekranı davranışı", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("API profili iki alan dolmadan kaydedilmez; kaydetme tam sözleşmeyle gider, hata sessiz yutulmaz", async () => {
    setup();
    await ready();

    const save = button(t("common.save"));
    expect(save.disabled).toBe(true);

    fireEvent.change(screen.getByLabelText(t("providers.endpointAria")), {
      target: { value: "https://ai.test/v1" },
    });
    expect(button(t("common.save")).disabled).toBe(true); // model boş

    fireEvent.change(screen.getByLabelText(t("providers.modelAria")), {
      target: { value: "model-x" },
    });
    expect(button(t("common.save")).disabled).toBe(false);
    fireEvent.click(button(t("common.save")));

    await waitFor(() =>
      expect(mocks.providerApiProfileSave).toHaveBeenCalledWith("https://ai.test/v1", "model-x"),
    );
    expect(await screen.findByText(t("providers.profileSaved"))).toBeTruthy();

    // Hata sessizce yutulmaz: çekirdeğin iletisi ekranda kalır.
    mocks.providerApiProfileSave.mockRejectedValueOnce(
      new Error("E_AI_PROFILE: uç nokta https olmalı"),
    );
    fireEvent.click(button(t("common.save")));
    expect(await screen.findByText(/E_AI_PROFILE/)).toBeTruthy();
  });

  it("sağlayıcı satırları sunucudan yazılır; eksik sürüm/yol/mimari/argüman uydurulmaz", async () => {
    setup();
    await ready();

    // Dört satır, durum rozetleri çekirdekten: 2 kurulu, 1 sınırlı, 1 eksik.
    expect(screen.getAllByText(t("providers.stateInstalled"))).toHaveLength(2);
    expect(screen.getAllByText(t("providers.stateLimited"))).toHaveLength(1);
    expect(screen.getAllByText(t("providers.stateMissing"))).toHaveLength(1);

    const claude = adapterCard("Claude Code");
    expect(within(claude).getByText("2.1.303")).toBeTruthy();
    expect(within(claude).getByText("/opt/homebrew/bin/claude")).toBeTruthy();
    expect(within(claude).getByText(t("providers.authSubscription"))).toBeTruthy();
    // İzinli argümanlar boşlukla birleşir; tırnaklı bayrak olduğu gibi kalır.
    expect(within(claude).getByText('--print --tools ""')).toBeTruthy();
    // Mimari satırı lib'den gelir: "arm64 · <rozet>".
    expect(
      within(claude).getByText(
        exact(`${archList(CLAUDE.architecture!.architectures)} · ${archBadgeText(CLAUDE.architecture!)}`),
      ),
    ).toBeTruthy();

    // Sürümü/yolu/mimarisi olmayan satırlarda uydurma yok.
    const antigravity = adapterCard("Antigravity");
    expect(within(antigravity).getAllByText(t("common.unknown")).length).toBeGreaterThan(0);
    expect(within(antigravity).getByText(t("providers.pathNotOnPath"))).toBeTruthy();
    expect(within(antigravity).getByText(t("providers.authInteractiveOnly"))).toBeTruthy();

    const gemini = adapterCard("Gemini");
    expect(within(gemini).getByText(t("providers.stateMissing"))).toBeTruthy();
    expect(within(gemini).getByText(t("providers.pathNotOnPath"))).toBeTruthy();

    const kimi = adapterCard("Kimi");
    expect(within(kimi).getByText(t("providers.authApiKey"))).toBeTruthy();
    // Farklı mimari: rozet etiketi lib'den gelir (reason: x86_64_under_arm64).
    expect(within(kimi).getAllByText(adapterArchBadge(KIMI.architecture).label).length).toBeGreaterThan(0);
  });

  it("yoklama kapısı: kurulu olmayan ve toplu çağrısı yasak satırda 'Yokla' kapalı, neden yazılır", async () => {
    setup();
    await ready();

    expect(button(t("providers.probeRun"), adapterCard("Claude Code")).disabled).toBe(false);

    const kimi = adapterCard("Kimi");
    expect(button(t("providers.probeRun"), kimi).disabled).toBe(true);
    expect(within(kimi).getByText(t("providers.batchForbidden"))).toBeTruthy();

    const gemini = adapterCard("Gemini");
    expect(button(t("providers.probeRun"), gemini).disabled).toBe(true);
    expect(within(gemini).getByText(t("providers.probeUnavailable"))).toBeTruthy();

    const antigravity = adapterCard("Antigravity");
    expect(button(t("providers.probeRun"), antigravity).disabled).toBe(true);
    expect(within(antigravity).getByText(t("providers.probeUnavailable"))).toBeTruthy();
  });

  it("yoklama çalışırken yalnız o satır kilitlenir; vazgeçme çekirdeğe gider ve sonucu yazılır", async () => {
    let resolveProbe: (value: CliProbeView) => void = () => {};
    setup({
      probeRun: () =>
        new Promise<CliProbeView>((resolve) => {
          resolveProbe = resolve;
        }),
    });
    await ready();

    const claude = adapterCard("Claude Code");
    fireEvent.click(button(t("providers.probeRun"), claude));

    // Çalışırken etiket değişir ve "Vazgeç" görünür; diğer satırlar kilitli.
    expect(await within(claude).findByRole("button", { name: t("providers.probing") })).toBeTruthy();
    expect(button(t("providers.probeCancel"), claude)).toBeTruthy();
    expect(button(t("providers.probeRun"), adapterCard("Antigravity")).disabled).toBe(true);

    fireEvent.click(button(t("providers.probeCancel"), claude));
    await waitFor(() => expect(mocks.providerProbeCancel).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(t("providers.cancelSignalled"))).toBeTruthy();

    resolveProbe(probe());
    await within(claude).findByText(probeStatusLabel(probe()).label);
    // Vazgeç düğmesi iş bitince kalkar.
    expect(within(claude).queryByRole("button", { name: t("providers.probeCancel") })).toBeNull();
  });

  it("yoklama sonucu kapsam kanıtıyla yazılır; çıktı yoksa açıkça söylenir", async () => {
    mocks.providerProbeRun
      .mockResolvedValueOnce(probe())
      .mockResolvedValueOnce(
        probe({
          stdoutHead: "   ",
          stderrHead: "",
          sandbox: { ...probe().sandbox, files: [], filesTruncated: true, totalBytes: 0 },
        }),
      );
    setup();
    await ready();

    const claude = adapterCard("Claude Code");
    fireEvent.click(button(t("providers.probeRun"), claude));

    expect(await within(claude).findByText(probeStatusLabel(probe()).label)).toBeTruthy();
    expect(within(claude).getByText(probeSummary(probe()))).toBeTruthy();
    expect(within(claude).getByText(probeArgv(probe()))).toBeTruthy();
    expect(within(claude).getByText("PATH, HOME")).toBeTruthy();
    // Kapsam satırları: izin kipi, HOME politikası, stdin boyutu, dosya sayısı.
    for (const line of probeScopeLines(probe())) {
      expect(within(claude).getByText(line.label)).toBeTruthy();
      expect(within(claude).getByText(line.value)).toBeTruthy();
    }
    expect(within(claude).getByText(t("providers.workdirNote"))).toBeTruthy();
    // Kum havuzunda bırakılan dosya adı + boyutu.
    expect(within(claude).getByText(`${t("probe.filesWithSize", { count: 1, size: formatSize(12) })}`)).toBeTruthy();
    const outputs = probeOutputs(probe());
    expect(outputs).toHaveLength(1); // yalnız stdout dolu
    expect(within(claude).getByText("stdout")).toBeTruthy();
    expect(claude.querySelector("pre")?.textContent).toBe(outputs[0]!.text);

    // İkinci koşu: çıktı yok → "Çıktı yok." yazılır, eski stdout kalmaz.
    await waitFor(() => expect(button(t("providers.probeRun"), claude).disabled).toBe(false));
    fireEvent.click(button(t("providers.probeRun"), claude));
    expect(await within(claude).findByText(t("providers.noOutput"))).toBeTruthy();
    expect(claude.querySelector("pre")).toBeNull();
  });

  it("başarısız yoklama önceki başarılı sonucu ekranda bırakmaz", async () => {
    mocks.providerProbeRun
      .mockResolvedValueOnce(probe())
      .mockRejectedValueOnce({ code: "E_CLI_PROBE", message: "süreç başlatılamadı" });
    setup();
    await ready();

    const claude = adapterCard("Claude Code");
    fireEvent.click(button(t("providers.probeRun"), claude));
    await within(claude).findByText(probeArgv(probe()));

    await waitFor(() => expect(button(t("providers.probeRun"), claude).disabled).toBe(false));
    fireEvent.click(button(t("providers.probeRun"), claude));
    expect(await within(claude).findByText("E_CLI_PROBE")).toBeTruthy();
    // Aynı satırda hem yeşil sonuç hem kırmızı hata görünmemeli.
    expect(within(claude).queryByText(probeArgv(probe()))).toBeNull();
    expect(within(claude).queryByText(probeStatusLabel(probe()).label)).toBeNull();
  });

  it("yenileme hatası başarılı yenilemeden sonra silinir", async () => {
    mocks.providerProbe
      .mockRejectedValueOnce({ code: "E_CLI_LIST", message: "liste okunamadı" })
      .mockResolvedValueOnce([CLAUDE, ANTIGRAVITY, KIMI, GEMINI]);
    setup();

    // İlk liste isteği düştü: kart yok, kırmızı bildirim var.
    expect((await screen.findByRole("alert")).textContent).toContain("E_CLI_LIST");

    fireEvent.click(button(t("providers.refresh")));
    await waitFor(() => expect(mocks.providerProbe).toHaveBeenCalledTimes(2));
    await ready("Kimi");
    // Liste tazelendi; eski hata bildirimi ekranda kalmamalı.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("yetenek matrisi çekirdekten gelir: sürüm, yardım komutu, satır durumları, kapı ve sınırlar", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.matrixRun")));
    const worst = matrixWorstLink(MATRIX);
    await screen.findByRole("heading", { level: 3, name: "Claude Code (matris)" });

    // "Yoklandı: … · yardım sınırı 8 sn · en kötü durum: X" satırı lib'den birebir.
    expect(
      screen.getByText(
        byTextContent(
          `${t("providers.matrixProbed", {
            at: formatUnixSeconds(MATRIX.probedAtUnix),
            seconds: Math.round(MATRIX.helpTimeoutMs / 1000),
          })} ${worst.label}`,
        ),
      ),
    ).toBeTruthy();

    // İlk adaptör: sürüm + yardım komutu + bağ ve kapı rozetleri.
    const first = screen.getByRole("heading", { level: 3, name: "Claude Code (matris)" }).closest(".card") as HTMLElement;
    expect(within(first).getByText(exact(`2.1.303 · ${capabilityHelpCommand(MATRIX.adapters[0]!)}`))).toBeTruthy();
    // Bağ etiketi satır durum etiketiyle aynı sözcüğü taşıyabilir → varlık ölçülür.
    expect(within(first).getAllByText(MATRIX.adapters[0]!.linkLabel).length).toBeGreaterThan(0);
    expect(within(first).getByText(t("providers.gateOpen"))).toBeTruthy();
    for (const row of MATRIX.adapters[0]!.rows) {
      expect(within(first).getByText(row.label)).toBeTruthy();
      expect(within(first).getAllByText(row.stateLabel).length).toBeGreaterThan(0);
      expect(within(first).getByText(row.note)).toBeTruthy();
    }
    // Zorunlu yetenek işareti ve detayı (boş detay "bilinmiyor"a düşer).
    expect(within(first).getByText(t("providers.requiredSuffix"), { exact: false })).toBeTruthy();
    expect(within(first).getByText("--print")).toBeTruthy();
    expect(within(first).getAllByText(t("common.unknown")).length).toBeGreaterThan(0);
    expect(
      within(first).getByText(
        byTextContent(`${capabilitySummary(MATRIX.adapters[0]!)} · ${capabilityGateText(MATRIX.adapters[0]!)}`),
      ),
    ).toBeTruthy();
    for (const warning of capabilityWarnings(MATRIX.adapters[0]!)) {
      expect(within(first).getByText(warning)).toBeTruthy();
    }

    // İkinci adaptör: sürüm yok → "sürüm okunamadı", yardım okunamadı uyarısı.
    const second = screen.getByRole("heading", { level: 3, name: "Gemini (matris)" }).closest(".card") as HTMLElement;
    // Sürüm ve yardım komutu aynı `<p>` içinde: satırın tamamı eşleştirilir
    // (RTL varsayılanı yalnız doğrudan metin düğümlerini birleştirir).
    expect(
      within(second).getByText(
        byTextContent(
          `${t("providers.versionUnknown")} · ${capabilityHelpCommand(MATRIX.adapters[1]!)}`,
        ),
      ),
    ).toBeTruthy();
    expect(within(second).getByText(t("providers.gateClosed"))).toBeTruthy();
    for (const warning of capabilityWarnings(MATRIX.adapters[1]!)) {
      expect(within(second).getByText(warning)).toBeTruthy();
    }

    // Sınırlar açılır listede; notlar sunucudan geldiği gibi yazılır.
    for (const note of MATRIX.notes) {
      expect(screen.getByText(note)).toBeTruthy();
    }
  });

  it("matris koşusu başarısız olursa önceki matris ekranda kalmaz", async () => {
    mocks.cliCapabilityMatrix
      .mockResolvedValueOnce(MATRIX)
      .mockRejectedValueOnce({ code: "E_CLI_HELP", message: "yardım metni okunamadı" });
    setup();
    await ready();

    fireEvent.click(button(t("providers.matrixRun")));
    await screen.findByRole("heading", { level: 3, name: "Claude Code (matris)" });

    fireEvent.click(button(t("providers.matrixRun")));
    expect(await screen.findByText("E_CLI_HELP")).toBeTruthy();
    expect(screen.queryByRole("heading", { level: 3, name: "Claude Code (matris)" })).toBeNull();
    expect(screen.getByText(t("providers.matrixIdleBefore"), { exact: false })).toBeTruthy();
  });

  it("sınama, mimari ve paket koşusu düşerse önceki rapor ekranda kalmaz", async () => {
    // Saldırı sınaması
    mocks.cliAttackProbe
      .mockResolvedValueOnce(ATTACK)
      .mockRejectedValueOnce({ code: "E_CLI_ATTACK", message: "sınama düştü" });
    setup();
    await ready();
    fireEvent.click(button(t("providers.attackRun")));
    expect(await screen.findByText(attackFixtureSummary(ATTACK.fixture))).toBeTruthy();
    await waitFor(() => expect(button(t("providers.attackRun")).disabled).toBe(false));
    fireEvent.click(button(t("providers.attackRun")));
    expect(await screen.findByText("E_CLI_ATTACK")).toBeTruthy();
    expect(screen.queryByText(attackFixtureSummary(ATTACK.fixture))).toBeNull();
    cleanup();

    // Mimari raporu
    mocks.cliArchReport
      .mockResolvedValueOnce(ARCH_REPORT)
      .mockRejectedValueOnce({ code: "E_CLI_ARCH", message: "rapor düştü" });
    setup();
    await ready();
    fireEvent.click(button(t("providers.archRun")));
    expect(await screen.findByRole("heading", { level: 3, name: "Kimi" })).toBeTruthy();
    await waitFor(() => expect(button(t("providers.archRun")).disabled).toBe(false));
    fireEvent.click(button(t("providers.archRun")));
    expect(await screen.findByText("E_CLI_ARCH")).toBeTruthy();
    expect(screen.queryByRole("heading", { level: 3, name: "Kimi" })).toBeNull();
    cleanup();

    // Paket raporu
    mocks.workerPackageReport
      .mockResolvedValueOnce(PKG)
      .mockRejectedValueOnce({ code: "E_PKG", message: "paket okunamadı" });
    setup();
    await ready();
    fireEvent.click(button(t("providers.packageRun")));
    expect(await screen.findByText(packageStatusText(PKG))).toBeTruthy();
    await waitFor(() => expect(button(t("providers.packageRun")).disabled).toBe(false));
    fireEvent.click(button(t("providers.packageRun")));
    expect(await screen.findByText("E_PKG")).toBeTruthy();
    expect(screen.queryByText(packageStatusText(PKG))).toBeNull();
    cleanup();

    // Kalıntı ölçümü: düşen yeni ölçüm, eski ölçümün temizlik hedeflerini
    // ekranda bırakmamalı.
    mocks.cliResidueMeasure
      .mockResolvedValueOnce(MEASURE)
      .mockRejectedValueOnce({ code: "E_CLI_MEASURE", message: "ölçüm düştü" });
    setup();
    await ready();
    fireEvent.click(button(t("providers.residueScan")));
    await screen.findByText(persistenceText(SCAN));
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(button(t("providers.measureRun")));
    expect(await screen.findByText(residueMeasureSummary(MEASURE))).toBeTruthy();
    await waitFor(() => expect(button(t("providers.measureRun")).disabled).toBe(false));
    fireEvent.click(button(t("providers.measureRun")));
    expect(await screen.findByText("E_CLI_MEASURE")).toBeTruthy();
    expect(screen.queryByText(residueMeasureSummary(MEASURE))).toBeNull();
    expect(
      screen.queryByRole("button", {
        name: t("providers.cleanupRun", { count: MEASURE.created.length }),
      }),
    ).toBeNull();
    cleanup();

    // Kum havuzu süpürmesi (adaptörden bağımsız rapor)
    mocks.cliStaleSandboxSweep
      .mockResolvedValueOnce(SWEEP)
      .mockRejectedValueOnce({ code: "E_CLI_SWEEP", message: "süpürme düştü" });
    setup();
    await ready();
    fireEvent.click(button(t("providers.residueSweep")));
    expect(await screen.findByText(sandboxSweepSummary(SWEEP))).toBeTruthy();
    await waitFor(() => expect(button(t("providers.residueSweep")).disabled).toBe(false));
    fireEvent.click(button(t("providers.residueSweep")));
    expect(await screen.findByText("E_CLI_SWEEP")).toBeTruthy();
    expect(screen.queryByText(sandboxSweepSummary(SWEEP))).toBeNull();
  });

  it("paket raporu: doğrulama sayaçları, sorunlar, fazlalıklar, tesseract ve sınırlar", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.packageRun")));
    await screen.findByText(packageStatusText(PKG));

    expect(screen.getByText(packageSummary(PKG.manifest))).toBeTruthy();
    expect(
      screen.getByText(
        byTextContent(
          `${PKG.manifest!.buildPlatform} · ${t("providers.manifestLine", {
            python: PKG.manifest!.pythonVersion,
            pyinstaller: PKG.manifest!.pyinstallerVersion,
          })}`,
        ),
      ),
    ).toBeTruthy();

    const counts = verificationCounts(PKG.verification!);
    expect(screen.getByText(t("providers.countVerified"))).toBeTruthy();
    expect(screen.getByText(t("providers.countProblems"))).toBeTruthy();
    expect(screen.getByText(t("providers.countExtra"))).toBeTruthy();
    expect(screen.getByText(String(counts.verified))).toBeTruthy();
    // Sorun ve fazlalık sayıları aynı olabilir → varlık ölçülür.
    expect(screen.getAllByText(String(counts.problems)).length).toBeGreaterThan(0);
    expect(screen.getAllByText(String(counts.extra)).length).toBeGreaterThan(0);
    expect(screen.getByText(PKG.packageDir!)).toBeTruthy();

    // Roller: rol etiketi + sayı.
    for (const row of roleRows(PKG.manifest)) {
      expect(screen.getByText(t("providers.roleRow", { label: row.label, count: row.count }))).toBeTruthy();
    }
    // Sorunlar: yol — açıklama (tek satır); fazlalıklar yol olarak. Tek sorun
    // olduğu için `<ul>` de aynı metni taşır → varlık ölçülür.
    expect(
      screen.getAllByText(
        byTextContent(
          `${PKG.verification!.problems[0]!.path} — ${problemText(PKG.verification!.problems[0]!)}`,
        ),
      ).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText("lib/eski.dylib")).toBeTruthy();

    // Tesseract: PATH'te + gömülü değil + manifest notu. Aynı cümle birden çok
    // yerde geçebilir (manifest satırı + uyarı) → varlık ölçülür.
    expect(screen.getByText(t("providers.tesseractFound"))).toBeTruthy();
    expect(screen.getAllByText(t("providers.tesseractNoEmbed"), { exact: false }).length).toBeGreaterThan(0);
    for (const warning of packageWarnings(PKG)) {
      expect(screen.getByText(warning)).toBeTruthy();
    }
    expect(screen.getByText(packageLimits())).toBeTruthy();

    // Bulunmayan paket hata değil: geliştirme kipi olarak yazılır; nedeni uyarı
    // olarak görünür (`packageWarnings` çekirdeğin notunu taşır).
    cleanup();
    setup({ pkg: async () => PKG_MISSING });
    await ready();
    fireEvent.click(button(t("providers.packageRun")));
    expect(await screen.findByText(packageStatusText(PKG_MISSING))).toBeTruthy();
    expect(screen.getByText(PKG_MISSING.note!)).toBeTruthy();
    // "bulunamadı" durum rozetinde de geçer → tesseract satırı kendi
    // kapsamında doğrulanır (satır tek `<div>`).
    const tesseractLine = screen.getByText((_content, element) => {
      if (!(element instanceof HTMLElement) || element.tagName !== "DIV") return false;
      return (element.textContent ?? "").replace(/\s+/g, " ").trim().startsWith(t("providers.tesseractLabel"));
    });
    expect(tesseractLine.textContent).toContain(t("providers.tesseractMissing"));
  });

  it("saldırı sınaması: fixture kanıtı, ürün yapılandırması, isabet/atlandı rozetleri ve hükümler", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.attackRun")));
    await screen.findByText(attackFixtureSummary(ATTACK.fixture));

    const worst = attackWorstVerdict(ATTACK.adapters);
    expect(
      screen.getByText(
        byTextContent(
          `${t("providers.attackProbed", {
            at: formatUnixSeconds(ATTACK.probedAtUnix),
          })} ${worst.label} · ${t("providers.attackHitsLabel")} ${attackTotalHits(ATTACK)}`,
        ),
      ),
    ).toBeTruthy();
    for (const item of ATTACK.fixture.productConfig) {
      expect(screen.getByText(attackProductText(item))).toBeTruthy();
    }
    for (const warning of attackFixtureWarnings(ATTACK.fixture)) {
      expect(screen.getByText(warning)).toBeTruthy();
    }

    const first = screen.getByRole("heading", { level: 3, name: "Claude Code (saldırı)" }).closest(".card") as HTMLElement;
    expect(within(first).getByText(attackCommand(ATTACK.adapters[0]!))).toBeTruthy();
    expect(within(first).getByText(t("providers.hitsBadge", { count: 1 }))).toBeTruthy();
    for (const vector of ATTACK.adapters[0]!.vectors) {
      expect(within(first).getByText(vector.label)).toBeTruthy();
      // Kapatma metni "kapatma: …" önekiyle aynı satırda yazılır.
      expect(
        within(first).getByText(exact(`${t("providers.guardPrefix")} ${attackGuardText(vector)}`)),
      ).toBeTruthy();
      expect(within(first).getByText(attackVerdictText(vector))).toBeTruthy();
    }
    for (const warning of attackAdapterWarnings(ATTACK.adapters[0]!)) {
      expect(within(first).getByText(warning)).toBeTruthy();
    }

    // Atlanan adaptör: "isabet yok" + atlanma nedeni rozeti, vektör listesi yok.
    // Atlanma nedeni hem rozette hem uyarı satırında geçebilir.
    const second = screen.getByRole("heading", { level: 3, name: "Kimi (saldırı)" }).closest(".card") as HTMLElement;
    expect(within(second).getByText(t("providers.noHits"))).toBeTruthy();
    expect(
      within(second).getAllByText(attackSkipText("etkileşimli_only")!).length,
    ).toBeGreaterThan(0);

    for (const note of ATTACK.notes) {
      expect(screen.getByText(note)).toBeTruthy();
    }
  });

  it("mimari raporu: biçim, mimari karşılaştırması, yorumlayıcı, neden ve başlık baytları", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.archRun")));
    await screen.findByRole("heading", { level: 3, name: "Kimi" });

    const worst = archWorst(ARCH_REPORT);
    expect(
      screen.getByText(
        byTextContent(
          `${t("providers.archProbed", {
            at: archProbedAt(ARCH_REPORT.probedAtUnix),
            summary: archSummaryText(ARCH_REPORT),
          })} ${worst.label}`,
        ),
      ),
    ).toBeTruthy();

    const first = screen.getByRole("heading", { level: 3, name: "Claude Code" }).closest(".card") as HTMLElement;
    expect(
      within(first).getByText(`claude · ${archFormatText(ARCH_REPORT.adapters[0]!)}`),
    ).toBeTruthy();
    expect(within(first).getByText(archBadgeText(ARCH_REPORT.adapters[0]!))).toBeTruthy();
    expect(within(first).getByText(archPathText(ARCH_REPORT.adapters[0]!))).toBeTruthy();
    expect(
      within(first).getByText(
        `${ARCH_REPORT.adapters[0]!.architectures.join(", ")} · ${ARCH_REPORT.adapters[0]!.comparisonLabel}`,
      ),
    ).toBeTruthy();
    expect(within(first).getByText(archInterpreterText(ARCH_REPORT.adapters[0]!)!)).toBeTruthy();
    expect(
      within(first).getByText(t("providers.headerBytes", { bytes: ARCH_REPORT.adapters[0]!.headerBytes })),
    ).toBeTruthy();

    // Başlık baytı 0 → "bilinmiyor" (uydurma yok) ve neden satırı yazılır.
    const second = screen.getByRole("heading", { level: 3, name: "Kimi" }).closest(".card") as HTMLElement;
    expect(within(second).getByText(t("common.unknown"))).toBeTruthy();
    expect(within(second).getByText(archReasonText(ARCH_REPORT.adapters[1]!.reason)!)).toBeTruthy();
    // Yorumlayıcı yoksa satırı hiç çizilmez.
    expect(within(second).queryByText(t("providers.interpreterLabel"))).toBeNull();

    for (const warning of archWarnings(ARCH_REPORT)) {
      expect(screen.getByText(warning)).toBeTruthy();
    }
    for (const note of ARCH_REPORT.notes) {
      expect(screen.getByText(note)).toBeTruthy();
    }
  });

  it("kalıntı taraması: kök, toplam, kalıcılık ve yol satırları", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.residueScan")));
    await screen.findByText(persistenceText(SCAN));

    expect(mocks.cliResidueScan).toHaveBeenCalledWith("claude-code");
    expect(screen.getByText(SCAN.root)).toBeTruthy();
    // Aynı metin hem toplam satırında hem yol satırında görünebilir.
    expect(
      screen.getAllByText(
        t("probe.filesWithSize", { count: SCAN.totalFiles, size: formatSize(SCAN.totalBytes) }),
      ).length,
    ).toBeGreaterThan(0);
    for (const path of SCAN.paths) {
      // Satır kapsamı: "—" (bilinmiyor) ve dosya özeti başka panellerde de
      // geçebilir; her hücre kendi satırında doğrulanır.
      const row = screen.getByText(path.relative).closest("tr");
      if (!(row instanceof HTMLElement)) throw new Error(`${path.relative} satırı yok`);
      expect(within(row).getByText(path.kind)).toBeTruthy();
      expect(within(row).getByText(residuePathLine(path))).toBeTruthy();
      expect(within(row).getByText(residueAge(path.newestAgeSecs))).toBeTruthy();
      expect(within(row).getByText(path.note)).toBeTruthy();
    }
  });

  it("kalıntı ölçümü açık onay ister; temizlik yalnız ölçümde oluşan dosyaları hedefler", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.residueScan")));
    await screen.findByText(persistenceText(SCAN));

    const confirm = screen.getByRole("checkbox") as HTMLInputElement;
    expect(confirm.checked).toBe(false);
    expect(button(t("providers.measureRun")).disabled).toBe(true);

    fireEvent.click(confirm);
    expect(button(t("providers.measureRun")).disabled).toBe(false);
    fireEvent.click(button(t("providers.measureRun")));

    await waitFor(() =>
      expect(mocks.cliResidueMeasure).toHaveBeenCalledWith("claude-code", true),
    );
    expect(await screen.findByText(residueMeasureSummary(MEASURE))).toBeTruthy();
    expect(
      screen.getAllByText(
        t("probe.filesWithSize", {
          count: MEASURE.before.totalFiles,
          size: formatSize(MEASURE.before.totalBytes),
        }),
      ).length,
    ).toBeGreaterThan(0);
    expect(
      screen.getAllByText(t("probe.filesWithSize", { count: MEASURE.after.totalFiles, size: formatSize(MEASURE.after.totalBytes) })).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText(MEASURE.cleanupHint)).toBeTruthy();
    expect(
      screen.getByText(`${MEASURE.created[0]!.relative} · ${formatSize(MEASURE.created[0]!.bytes)}`),
    ).toBeTruthy();

    // Temizlik: yalnız oluşan dosyalar hedef, ölçüm zamanı ve adaptör birebir.
    fireEvent.click(button(t("providers.cleanupRun", { count: MEASURE.created.length })));
    await waitFor(() =>
      expect(mocks.cliResidueCleanup).toHaveBeenCalledWith("claude-code", MEASURE.measuredAtUnix, [
        MEASURE.created[0]!.relative,
      ]),
    );
    expect(await screen.findByText(residueCleanupSummary(CLEANUP))).toBeTruthy();
    expect(screen.getByText(`${CLEANUP.skipped[0]!.path} — ${CLEANUP.skipped[0]!.reason}`)).toBeTruthy();
    // Temizlik sonrası yeniden taranır (bayat envanter kalmasın).
    expect(mocks.cliResidueScan).toHaveBeenCalledTimes(2);
  });

  it("eski kum havuzu süpürmesi özetlenir; atlananlar gizlenmez", async () => {
    setup();
    await ready();

    fireEvent.click(button(t("providers.residueSweep")));
    expect(await screen.findByText(sandboxSweepSummary(SWEEP))).toBeTruthy();
    expect(screen.getByText(`${SWEEP.skipped[0]!.path} — ${SWEEP.skipped[0]!.reason}`)).toBeTruthy();
    expect(mocks.cliStaleSandboxSweep).toHaveBeenCalledTimes(1);
  });

  it("adaptör değişince kalıntı panelinin önceki hatası ekranda kalmaz", async () => {
    mocks.cliResidueScan
      .mockRejectedValueOnce({ code: "E_CLI_RESIDUE", message: "ev dizini okunamadı" })
      .mockResolvedValueOnce(SCAN);
    setup();
    await ready();

    fireEvent.click(button(t("providers.residueScan")));
    expect(await screen.findByText("E_CLI_RESIDUE")).toBeTruthy();

    // Başka adaptöre geçilince eski hata o adaptörün hatası gibi okunmamalı.
    fireEvent.change(screen.getByLabelText(t("providers.residueSelectAria")), {
      target: { value: "antigravity" },
    });
    await waitFor(() => expect(screen.queryByText("E_CLI_RESIDUE")).toBeNull());
    expect(screen.getByText(t("providers.residueIdle"))).toBeTruthy();
  });
});
