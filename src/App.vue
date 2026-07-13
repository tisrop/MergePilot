<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { RouterView } from "vue-router";
import { useUpdateStore } from "@/stores/useUpdateStore";

const router = useRouter();
const route = useRoute();
const updates = useUpdateStore();

function goToSettings() {
  if (route.path !== "/settings") {
    router.push("/settings");
  }
}

onMounted(() => {
  window.__goToSettings = goToSettings;
  void updates.maybeCheckForUpdatesInBackground();
});

onUnmounted(() => {
  delete window.__goToSettings;
});
</script>

<template>
  <RouterView />
</template>
