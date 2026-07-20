import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { HudBadge } from "../../components/hud";
import { api } from "../../lib/api";
import { CodexSetupPanel, ProfileForm } from "../profile";

export function OnboardingPage({ onSaved }: { onSaved?: () => void }) {
  const [profileSaved, setProfileSaved] = useState(false);
  const save = useMutation({ mutationFn: api.updateUserProfile, onSuccess: () => setProfileSaved(true) });
  if (profileSaved) return <div className="onboarding-card"><header className="onboarding-hero"><HudBadge tone="green">PROFILE SAVED</HudBadge><p className="hud-kicker">OPTIONAL AI CONNECTION</p><h1>Codex CLIを接続</h1><p>プロフィールは保存済みです。AI接続は自動検出から設定するか、後回しにして記録だけで始められます。</p></header><CodexSetupPanel allowDefer onDeferred={onSaved} onSaved={onSaved} /></div>;
  return <div className="onboarding-card"><header className="onboarding-hero"><HudBadge tone="purple">SYSTEM INITIALIZATION</HudBadge><p className="hud-kicker">LEVEL·LOG / LOCAL GROWTH SYSTEM</p><h1>成長プロフィールを設定</h1><p>プロフィールはこの端末内に保存され、後から設定画面でいつでも編集できます。AIへ送る内容は実行前に確認できます。</p></header><ProfileForm mode="create" submitting={save.isPending} error={save.error} onSubmit={(value) => save.mutate(value)} /></div>;
}
