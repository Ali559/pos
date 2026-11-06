<script setup lang="ts">
import { ref } from "vue";
import { Tag } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";

// The price of something sold at a typed price (the misc item), and an
// optional note of what it was. Resolves with both, or null.
const open = ref(false);
const name = ref("");
const price = ref("");
const note = ref("");
let settle: ((r: { price: number; note: string } | null) => void) | null = null;

const ask = (opts: { name: string }) =>
  new Promise<{ price: number; note: string } | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    name.value = opts.name;
    price.value = "";
    note.value = "";
    open.value = true;
  });

const finish = (r: { price: number; note: string } | null) => {
  settle?.(r);
  settle = null;
  open.value = false;
};

const submit = () => {
  const n = Number(price.value);
  if (n > 0) finish({ price: n, note: note.value.trim() });
};

defineExpose({ ask });
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && finish(null)">
    <DialogContent class="max-w-sm">
      <form class="space-y-4" @submit.prevent="submit">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><Tag :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold"><bdi>{{ name }}</bdi></DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("openPrice.hint") }}</DialogDescription>
          </div>
        </div>
        <input
          v-model="price"
          type="number"
          min="0"
          step="any"
          dir="ltr"
          autofocus
          :aria-label="$t('openPrice.price')"
          :placeholder="$t('openPrice.price')"
          class="num h-16 w-full rounded-xl border-2 border-input bg-card px-4 text-center text-3xl font-semibold outline-none placeholder:text-base placeholder:font-normal focus:border-primary"
        />
        <Input v-model="note" dir="auto" maxlength="60" :placeholder="$t('openPrice.note')" :aria-label="$t('openPrice.note')" />
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="finish(null)">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="!(Number(price) > 0)">{{ $t("openPrice.add") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
