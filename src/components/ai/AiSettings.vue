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

// Model list
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
    // If user entered a new API key but hasn't saved yet, save it first
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

    // Auto-fetch models after saving config + key
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
    <!-- Presets -->
    <div class="presets">
      <label>预设配置</label>
      <div class="preset-btns">
        <button
          v-for="p in presets"
          :key="p.name"
          @click="applyPreset(p)"
        >
          {{ p.name }}
        </button>
      </div>
    </div>

    <!-- Endpoint -->
    <div class="field">
      <label>API 端点</label>
      <input
        v-model="config.endpoint"
        type="text"
        placeholder="https://api.openai.com/v1"
      />
    </div>

    <!-- API Key -->
    <div class="field">
      <label>API Key</label>
      <input
        v-model="newApiKey"
        type="password"
        :placeholder="config.api_key_configured ? '●●●●●● (已设置，输入新 Key 替换)' : '输入 API Key'"
      />
      <p class="hint">
        API Key 经 AES-256-GCM 加密后写入配置文件，不会明文存储。
      </p>
    </div>

    <!-- Model with fetch button -->
    <div class="field">
      <label>模型</label>
      <div class="model-row">
        <div class="model-input-wrap">
          <input
            :value="showModelDropdown ? modelSearch : config.model"
            type="text"
            :placeholder="config.model || 'gpt-4o'"
            @focus="onFocusModel"
            @input="modelSearch = ($event.target as HTMLInputElement).value; onInputModel()"
            @blur="onBlurDropdown"
            @keydown="onKeydownModel"
          />
          <!-- Dropdown -->
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
          class="fetch-btn"
          :disabled="fetchingModels"
          @click="handleFetchModels"
        >
          {{ fetchingModels ? "获取中..." : "获取模型" }}
        </button>
      </div>
      <p v-if="modelError" class="model-error">{{ modelError }}</p>
      <p v-else-if="models.length > 0" class="model-count">
        共 {{ models.length }} 个可用模型
      </p>
      <p v-else class="hint">填写端点并保存 API Key 后，点击"获取模型"自动列出可用模型</p>
    </div>

    <!-- Temperature -->
    <div class="field">
      <label>Temperature: {{ config.temperature }}</label>
      <input
        v-model.number="config.temperature"
        type="range"
        min="0"
        max="2"
        step="0.1"
      />
    </div>

    <!-- Max Tokens -->
    <div class="field">
      <label>Max Tokens</label>
      <input
        v-model.number="config.max_tokens"
        type="number"
        min="256"
        max="32768"
      />
    </div>

    <!-- Actions -->
    <div class="actions">
      <button :disabled="testing" @click="handleTest">
        {{ testing ? "测试中..." : "测试连接" }}
      </button>
      <button :disabled="saving" class="save-btn" @click="handleSave">
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
  gap: 16px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field label {
  font-weight: 600;
  font-size: 13px;
}

.field input[type="text"],
.field input[type="password"],
.field input[type="number"] {
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-size: 13px;
  outline: none;
}

.field input:focus {
  border-color: var(--color-primary);
}

.hint {
  font-size: 11px;
  color: var(--color-text-secondary);
}

/* Model row: input + fetch button */
.model-row {
  display: flex;
  gap: 6px;
}

.model-input-wrap {
  flex: 1;
  position: relative;
}

.model-input-wrap input {
  width: 100%;
}

.fetch-btn {
  padding: 8px 14px;
  border: 1px solid var(--color-primary);
  border-radius: 6px;
  background: none;
  color: var(--color-primary);
  font-size: 12px;
  white-space: nowrap;
  cursor: pointer;
  transition: all 0.15s;
}

.fetch-btn:hover:not(:disabled) {
  background: #e8f0fe;
}

.fetch-btn:disabled {
  opacity: 0.5;
}

/* Model dropdown */
.model-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  max-height: 200px;
  overflow-y: auto;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  z-index: 10;
  margin-top: 4px;
}

.model-item {
  padding: 8px 12px;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.1s;
}

.model-item:hover {
  background: #f0f4ff;
}

.model-item.selected {
  background: #e8f0fe;
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
  padding: 6px 12px;
  font-size: 11px;
  color: var(--color-text-secondary);
  background: #f6f8fa;
  border-bottom: 1px solid var(--color-border);
  position: sticky;
  top: 0;
  z-index: 1;
}

.model-empty {
  padding: 12px;
  text-align: center;
  color: var(--color-text-secondary);
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
  margin-bottom: 6px;
}

.preset-btns {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.preset-btns button {
  padding: 6px 12px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: none;
  font-size: 12px;
  transition: all 0.15s;
}

.preset-btns button:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.actions {
  display: flex;
  gap: 8px;
}

.actions button {
  padding: 8px 18px;
  border-radius: 6px;
  font-size: 13px;
  border: 1px solid var(--color-border);
  background: none;
}

.save-btn {
  background: var(--color-primary) !important;
  color: #fff !important;
  border-color: var(--color-primary) !important;
}

.actions button:disabled {
  opacity: 0.5;
}

.test-result {
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
}

.test-result.success {
  background: #d1f5e0;
  color: #116329;
}

.test-result.fail {
  background: #ffeaea;
  color: #86181d;
}

.save-msg {
  font-size: 13px;
  color: var(--color-success);
}
</style>
