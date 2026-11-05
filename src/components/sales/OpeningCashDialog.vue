<script setup lang="ts">
import { ref, watch } from "vue";
import { Wallet } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";

// First thing on a new shift: count what's in the drawer. The usual float
// is filled in; the count is what End shift measures against.
const props = defineProps<{ expected: number }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ counted: [amount: number] }>();

const amount = ref("");
const busy = ref(false);
const error = ref("");

watch(open, (isOpen) => {
  if (!isOpen) return;
  amount.value = String(props.expected);
  error.value = "";
});

const submit = async () => {
  busy.value = true;
  error.value = "";
  try {
    const n = Number(amount.value) || 0;
    await call("count_opening_cash", { amount: n });
    open.value = false;
    emit("counted", n);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-sm">
      <form class="space-y-5" @submit.prevent="submit">
        <div class="flex items-center gap-3">
          <span class="grid size-10 place-items-center rounded-full bg-success/12 text-success"><Wallet :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("shiftStart.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("shiftStart.hint", { amount: formatCurrency(expected) }) }}</DialogDescription>
          </div>
        </div>
        <input
          v-model="amount"
          type="number"
          min="0"
          step="any"
          dir="ltr"
          autofocus
          :aria-label="$t('shiftStart.counted')"
          class="num h-16 w-full rounded-xl border-2 border-input bg-card px-4 text-center text-3xl font-semibold outline-none focus:border-primary"
        />
        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="open = false">{{ $t("shiftStart.later") }}</Button>
          <Button type="submit" :disabled="busy || amount === ''">{{ $t("shiftStart.confirm") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
