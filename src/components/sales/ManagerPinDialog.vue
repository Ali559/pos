<script setup lang="ts">
import { ref } from "vue";
import { ShieldCheck } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

// A manager types their PIN to approve one action the cashier can't do
// alone. `ask` resolves with a single-use token, or null if cancelled.
export interface Grant {
  token: string;
  approverName: string;
}

const open = ref(false);
const permission = ref("");
const pin = ref("");
const busy = ref(false);
const error = ref("");
let settle: ((g: Grant | null) => void) | null = null;

const ask = (perm: string) =>
  new Promise<Grant | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    permission.value = perm;
    pin.value = "";
    error.value = "";
    open.value = true;
  });

const finish = (g: Grant | null) => {
  settle?.(g);
  settle = null;
  open.value = false;
};

const submit = async () => {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    finish(await call<Grant>("authorize_override", { pin: pin.value, permission: permission.value }));
  } catch (e) {
    error.value = errorMessage(e);
    pin.value = "";
  } finally {
    busy.value = false;
  }
};

defineExpose({ ask });
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && finish(null)">
    <DialogContent class="max-w-sm">
      <form class="space-y-4" @submit.prevent="submit">
        <div class="flex items-center gap-3">
          <span class="grid size-10 place-items-center rounded-full bg-primary/10 text-primary"><ShieldCheck :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("approval.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">
              {{ $t(`approval.why.${permission.replace("sales.", "")}`) }}
            </DialogDescription>
          </div>
        </div>
        <Input
          v-model="pin"
          type="password"
          inputmode="numeric"
          autocomplete="off"
          maxlength="6"
          dir="ltr"
          autofocus
          :aria-label="$t('approval.pin')"
          :placeholder="$t('approval.pin')"
          class="num h-12 text-center text-2xl tracking-[0.5em]"
        />
        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="finish(null)">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="busy || pin.length < 4">{{ $t("approval.approve") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
