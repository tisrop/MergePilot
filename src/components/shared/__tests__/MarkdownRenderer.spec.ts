import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import MarkdownRenderer from "@/components/shared/MarkdownRenderer.vue";

describe("MarkdownRenderer", () => {
  it("渲染 GFM Markdown 并清理脚本和危险链接", () => {
    const wrapper = mount(MarkdownRenderer, {
      props: {
        content:
          "# 标题\n\n- 项目\n\n<script>alert(1)</script>\n\n[危险](javascript:alert(1)) [安全](https://example.com)",
      },
    });

    expect(wrapper.find("h1").text()).toBe("标题");
    expect(wrapper.find("li").text()).toBe("项目");
    expect(wrapper.find("script").exists()).toBe(false);
    expect(wrapper.find("a[href^='javascript:']").exists()).toBe(false);
    expect(wrapper.find("a[href='https://example.com']").attributes("rel")).toBe(
      "noopener noreferrer",
    );
  });

  it("移除协议相对链接和图片并保留本地路径", () => {
    const wrapper = mount(MarkdownRenderer, {
      props: {
        content:
          "[协议相对链接](//evil.example/phish) ![协议相对图片](//evil.example/pixel.png)\n\n" +
          '<a href="\\\\evil.example/phish">反斜杠链接</a> ' +
          "[本地链接](/docs/review) ![本地图片](./assets/diagram.png)",
      },
    });

    expect(wrapper.find("a[href='//evil.example/phish']").exists()).toBe(false);
    expect(wrapper.find("img[src='//evil.example/pixel.png']").exists()).toBe(false);
    expect(wrapper.find("a[href^='\\\\evil.example']").exists()).toBe(false);
    expect(wrapper.find("a[href='/docs/review']").exists()).toBe(true);
    expect(wrapper.find("img[src='./assets/diagram.png']").exists()).toBe(true);
  });
});
