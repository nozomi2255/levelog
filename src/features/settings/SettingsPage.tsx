import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { CheckCircle2, DatabaseBackup, Download, LoaderCircle, PlugZap } from "lucide-react";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";
import type { BackupResult, CodexConnectionStatus, ExportResult } from "../../lib/types";

type Operation = "backup" | "export" | null;

function ResultLine({ label, path }: { label: string; path: string }) {
  return <p role="status"><CheckCircle2 aria-hidden="true" size={18} /> {label}: <code>{path}</code></p>;
}

export function SettingsPage() {
  const queryClient = useQueryClient();
  const boot = useQuery({ queryKey: ["boot-state"], queryFn: api.getBootState });
  const [codexPath, setCodexPath] = useState("");
  const effectiveCodexPath = codexPath || boot.data?.codex?.path || "";
  const [pathSaved, setPathSaved] = useState(false);
  const [connection, setConnection] = useState<CodexConnectionStatus | null>(null);
  const [connectionError, setConnectionError] = useState("");
  const [operation, setOperation] = useState<Operation>(null);
  const [backup, setBackup] = useState<BackupResult | null>(null);
  const [exported, setExported] = useState<ExportResult | null>(null);
  const [error, setError] = useState("");

  async function checkConnection() {
    setConnectionError("");
    try { setConnection(await api.testCodexConnection(effectiveCodexPath)); }
    catch (reason) { setConnection(null); setConnectionError(String(reason)); }
  }
  async function saveCodexPath() {
    setConnectionError(""); setPathSaved(false);
    try { await api.updateCodexPath(effectiveCodexPath); setPathSaved(true); await queryClient.invalidateQueries({ queryKey: ["boot-state"] }); }
    catch (reason) { setConnectionError(String(reason)); }
  }
  async function runBackup() {
    setOperation("backup"); setError("");
    try { setBackup(await api.createBackup()); } catch (reason) { setError(String(reason)); } finally { setOperation(null); }
  }
  async function runExport() {
    setOperation("export"); setError("");
    try { setExported(await api.exportJson()); } catch (reason) { setError(String(reason)); } finally { setOperation(null); }
  }

  return <section className="feature-page" aria-labelledby="settings-title"><header className="page-heading"><div><p className="hud-kicker">LOCAL DATA CONTROL</p><h1 id="settings-title">設定</h1><p>Codex接続と、ローカルデータのバックアップ・持ち出しを管理します。</p></div></header><div className="settings-page">
    <HudPanel title="Codex CLI">
      <div className="settings-stack"><p className="empty-copy">Codex CLIの絶対パスと接続状態を確認します。記録内容は送信前に確認できます。</p>
        <label className="settings-field">Codex CLIのパス<input value={effectiveCodexPath} onChange={(event) => { setCodexPath(event.target.value); setPathSaved(false); }} placeholder="/opt/homebrew/bin/codex" aria-describedby="codex-help" /></label>
        <p id="codex-help" className="empty-copy">Finderから起動したアプリでは、ターミナルのPATHを利用できない場合があります。</p>
        <div className="settings-actions"><HudButton type="button" onClick={saveCodexPath} disabled={!effectiveCodexPath.trim()}>パスを保存</HudButton><HudButton type="button" onClick={checkConnection} disabled={!effectiveCodexPath.trim()}><PlugZap aria-hidden="true" size={18} /> 接続をテスト</HudButton></div>
        {pathSaved && <p role="status"><HudBadge tone="green">SAVED</HudBadge> Codex CLIのパスを保存しました。</p>}
        {connection && <p role="status"><HudBadge tone={connection.available && connection.authenticated ? "green" : "gold"}>{connection.available && connection.authenticated ? "利用可能" : "確認が必要"}</HudBadge> {connection.message}{connection.version ? ` (${connection.version})` : ""}</p>}
        {connectionError && <p role="alert">接続確認に失敗しました: {connectionError}</p>}
      </div>
    </HudPanel>
    <HudPanel title="ローカルデータ">
      <div className="settings-stack"><p className="empty-copy">バックアップはSQLiteの整合性を保って作成します。JSONエクスポートには将来の秘密情報は含めません。</p>
        <div className="settings-actions"><HudButton type="button" tone="gold" onClick={runBackup} disabled={operation !== null}>{operation === "backup" ? <LoaderCircle className="spin" aria-hidden="true" size={18} /> : <DatabaseBackup aria-hidden="true" size={18} />} バックアップを作成</HudButton><HudButton type="button" tone="cyan" onClick={runExport} disabled={operation !== null}>{operation === "export" ? <LoaderCircle className="spin" aria-hidden="true" size={18} /> : <Download aria-hidden="true" size={18} />} JSONを書き出す</HudButton></div>
        {backup && <ResultLine label="バックアップを作成しました" path={backup.path} />}
        {exported && <ResultLine label={`JSONを書き出しました（schema v${exported.schemaVersion}）`} path={exported.path} />}
        {error && <p role="alert">データ操作に失敗しました: {error}</p>}
      </div>
    </HudPanel>
  </div></section>;
}
