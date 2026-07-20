import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AppMultiSelect from "@/components/shared/AppMultiSelect.vue";

const options = [
  { value: "bug", label: "bug", color: "#d73a4a", description: "需要修复的问题" },
  { value: "feature", label: "feature", color: "#a2eeef", description: "新功能" },
  { value: "frontend", label: "frontend" },
];

describe("AppMultiSelect", () => {
  it("搜索后可以连续选择多个选项且保持下拉打开", async () => {
    const wrapper = mount(AppMultiSelect, {
      props: { modelValue: [], options, searchPlaceholder: "搜索标签" },
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    expect(wrapper.get(".multi-select-swatch").attributes("style")).toContain(
      "background-color: rgb(215, 58, 74)",
    );
    expect(wrapper.text()).toContain("需要修复的问题");
    await wrapper.get('input[placeholder="搜索标签"]').setValue("front");
    await wrapper.get(".multi-select-option[data-value='frontend']").trigger("click");

    expect(wrapper.emitted("update:modelValue")).toEqual([[["frontend"]]]);
    expect(wrapper.find(".multi-select-dropdown").exists()).toBe(true);

    await wrapper.setProps({ modelValue: ["frontend"] });
    await wrapper.get('input[placeholder="搜索标签"]').setValue("");
    await wrapper.get(".multi-select-option[data-value='bug']").trigger("click");
    expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([["bug", "frontend"]]);
  });

  it("中文输入法组合态回车不会误选标签", async () => {
    const wrapper = mount(AppMultiSelect, { props: { modelValue: [], options } });

    await wrapper.get('[role="combobox"]').trigger("click");
    const search = wrapper.get('input[type="search"]');
    await search.setValue("feature");
    await search.trigger("keydown", { key: "Enter", keyCode: 229, isComposing: true });

    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    expect(wrapper.find(".multi-select-dropdown").exists()).toBe(true);
  });

  it("没有选项时显示调用方配置的空状态", async () => {
    const wrapper = mount(AppMultiSelect, {
      props: { modelValue: [], options: [], emptyText: "仓库暂无成员" },
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    expect(wrapper.get(".multi-select-empty").text()).toBe("仓库暂无成员");
  });

  it("搜索无结果时显示调用方配置的空状态", async () => {
    const wrapper = mount(AppMultiSelect, { props: { modelValue: [], options: [] } });

    await wrapper.get('[role="combobox"]').trigger("click");
    await wrapper.get('input[type="search"]').setValue("missing");
    await wrapper.setProps({ emptySearchText: "没有匹配成员" });

    expect(wrapper.get(".multi-select-empty").text()).toBe("没有匹配成员");
  });

  it("点击组件外部后关闭下拉并清空搜索", async () => {
    const wrapper = mount(AppMultiSelect, {
      props: { modelValue: [], options },
      attachTo: document.body,
    });

    await wrapper.get('[role="combobox"]').trigger("click");
    await wrapper.get('input[type="search"]').setValue("feature");
    document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await wrapper.vm.$nextTick();

    expect(wrapper.find(".multi-select-dropdown").exists()).toBe(false);
    await wrapper.get('[role="combobox"]').trigger("click");
    expect(wrapper.get<HTMLInputElement>('input[type="search"]').element.value).toBe("");
    wrapper.unmount();
  });
});
