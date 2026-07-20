import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getPlatformCapabilities } from "@/api";
import { useCapabilityStore } from "@/stores/useCapabilityStore";
import type { PlatformCapabilities } from "@/types";

vi.mock("@/api", () => ({ getPlatformCapabilities: vi.fn() }));

const github: PlatformCapabilities = {
  platform: "github",
  review_events: ["comment", "approve", "request_changes"],
  merge_strategies: ["merge", "squash", "rebase"],
  supports_fork_context: true,
  supports_issue_auto_close: true,
  supports_compare_diff: true,
  supports_review_thread_resolution: true,
  supports_remote_file_viewed_state: true,
  supports_pr_title_body_edit: true,
  supports_pr_draft_toggle: true,
  supports_pr_reviewer_management: true,
  supports_pr_assignee_management: true,
  supports_pr_label_management: true,
  supports_pr_milestone_management: true,
  supports_pr_creation: true,
};

describe("useCapabilityStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("按平台缓存能力并合并并发请求", async () => {
    vi.mocked(getPlatformCapabilities).mockResolvedValue(github);
    const store = useCapabilityStore();

    const [first, second] = await Promise.all([store.load("github"), store.load("github")]);
    expect(first).toEqual(github);
    expect(second).toEqual(github);
    expect(getPlatformCapabilities).toHaveBeenCalledTimes(1);
    expect(store.values.github?.review_events).toContain("approve");
  });
});
