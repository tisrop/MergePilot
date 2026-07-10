<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { aiGetConfig, aiSaveConfig, aiSaveApiKey, aiTestConnection, aiListModels } from "@/api";
import type { AiConfig } from "@/types";

const config = ref<AiConfig>({
  endpoint: "https://api.openai.com/v1",
  model: "gpt-4o",
  api_key_configured: false,
  system_prompt: null,
  temperature: 0.3,
  max_tokens: 4096,
});

const newApiKey = ref("");
const testing = ref(false);
const saving = ref(false);
const testResult = ref<boolean | null>(null);
const saveMsg = ref("");

const models = ref<string[]>([]);
const fetchingModels = ref(false);
const showModelDropdown = ref(false);
const modelError = ref("");
const modelSearch = ref("");

const filteredModels = computed(() => {
  if (!modelSearch.value) return models.value;
  const q = modelSearch.value.toLowerCase();
  return models.value.filter((m) => m.toLowerCase().includes(q));
});

function onFocusModel() {
  if (models.value.length > 0) {
    modelSearch.value = "";
    showModelDropdown.value = true;
  }
}

function onInputModel() {
  showModelDropdown.value = true;
}

function selectModel(model: string) {
  config.value.model = model;
  modelSearch.value = "";
  showModelDropdown.value = false;
}

function onBlurDropdown() {
  setTimeout(() => { showModelDropdown.value = false; }, 200);
}

function onKeydownModel(e: KeyboardEvent) {
  if (!showModelDropdown.value || filteredModels.value.length === 0) return;
  if (e.key === "Escape") { showModelDropdown.value = false; }
  if (e.key === "Enter") {
    const first = filteredModels.value[0];
    if (first) selectModel(first);
    e.preventDefault();
  }
}

function highlight(text: string, query: string): string {
  if (!query) return text;
  const re = new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
  return text.replace(re, '<mark>$1</mark>');
}

const presets = [
  { name: "OpenAI", endpoint: "https://api.openai.com/v1", model: "gpt-4o" },
  { name: "DeepSeek", endpoint: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  { name: "通义千问", endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1", model: "qwen-plus" },
  { name: "Moonshot", endpoint: "https://api.moonshot.cn/v1", model: "moonshot-v1-8k" },
  { name: "Ollama 本地", endpoint: "http://localhost:11434/v1", model: "llama3" },
];

onMounted(async () => {
  try {
    config.value = await aiGetConfig();
  } catch {
    // use defaults
  }
});

function applyPreset(p: (typeof presets)[number]) {
  config.value.endpoint = p.endpoint;
  config.value.model = p.model;
  models.value = [];
}

async function handleFetchModels() {
  if (!config.value.endpoint) {
    modelError.value = "请先填写 API 端点";
    return;
  }

  fetchingModels.value = true;
  modelError.value = "";
  models.value = [];

  try {
    if (newApiKey.value.trim()) {
      await aiSaveApiKey(newApiKey.value.trim());
      newApiKey.value = "";
      config.value.api_key_configured = true;
    }

    const result = await aiListModels(config.value.endpoint);
    models.value = result;
    if (result.length === 0) {
      modelError.value = "未获取到模型，请检查端点地址和 API Key";
    }
    showModelDropdown.value = true;
  } catch (e: any) {
    modelError.value = e?.toString() || "获取模型列表失败";
  } finally {
    fetchingModels.value = false;
  }
}


async function handleSave() {
  saving.value = true;
  saveMsg.value = "";
  const hasNewKey = !!newApiKey.value;
  try {
    await aiSaveConfig(config.value);
    if (hasNewKey) {
      await aiSaveApiKey(newApiKey.value);
      newApiKey.value = "";
      config.value.api_key_configured = true;
    }
    saveMsg.value = "✓ 设置已保存";

    if (hasNewKey && config.value.endpoint) {
      await handleFetchModels();
    }
  } catch (e: any) {
    saveMsg.value = "保存失败: " + (e?.toString() ?? "未知错误");
  } finally {
    saving.value = false;
  }
}

async function handleTest() {
  testing.value = true;
  testResult.value = null;
  try {
    testResult.value = await aiTestConnection();
  } catch {
    testResult.value = false;
  } finally {
    testing.value = false;
  }
}
</script>

<template>
  <div class="ai-settings">
    <div class="presets">
      <label>预设配置</label>
      <div class="preset-btns">
        <button
          v-for="p in presets"
          :key="p.name"
          class="btn btn-sm"
          @click="applyPreset(p)"
        >
          {{ p.name }}
        </button>
      </div>
    </div>

    <div class="field">
      <label>API 端点</label>
      <input
        v-model="config.endpoint"
        class="input"
        type="text"
        placeholder="https://api.openai.com/v1"
      />
    </div>

    <div class="field">
      <label>API Key</label>
      <input
        v-model="newApiKey"
        class="input"
        type="password"
        :placeholder="config.api_key_configured ? '•••••• (已设置，输入新 Key 替换)' : '输入 API Key'"
      />
      <p class="hint">API Key 经 AES-256-GCM 加密后写入配置文件，不会明文存储。</p>
    </div>

    <div class="field">
      <label>模型</label>
      <div class="model-row">
        <div class="model-input-wrap">
          <input
            :value="showModelDropdown ? modelSearch : config.model"
            class="input"
            type="text"
            :placeholder="config.model || 'gpt-4o'"
            @focus="onFocusModel"
            @input="modelSearch = ($event.target as HTMLInputElement).value; onInputModel()"
            @blur="onBlurDropdown"
            @keydown="onKeydownModel"
          />
          <div v-if="showModelDropdown && models.length > 0" class="model-dropdown">
            <div v-if="modelSearch" class="model-search-hint">
              搜索 "{{ modelSearch }}" — {{ filteredModels.length }} 个结果
            </div>
            <div
              v-for="m in filteredModels"
              :key="m"
              class="model-item"
              :class="{ selected: m === config.model }"
              @mousedown.prevent="selectModel(m)"
            >
              <span v-if="modelSearch" v-html="highlight(m, modelSearch)" />
              <span v-else>{{ m }}</span>
            </div>
            <div v-if="filteredModels.length === 0" class="model-empty">
              无匹配结果
            </div>
          </div>
        </div>
        <button
          class="btn btn-sm"
          :disabled="fetchingModels"
          @click="handleFetchModels"
          style="color: var(--color-primary); border-color: var(--color-primary);"
        >
          {{ fetchingModels ? "获取中..." : "获取模型" }}
        </button>
      </div>
      <p v-if="modelError" class="model-error">{{ modelError }}</p>
      <p v-else-if="models.length > 0" class="model-count">共 {{ models.length }} 个可用模型</p>
      <p v-else class="hint">填写端点并保存 API Key 后，点击"获取模型"自动列出可用模型</p>
    </div>

    <div class="field">
      <label>Temperature: {{ config.temperature }}</label>
      <input
        v-model.number="config.temperature"
        type="range"
        min="0"
        max="2"
        step="0.1"
        class="range-input"
      />
    </div>

    <div class="field">
      <label>Max Tokens</label>
      <input
        v-model.number="config.max_tokens"
        class="input"
        type="number"
        min="256"
        max="32768"
      />
    </div>

    <div class="actions">
      <button class="btn" :disabled="testing" @click="handleTest">
        {{ testing ? "测试中..." : "测试连接" }}
      </button>
      <button class="btn btn-primary" :disabled="saving" @click="handleSave">
        {{ saving ? "保存中..." : "保存设置" }}
      </button>
    </div>

    <div v-if="testResult !== null" class="test-result" :class="{ success: testResult, fail: !testResult }">
      {{ testResult ? "✓ 连接成功" : "✗ 连接失败" }}
    </div>

    <div v-if="saveMsg" class="save-msg">{{ saveMsg }}</div>
  </div>
</template>

<style scoped>
.ai-settings {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.field label {
  font-weight: 600;
  font-size: 13px;
}

.hint {
  font-size: 11px;
  color: var(--color-text-tertiary);
}

.model-row {
  display: flex;
  gap: var(--space-1);
}

.model-input-wrap {
  flex: 1;
  position: relative;
}

.model-input-wrap .input {
  width: 100%;
}

.model-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  max-height: 200px;
  overflow-y: auto;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  z-index: 10;
  margin-top: var(--space-1);
}

.model-item {
  padding: var(--space-2) var(--space-3);
  font-size: 13px;
  cursor: pointer;
  transition: background var(--transition-fast);
}

.model-item:hover {
  background: var(--color-primary-light);
}

.model-item.selected {
  background: var(--color-primary-light);
  color: var(--color-primary);
  font-weight: 600;
}

.model-error {
  font-size: 11px;
  color: var(--color-danger);
  margin-top: 2px;
}

.model-count {
  font-size: 11px;
  color: var(--color-success);
  margin-top: 2px;
}

.model-search-hint {
  padding: var(--space-1) var(--space-3);
  font-size: 11px;
  color: var(--color-text-tertiary);
  background: var(--color-surface-hover);
  border-bottom: 1px solid var(--color-border);
  position: sticky;
  top: 0;
  z-index: 1;
}

.model-empty {
  padding: var(--space-3);
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}

.model-item :deep(mark) {
  background: #fff3b0;
  color: inherit;
  padding: 0 2px;
  border-radius: 2px;
}

.presets label {
  font-weight: 600;
  font-size: 13px;
  display: block;
  margin-bottom: var(--space-1);
}

.preset-btns {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
}

.actions {
  display: flex;
  gap: var(--space-2);
}

.range-input {
  width: 100%;
  accent-color: var(--color-primary);
}

.test-result {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
}

.test-result.success {
  background: var(--color-success-light);
  color: #065f46;
}

.test-result.fail {
  background: var(--color-danger-light);
  color: #991b1b;
}

.save-msg {
  font-size: 13px;
  color: var(--color-success);
  font-weight: 500;
}
</style>
