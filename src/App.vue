<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useRouter, useRoute } from "vue-router";
import { RouterView } from "vue-router";
import { useUpdateStore } from "@/stores/useUpdateStore";
import CommandPalette from "@/components/command/CommandPalette.vue";
import NotificationManager from "@/components/notification/NotificationManager.vue";

const router = useRouter();
const route = useRoute();
const updates = useUpdateStore();
const commandPaletteRef = ref<InstanceType<typeof CommandPalette> | null>(null);

function goToSettings() {
  if (route.path !== "/settings") {
    router.push("/settings");
  }
}

function openCommandPalette() {
  commandPaletteRef.value?.open();
}

onMounted(() => {
  window.__goToSettings = goToSettings;
  window.__openCommandPalette = openCommandPalette;
  void updates.maybeCheckForUpdatesInBackground();
});

onUnmounted(() => {
  delete window.__goToSettings;
  delete window.__openCommandPalette;
});
</script>

<template>
  <RouterView v-slot="{ Component, route: matchedRoute }">
    <component
      :is="Component"
      :key="matchedRoute.name === 'pr-detail' ? matchedRoute.path : undefined"
    />
  </RouterView>
  <CommandPalette ref="commandPaletteRef" />
  <NotificationManager />
</template>
