import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProfileForm } from "./ProfileForm";

describe("ProfileForm", () => {
  afterEach(cleanup);
  it("collects the rich profile with one to three focus skills", async () => {
    const user = userEvent.setup(); const onSubmit = vi.fn();
    render(<ProfileForm mode="create" submitting={false} onSubmit={onSubmit} />);
    await user.type(screen.getByLabelText("現在の役割"), "プロダクトエンジニア");
    await user.type(screen.getByLabelText("成長目標"), "説明を分かりやすくする");
    await user.click(screen.getByLabelText("説明"));
    await user.click(screen.getByRole("button", { name: "プロフィールを保存してAI設定へ" }));
    expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ role: "プロダクトエンジニア", growthGoal: "説明を分かりやすくする", focusSkillIds: ["communication.explanation"] }));
  });
  it("offers an explicit latest-revision reload after a save conflict", async () => {
    const user = userEvent.setup(); const onReload = vi.fn();
    render(<ProfileForm mode="edit" submitting={false} error="revision conflict" onReload={onReload} onSubmit={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "最新プロフィールを再読み込み" }));
    expect(onReload).toHaveBeenCalledOnce();
  });
});
