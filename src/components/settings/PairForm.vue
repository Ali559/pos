<script setup lang="ts">
import { ref } from "vue";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { call, inDesktopApp } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

// Pairs this computer with the main computer as an extra till. Used in
// Settings → Tills and on the first-run screen. The app restarts when done.
defineProps<{ warn?: boolean }>();

const address = ref("");
const code = ref("");
const name = ref("");
const found = ref<{ shop: string | null; version: string } | null>(null);
const busy = ref(false);
const error = ref("");

const check = async () => {
  busy.value = true;
  error.value = "";
  found.value = null;
  try {
    found.value = await call<{ shop: string | null; version: string }>("probe_main", { address: address.value });
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};

const pair = async () => {
  busy.value = true;
  error.value = "";
  try {
    await call("pair_with_main", { address: address.value, code: code.value, name: name.value });
  } catch (e) {
    error.value = errorMessage(e);
    busy.value = false;
  }
};
</script>

<template>
  <form class="space-y-4" @submit.prevent="pair">
    <div class="flex flex-wrap items-end gap-3">
      <div class="space-y-2">
        <Label for="main-address">{{ $t("tills.mainAddress") }}</Label>
        <Input id="main-address" v-model="address" dir="ltr" class="w-56" placeholder="192.168.1.20:7420" @change="found = null" />
      </div>
      <Button type="button" variant="outline" :disabled="!address.trim() || busy || !inDesktopApp" @click="check">{{ $t("tills.check") }}</Button>
    </div>
    <p v-if="found" class="text-sm text-success">
      {{ $t("tills.found", { shop: found.shop ?? "—", version: found.version }) }}
    </p>
    <div class="flex flex-wrap gap-3">
      <div class="space-y-2">
        <Label for="pair-code">{{ $t("tills.code") }}</Label>
        <Input id="pair-code" v-model="code" dir="ltr" inputmode="numeric" maxlength="6" class="num w-32 font-mono tracking-widest" placeholder="000000" />
      </div>
      <div class="space-y-2">
        <Label for="till-name">{{ $t("tills.tillName") }}</Label>
        <Input id="till-name" v-model="name" dir="auto" class="w-56" :placeholder="$t('tills.tillNamePlaceholder')" />
      </div>
    </div>
    <p v-if="warn" class="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-800 dark:text-amber-300">{{ $t("tills.joinWarning") }}</p>
    <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
    <Button type="submit" :disabled="busy || !inDesktopApp || !address.trim() || code.trim().length !== 6 || !name.trim()">
      <Loader2 v-if="busy" class="animate-spin" /> {{ $t("tills.connect") }}
    </Button>
  </form>
</template>
