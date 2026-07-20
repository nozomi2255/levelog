import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { DownloadCloud, LoaderCircle, RefreshCw, ShieldCheck } from "lucide-react";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";
import type { AppUpdateDto } from "../../lib/types";

type UpdateOperation = "checking" | "installing" | null;

function formatDate(value: string | null) {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("ja-JP", { dateStyle: "medium", timeStyle: "short" }).format(date);
}

export function AppUpdatePanel() {
  const release = useQuery({
    queryKey: ["release-info"],
    queryFn: api.getReleaseInfo,
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const [operation, setOperation] = useState<UpdateOperation>(null);
  const [available, setAvailable] = useState<AppUpdateDto | null>(null);
  const [checked, setChecked] = useState(false);
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState<number | null>(null);
  const [error, setError] = useState("");

  const check = async () => {
    setOperation("checking");
    setError("");
    setChecked(false);
    setAvailable(null);
    try {
      const update = await api.checkForAppUpdate();
      setAvailable(update);
      setChecked(true);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setOperation(null);
    }
  };

  const install = async () => {
    setOperation("installing");
    setError("");
    setDownloaded(0);
    setTotal(null);
    try {
      await api.installAppUpdate((event) => {
        if (event.event === "started") setTotal(event.data.contentLength);
        if (event.event === "progress") setDownloaded((value) => value + event.data.chunkLength);
      });
    } catch (reason) {
      setError(String(reason));
      setOperation(null);
    }
  };

  const configured = release.data?.updaterConfigured === true;
  const progressMax = total && total > 0 ? total : Math.max(downloaded, 1);
  const publishedAt = formatDate(available?.publishedAt ?? null);
  const isAdHocDistribution = release.data?.macosDistribution === "ad-hoc";

  return (
    <HudPanel
      title="Levelogを最新に保つ"
      action={release.data ? <HudBadge tone={configured ? "green" : "gold"}>{release.data.currentVersion}</HudBadge> : undefined}
    >
      <div className="settings-stack update-panel">
        <p className="empty-copy">
          GitHub Releasesで公開された更新を確認し、Tauri更新署名を検証してからインストールします。更新元のURLや実行ファイルを手入力する必要はありません。
        </p>
        {release.isPending && <p role="status">バージョン情報を確認しています…</p>}
        {release.isError && <p role="alert">バージョン情報を取得できませんでした: {String(release.error)}</p>}
        {release.data && (
          <dl className="release-facts">
            <div><dt>現在</dt><dd>v{release.data.currentVersion}</dd></div>
            <div><dt>チャネル</dt><dd>{release.data.releaseChannel}</dd></div>
            <div><dt>検証</dt><dd><ShieldCheck aria-hidden="true" size={17} /> Tauri更新署名を必須化</dd></div>
          </dl>
        )}
        {isAdHocDistribution && (
          <p className="update-notice" role="status">
            この配布版はDeveloper ID署名・Apple公証を使用していません。初回のインストールは公式GitHub Releasesから行い、macOSの警告に従って許可してください。アプリ内更新はTauri更新署名で検証します。
          </p>
        )}
        {release.data && !configured && (
          <p className="update-notice" role="status">
            この開発ビルドには更新チャネルが設定されていません。GitHub Releasesから配布された正式版では利用できます。
          </p>
        )}
        <div className="settings-actions">
          <HudButton type="button" onClick={() => void check()} disabled={!configured || operation !== null}>
            {operation === "checking" ? <LoaderCircle className="spin" aria-hidden="true" /> : <RefreshCw aria-hidden="true" />}
            {operation === "checking" ? "確認中…" : "更新を確認"}
          </HudButton>
        </div>
        {checked && !available && <p className="result-message" role="status">最新バージョンを使用しています。</p>}
        {available && (
          <section className="update-available" aria-labelledby="available-update-title">
            <div>
              <HudBadge tone="gold">UPDATE AVAILABLE</HudBadge>
              <h3 id="available-update-title">v{available.version} を利用できます</h3>
              {publishedAt && <p className="empty-copy">公開: {publishedAt}</p>}
            </div>
            {available.notes && <pre className="release-notes" aria-label="リリースノート">{available.notes}</pre>}
            {operation === "installing" && (
              <div className="download-progress" role="status" aria-live="polite">
                <progress aria-label="更新のダウンロード進捗" value={downloaded} max={progressMax} />
                <span>{total ? `${Math.min(100, Math.round((downloaded / total) * 100))}%` : "ダウンロードと署名検証を実行中…"}</span>
              </div>
            )}
            <HudButton tone="gold" type="button" onClick={() => void install()} disabled={operation !== null}>
              {operation === "installing" ? <LoaderCircle className="spin" aria-hidden="true" /> : <DownloadCloud aria-hidden="true" />}
              {operation === "installing" ? "更新しています…" : "更新して再起動"}
            </HudButton>
            <p className="form-help">インストール完了後、Levelogは自動的に再起動します。記録済みのローカルデータはそのまま保持されます。</p>
          </section>
        )}
        {error && <p role="alert">更新操作を完了できませんでした: {error}</p>}
      </div>
    </HudPanel>
  );
}
