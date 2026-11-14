<script setup lang="ts">
import { computed } from "vue";
import { code128Bars } from "@/lib/code128";
import { formatCurrency, formatUsd } from "@/lib/format";
import { lineSold, paymentText } from "@/db/sales";
import { formatDateTime } from "@/lib/locale";
import { printing } from "@/lib/print";
import { useShop } from "@/stores/shop";

// The paper receipt. Only visible while printing (see index.css).
const { profile } = useShop();
const p = computed(() => printing.value);
const barcode = computed(() => (p.value ? code128Bars(p.value.sale.receipt_number) : null));
</script>

<template>
  <Teleport to="body">
    <div v-if="p" id="print-root" :style="{ width: `${p.width}mm` }" :class="p.width === 58 ? 'text-[10px]' : 'text-[12px]'">
      <header class="text-center">
        <img v-if="p.logo" :src="p.logo" alt="" class="mx-auto mb-1 max-h-14 max-w-[60%] object-contain grayscale" />
        <p class="text-[1.4em] font-bold">{{ profile?.name }}</p>
        <p v-if="profile?.address">{{ profile.address }}</p>
        <p v-if="profile?.phone || profile?.phone_alt" dir="ltr">{{ [profile?.phone, profile?.phone_alt].filter(Boolean).join(" / ") }}</p>
        <p v-if="profile?.tax_number">{{ $t("receipt.taxNumber") }}: <span dir="ltr">{{ profile.tax_number }}</span></p>
      </header>

      <p v-if="p.copy" class="my-1 border border-black text-center font-bold">{{ $t("receipt.copy") }}</p>
      <div class="rule my-1.5" />
      <dl class="grid grid-cols-[auto_1fr] gap-x-2">
        <dt>{{ $t("receipt.number") }}</dt><dd class="text-end" dir="ltr">{{ p.sale.receipt_number }}</dd>
        <dt>{{ $t("receipt.date") }}</dt><dd class="text-end">{{ formatDateTime(new Date(p.sale.sale_date)) }}</dd>
        <dt>{{ $t("receipt.cashier") }}</dt><dd class="text-end">{{ p.sale.cashier }}</dd>
        <template v-if="p.sale.customer_name">
          <dt>{{ $t("receipt.customer") }}</dt><dd class="text-end"><bdi>{{ p.sale.customer_name }}</bdi></dd>
        </template>
      </dl>
      <div class="rule my-1.5" />

      <table class="w-full">
        <tbody>
          <template v-for="l in p.sale.lines" :key="l.id">
            <tr><td colspan="2" class="pt-0.5 font-semibold"><bdi>{{ l.name }}</bdi></td></tr>
            <tr>
              <td class="text-start"><bdi>{{ lineSold(l).label }}</bdi> × {{ formatCurrency(lineSold(l).each) }}</td>
              <td class="text-end">{{ formatCurrency(l.subtotal + l.discount) }}</td>
            </tr>
            <tr v-if="l.promo_discount > 0">
              <td class="ps-2"><bdi>{{ l.promotion_name }}</bdi></td>
              <td class="text-end">−{{ formatCurrency(l.promo_discount) }}</td>
            </tr>
            <tr v-if="l.discount - l.promo_discount > 0.0001">
              <td class="ps-2">{{ $t("receipt.discount") }}</td>
              <td class="text-end">−{{ formatCurrency(l.discount - l.promo_discount) }}</td>
            </tr>
          </template>
        </tbody>
      </table>
      <div class="rule my-1.5" />

      <dl class="grid grid-cols-[1fr_auto] gap-x-2">
        <dt>{{ $t("pos.subtotal") }}</dt><dd class="text-end">{{ formatCurrency(p.sale.subtotal) }}</dd>
        <template v-if="p.sale.discount > 0">
          <dt>{{ $t("receipt.discount") }}</dt><dd class="text-end">−{{ formatCurrency(p.sale.discount) }}</dd>
        </template>
        <template v-if="p.sale.tax > 0">
          <dt>{{ $t("pos.tax") }}</dt><dd class="text-end">{{ formatCurrency(p.sale.tax) }}</dd>
        </template>
        <dt class="pt-1 text-[1.3em] font-bold">{{ $t("pos.total") }}</dt>
        <dd class="pt-1 text-end text-[1.3em] font-bold">{{ formatCurrency(p.sale.total) }}</dd>
        <template v-for="(pay, i) in p.sale.payments" :key="i">
          <dt>{{ $t(`pos.${pay.method}`) }}</dt>
          <dd class="text-end" dir="ltr">{{ paymentText(pay, formatCurrency, formatUsd) }}</dd>
        </template>
        <template v-if="p.sale.change">
          <dt>{{ $t("cash.change") }}</dt><dd class="text-end">{{ formatCurrency(p.sale.change) }}</dd>
        </template>
        <template v-if="p.sale.refunded > 0">
          <dt>{{ $t("receipt.refunded") }}</dt><dd class="text-end">−{{ formatCurrency(p.sale.refunded) }}</dd>
        </template>
        <template v-if="p.sale.customer_balance">
          <dt class="pt-1 font-bold">{{ $t("receipt.balanceOwed") }}</dt><dd class="pt-1 text-end font-bold">{{ formatCurrency(p.sale.customer_balance) }}</dd>
        </template>
        <template v-if="p.sale.points_earned > 0">
          <dt>{{ $t("loyalty.earnedOnReceipt") }}</dt><dd class="text-end">+{{ p.sale.points_earned }}</dd>
        </template>
        <template v-if="!p.copy && p.sale.points_balance">
          <dt>{{ $t("loyalty.balanceOnReceipt") }}</dt><dd class="text-end">{{ p.sale.points_balance }}</dd>
        </template>
      </dl>

      <svg
        v-if="barcode"
        class="mx-auto mt-3 block h-12 w-[85%]"
        :viewBox="`-10 0 ${barcode.width + 20} 40`"
        preserveAspectRatio="none"
        role="img"
        :aria-label="p.sale.receipt_number"
      >
        <rect v-for="([x, w], i) in barcode.bars" :key="i" :x="x" :width="w" y="0" height="40" fill="#000" />
      </svg>
      <p class="text-center" dir="ltr">{{ p.sale.receipt_number }}</p>
      <p v-if="profile?.receipt_footer" class="mt-2 text-center whitespace-pre-line">{{ profile.receipt_footer }}</p>
    </div>
  </Teleport>
</template>
