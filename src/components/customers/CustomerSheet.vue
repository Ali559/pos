<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import type { Customer } from "@/db/customers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { useAuth } from "@/stores/auth";

// Create or edit a customer. A cashier may add one at the till (to sell on
// account) but only someone with customers.edit sets a credit limit.
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ customer: Customer | null; initialName?: string }>();
const emit = defineEmits<{ saved: [id: number] }>();

const auth = useAuth();
const canEdit = computed(() => auth.can("customers.edit"));

const form = ref({ name: "", phone: "", address: "", notes: "", creditLimit: "", isActive: true });
const saving = ref(false);
const error = ref("");

watch(open, (isOpen) => {
  if (!isOpen) return;
  error.value = "";
  const c = props.customer;
  // A search that looks like a phone number fills the phone, else the name
  const typed = props.initialName?.trim() ?? "";
  const looksLikePhone = /^[\d\s+()-]{5,}$/.test(typed);
  form.value = c
    ? { name: c.name, phone: c.phone ?? "", address: c.address ?? "", notes: c.notes ?? "", creditLimit: c.credit_limit != null ? String(c.credit_limit) : "", isActive: !!c.is_active }
    : { name: looksLikePhone ? "" : typed, phone: looksLikePhone ? typed : "", address: "", notes: "", creditLimit: "", isActive: true };
});

const save = async () => {
  const f = form.value;
  const customer = {
    name: f.name,
    phone: f.phone || null,
    address: f.address || null,
    notes: f.notes || null,
    creditLimit: f.creditLimit === "" ? null : Number(f.creditLimit),
    isActive: f.isActive,
  };
  saving.value = true;
  error.value = "";
  try {
    let id = props.customer?.id;
    if (id) await call("update_customer", { id, customer });
    else id = await call<number>("create_customer", customer);
    open.value = false;
    emit("saved", id);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};
</script>

<template>
  <EditSheet v-model:open="open" :title="customer ? $t('customers.edit') : $t('customers.add')" @submit="save">
    <div class="space-y-2">
      <Label for="c-name">{{ $t("customers.fields.name") }}</Label>
      <Input id="c-name" v-model="form.name" required dir="auto" autofocus />
    </div>
    <div class="space-y-2">
      <Label for="c-phone">{{ $t("customers.fields.phone") }}</Label>
      <Input id="c-phone" v-model="form.phone" dir="ltr" inputmode="tel" />
    </div>
    <div class="space-y-2">
      <Label for="c-address">{{ $t("customers.fields.address") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
      <Input id="c-address" v-model="form.address" dir="auto" />
    </div>
    <div v-if="canEdit" class="space-y-2">
      <Label for="c-limit">{{ $t("customers.fields.creditLimit") }}</Label>
      <Input id="c-limit" v-model="form.creditLimit" type="number" min="0" step="any" dir="ltr" class="w-48" :placeholder="$t('customers.noLimit')" />
      <p class="text-xs text-muted-foreground">{{ $t("customers.creditLimitHint") }}</p>
    </div>
    <div class="space-y-2">
      <Label for="c-notes">{{ $t("customers.fields.notes") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
      <textarea id="c-notes" v-model="form.notes" dir="auto" rows="3" class="w-full rounded-md border border-input bg-card px-3 py-2 text-sm outline-none focus-visible:border-ring" />
    </div>
    <label v-if="customer && canEdit" class="flex items-center gap-3 text-sm">
      <input v-model="form.isActive" type="checkbox" class="size-4 accent-primary" />
      <span>
        <span class="font-medium">{{ $t("customers.active") }}</span>
        <span class="block text-muted-foreground">{{ $t("customers.activeHint") }}</span>
      </span>
    </label>

    <template #footer>
      <Button type="submit" :disabled="saving">
        <Loader2 v-if="saving" class="animate-spin" />
        {{ customer ? $t("common.save") : $t("customers.add") }}
      </Button>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </template>
  </EditSheet>
</template>
