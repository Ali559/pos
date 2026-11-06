<script setup lang="ts">
import { computed } from "vue";
import { formatCurrency } from "@/lib/format";
import { formatDate, formatDateTime } from "@/lib/locale";
import { printingStatement } from "@/lib/print";
import { useShop } from "@/stores/shop";

// A customer's statement on paper. Shares #print-root with receipts (only
// one prints at a time), so the same print CSS applies.
const { profile } = useShop();
const s = computed(() => printingStatement.value);
const balance = computed(() => s.value?.entries[s.value.entries.length - 1]?.running ?? 0);
</script>

<template>
  <Teleport to="body">
    <div v-if="s" id="print-root" :style="{ width: `${s.width}mm` }" :class="s.width === 58 ? 'text-[10px]' : 'text-[12px]'">
      <header class="text-center">
        <p class="text-[1.4em] font-bold">{{ profile?.name }}</p>
        <p v-if="profile?.phone" dir="ltr">{{ profile.phone }}</p>
        <p class="mt-1 font-bold">{{ $t("customers.statement") }}</p>
      </header>
      <div class="rule my-1.5" />
      <dl class="grid grid-cols-[auto_1fr] gap-x-2">
        <dt>{{ $t("customers.fields.name") }}</dt><dd class="text-end"><bdi>{{ s.customer.name }}</bdi></dd>
        <template v-if="s.customer.phone">
          <dt>{{ $t("customers.fields.phone") }}</dt><dd class="text-end" dir="ltr">{{ s.customer.phone }}</dd>
        </template>
        <dt>{{ $t("receipt.date") }}</dt><dd class="text-end">{{ formatDateTime(new Date()) }}</dd>
      </dl>
      <div class="rule my-1.5" />
      <table class="w-full">
        <tbody>
          <tr v-for="e in s.entries" :key="e.id" class="align-top">
            <td class="pe-1">
              {{ formatDate(new Date(e.created_at)) }}
              <span class="block">{{ $t(`customers.kinds.${e.kind}`) }}<template v-if="e.receipt_number"> <span dir="ltr">{{ e.receipt_number }}</span></template></span>
              <span v-if="e.note" class="block italic">{{ e.note }}</span>
            </td>
            <td class="text-end whitespace-nowrap">{{ e.amount > 0 ? "+" : "−" }}{{ formatCurrency(Math.abs(e.amount)) }}</td>
            <td class="ps-1 text-end whitespace-nowrap">{{ formatCurrency(e.running) }}</td>
          </tr>
        </tbody>
      </table>
      <div class="rule my-1.5" />
      <p class="flex justify-between text-[1.3em] font-bold">
        <span>{{ $t("customers.balance") }}</span><span>{{ formatCurrency(balance) }}</span>
      </p>
    </div>
  </Teleport>
</template>
