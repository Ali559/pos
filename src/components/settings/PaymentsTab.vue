<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { CreditCard, Gift } from "lucide-vue-next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { usePayments } from "@/stores/payments";

// Settings → Payments: whether the shop takes cards, and loyalty points.
const payments = usePayments();
const form = ref({ cards: false, loyalty: true, earnPer: 1000, pointValue: 10 });
const saved = ref(false);
const error = ref("");

onMounted(async () => {
  await payments.reload();
  form.value = {
    cards: payments.cards.value,
    loyalty: payments.loyalty.value,
    earnPer: payments.earnPer.value,
    pointValue: payments.pointValue.value,
  };
});

// What a customer gets back, as a share of what they spend
const giveBack = computed(() => {
  const { earnPer, pointValue } = form.value;
  return earnPer > 0 ? Math.round((pointValue / earnPer) * 1000) / 10 : 0;
});

const save = async () => {
  error.value = "";
  try {
    await payments.save({
      ...form.value,
      earnPer: Math.max(1, Number(form.value.earnPer) || 1000),
      pointValue: Math.max(0.01, Number(form.value.pointValue) || 10),
    });
    saved.value = true;
    setTimeout(() => (saved.value = false), 2000);
  } catch (e) {
    error.value = errorMessage(e);
  }
};
</script>

<template>
  <form class="space-y-5" @submit.prevent="save">
    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-2"><CreditCard :size="18" /> {{ $t("paymentsSettings.cardsTitle") }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <label class="flex items-start gap-3">
          <input v-model="form.cards" type="checkbox" class="mt-0.5 size-4 accent-primary" />
          <span>
            <span class="block text-sm font-medium">{{ $t("paymentsSettings.cards") }}</span>
            <span class="block text-xs text-muted-foreground">{{ $t("paymentsSettings.cardsHint") }}</span>
          </span>
        </label>
        <div class="rounded-lg bg-muted/60 p-3 text-xs leading-relaxed text-muted-foreground">
          <p class="mb-1 font-medium text-foreground">{{ $t("paymentsSettings.howCardsWork") }}</p>
          <p>{{ $t("paymentsSettings.howCardsWorkBody") }}</p>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-2"><Gift :size="18" /> {{ $t("paymentsSettings.loyaltyTitle") }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <label class="flex items-start gap-3">
          <input v-model="form.loyalty" type="checkbox" class="mt-0.5 size-4 accent-primary" />
          <span>
            <span class="block text-sm font-medium">{{ $t("paymentsSettings.loyalty") }}</span>
            <span class="block text-xs text-muted-foreground">{{ $t("paymentsSettings.loyaltyHint") }}</span>
          </span>
        </label>
        <div v-if="form.loyalty" class="flex flex-wrap gap-6">
          <div class="space-y-2">
            <Label for="earnPer">{{ $t("paymentsSettings.earnPer") }}</Label>
            <Input id="earnPer" v-model.number="form.earnPer" type="number" min="1" step="any" dir="ltr" class="w-40" />
          </div>
          <div class="space-y-2">
            <Label for="pointValue">{{ $t("paymentsSettings.pointValue") }}</Label>
            <Input id="pointValue" v-model.number="form.pointValue" type="number" min="0.01" step="any" dir="ltr" class="w-40" />
          </div>
        </div>
        <p v-if="form.loyalty" class="text-xs text-muted-foreground">
          {{ $t("paymentsSettings.example", { spend: formatCurrency(form.earnPer || 0), value: formatCurrency(form.pointValue || 0), percent: giveBack }) }}
        </p>
      </CardContent>
    </Card>

    <div class="flex items-center gap-3">
      <Button type="submit">{{ $t("common.save") }}</Button>
      <span v-if="saved" class="text-sm text-success">{{ $t("common.saved") }}</span>
      <span v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</span>
    </div>
  </form>
</template>
