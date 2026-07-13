<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { RouterView } from "vue-router";

const router = useRouter();
const route = useRoute();

function goToSettings() {
  if (route.path !== "/settings") {
    router.push("/settings");
  }
}

onMounted(() => {
  window.__goToSettings = goToSettings;
});

onUnmounted(() => {
  delete window.__goToSettings;
});
</script>

<template>
  <RouterView />
</template>
