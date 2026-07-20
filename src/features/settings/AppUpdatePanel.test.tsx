import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../lib/api";
import type { PropsWithChildren } from "react";
import { AppUpdatePanel } from "./AppUpdatePanel";

vi.mock("../../lib/api", () => ({
  api: {
    getReleaseInfo: vi.fn(),
    checkForAppUpdate: vi.fn(),
    installAppUpdate: vi.fn(),
  },
}));

function Wrapper({ children }: PropsWithChildren) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

describe("AppUpdatePanel", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("explains why development builds cannot check the release channel", async () => {
    vi.mocked(api.getReleaseInfo).mockResolvedValue({
      currentVersion: "0.1.0",
      updaterConfigured: false,
      releaseChannel: "GitHub Releases / stable",
    });
    render(<AppUpdatePanel />, { wrapper: Wrapper });

    expect(await screen.findByText("v0.1.0")).toBeInTheDocument();
    expect(screen.getByText(/この開発ビルドには更新チャネルが設定されていません/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "更新を確認" })).toBeDisabled();
  });

  it("shows signed release details and starts a user-confirmed install", async () => {
    const user = userEvent.setup();
    vi.mocked(api.getReleaseInfo).mockResolvedValue({
      currentVersion: "0.1.0",
      updaterConfigured: true,
      releaseChannel: "GitHub Releases / stable",
    });
    vi.mocked(api.checkForAppUpdate).mockResolvedValue({
      currentVersion: "0.1.0",
      version: "0.2.0",
      publishedAt: "2026-07-20T08:00:00Z",
      notes: "プロフィール編集と更新機能を改善しました。",
    });
    vi.mocked(api.installAppUpdate).mockImplementation(async (onEvent) => {
      onEvent({ event: "started", data: { contentLength: 100 } });
      onEvent({ event: "progress", data: { chunkLength: 50 } });
    });
    render(<AppUpdatePanel />, { wrapper: Wrapper });

    await user.click(await screen.findByRole("button", { name: "更新を確認" }));
    expect(await screen.findByRole("heading", { name: "v0.2.0 を利用できます" })).toBeInTheDocument();
    expect(screen.getByLabelText("リリースノート")).toHaveTextContent("プロフィール編集と更新機能を改善しました。");

    await user.click(screen.getByRole("button", { name: "更新して再起動" }));
    await waitFor(() => expect(api.installAppUpdate).toHaveBeenCalledOnce());
    expect(await screen.findByText("50%")).toBeInTheDocument();
  });

  it("reports that the installed version is current", async () => {
    const user = userEvent.setup();
    vi.mocked(api.getReleaseInfo).mockResolvedValue({
      currentVersion: "0.2.0",
      updaterConfigured: true,
      releaseChannel: "GitHub Releases / stable",
    });
    vi.mocked(api.checkForAppUpdate).mockResolvedValue(null);
    render(<AppUpdatePanel />, { wrapper: Wrapper });

    await user.click(await screen.findByRole("button", { name: "更新を確認" }));
    expect(await screen.findByText("最新バージョンを使用しています。")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "更新して再起動" })).not.toBeInTheDocument();
  });

  it("keeps a failed check recoverable", async () => {
    const user = userEvent.setup();
    vi.mocked(api.getReleaseInfo).mockResolvedValue({
      currentVersion: "0.1.0",
      updaterConfigured: true,
      releaseChannel: "GitHub Releases / stable",
    });
    vi.mocked(api.checkForAppUpdate).mockRejectedValue(new Error("network unavailable"));
    render(<AppUpdatePanel />, { wrapper: Wrapper });

    const button = await screen.findByRole("button", { name: "更新を確認" });
    await user.click(button);
    expect(await screen.findByRole("alert")).toHaveTextContent("network unavailable");
    expect(button).toBeEnabled();
  });
});
