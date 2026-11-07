<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Download, Loader2, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { available, installUpdate, progress, startUpdateChecks } from "@/lib/updates";
import { errorMessage } from "@/lib/errors";

// A thin bar over the app when a new version is ready. Installing restarts
// the app, so it's always the cashier's choice when.
const hidden = ref<string | null>(null);
const error = ref("");

onMounted(startUpdateChecks);

const install = async () => {
  error.value = "";
  try {
    await installUpdate();
  } catch (e) {
    error.value = errorMessage(e);
    progress.value = null;
  }
};
</script>

<template>
  <div
    v-if="available && hidden !== available.version"
    role="status"
    class="flex items-center gap-3 border-b bg-primary/10 px-5 py-2 text-sm"
  >
    <Download :size="16" class="shrink-0 text-primary" />
    <span class="min-w-0 flex-1">
      {{ $t("updates.ready", { version: available.version }) }}
      <span class="text-muted-foreground">{{ $t("updates.restartHint") }}</span>
      <span v-if="error" class="ms-2 text-destructive">{{ error }}</span>
    </span>
    <Button size="sm" :disabled="progress !== null" @click="install">
      <Loader2 v-if="progress !== null" class="animate-spin" />
      {{ progress !== null ? $t("updates.installing", { percent: Math.round((progress ?? 0) * 100) }) : $t("updates.install") }}
    </Button>
    <button
      v-if="progress === null"
      type="button"
      class="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
      :aria-label="$t('updates.later')"
      :title="$t('updates.later')"
      @click="hidden = available.version"
    >
      <X :size="14" />
    </button>
  </div>
</template>
