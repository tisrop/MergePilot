import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AppSelect from "@/components/shared/AppSelect.vue";

const options = [
  { value: "main", label: "main" },
  { value: "feature/search", label: "feature/search" },
  { value: "release/1.0", label: "release/1.0" },
];

describe("AppSelect", () => {
  it("搜索选项后支持键盘选择匹配结果", async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: "main",
        options,
        searchable: true,
        searchPlaceholder: "搜索分支",
      },
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    const search = wrapper.get('input[placeholder="搜索分支"]');
    await search.setValue("RELEASE");
    await flushPromises();

    expect(wrapper.findAll(".dropdown-option").map((option) => option.text())).toEqual([
      "release/1.0",
    ]);
    await search.trigger("keydown", { key: "Enter" });
    expect(wrapper.emitted("update:modelValue")).toEqual([["release/1.0"]]);
    expect(wrapper.find(".dropdown-panel").exists()).toBe(false);
  });

  it("没有搜索结果时显示明确空状态", async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: "",
        options,
        searchable: true,
      },
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    await wrapper.get('input[type="search"]').setValue("missing");

    expect(wrapper.findAll(".dropdown-option")).toHaveLength(0);
    expect(wrapper.get(".dropdown-empty").text()).toBe("没有匹配选项");
  });

  it("输入法组合态回车不会选中仓库或分支", async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: "",
        options,
        searchable: true,
      },
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    const search = wrapper.get('input[type="search"]');
    await search.setValue("feature");
    await search.trigger("keydown", { key: "Enter", keyCode: 229, isComposing: true });

    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    expect(wrapper.find(".dropdown-panel").exists()).toBe(true);

    await wrapper.get(".dropdown-option[data-value='feature/search']").trigger("click");
    expect(wrapper.emitted("update:modelValue")).toEqual([["feature/search"]]);
  });

  it("未启用搜索时保持原有下拉行为", async () => {
    const wrapper = mount(AppSelect, { props: { modelValue: "main", options } });

    await wrapper.get('[role="combobox"]').trigger("click");

    expect(wrapper.find('input[type="search"]').exists()).toBe(false);
    expect(wrapper.findAll(".dropdown-option")).toHaveLength(3);
  });

  it("方向键在列表边界循环并跳过禁用项", async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: "release/1.0",
        options: [options[0], { ...options[1], disabled: true }, options[2]],
      },
    });
    const trigger = wrapper.get('[role="combobox"]');

    await trigger.trigger("click");
    expect(wrapper.get(".dropdown-option.highlighted").attributes("data-value")).toBe(
      "release/1.0",
    );

    await trigger.trigger("keydown", { key: "ArrowDown" });
    expect(wrapper.get(".dropdown-option.highlighted").attributes("data-value")).toBe("main");

    await trigger.trigger("keydown", { key: "ArrowUp" });
    expect(wrapper.get(".dropdown-option.highlighted").attributes("data-value")).toBe(
      "release/1.0",
    );
  });

  it("有下一页时在下拉内触发加载更多并保持展开", async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: "main",
        options,
        hasMore: true,
        loadMoreText: "加载更多仓库",
      },
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    await wrapper.get(".dropdown-load-more").trigger("click");

    expect(wrapper.emitted("load-more")).toHaveLength(1);
    expect(wrapper.find(".dropdown-panel").exists()).toBe(true);
    await wrapper.setProps({ loadingMore: true });
    expect(wrapper.get<HTMLButtonElement>(".dropdown-load-more").element.disabled).toBe(true);
    expect(wrapper.get(".dropdown-load-more").text()).toBe("加载中…");
  });

  it("搜索框按 Escape 后关闭并将焦点还给触发器", async () => {
    const wrapper = mount(AppSelect, {
      props: { modelValue: "main", options, searchable: true },
      attachTo: document.body,
    });

    const trigger = wrapper.get<HTMLElement>('[role="combobox"]');
    await trigger.trigger("click");
    await wrapper.get('input[type="search"]').trigger("keydown", { key: "Escape" });

    expect(wrapper.find(".dropdown-panel").exists()).toBe(false);
    expect(document.activeElement).toBe(trigger.element);
    wrapper.unmount();
  });

  it("搜索后关闭再打开时重新高亮当前选中项", async () => {
    const wrapper = mount(AppSelect, {
      props: {
        modelValue: "release/1.0",
        options,
        searchable: true,
        searchPlaceholder: "搜索分支",
      },
    });
    const trigger = wrapper.get('[role="combobox"]');

    await trigger.trigger("click");
    await wrapper.get('input[placeholder="搜索分支"]').setValue("feature");
    await trigger.trigger("click");
    await trigger.trigger("click");

    const search = wrapper.get<HTMLInputElement>('input[placeholder="搜索分支"]');
    expect(search.element.value).toBe("");
    expect(wrapper.get(".dropdown-option.highlighted").attributes("data-value")).toBe(
      "release/1.0",
    );
    await search.trigger("keydown", { key: "Enter" });
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual(["release/1.0"]);
  });
});
