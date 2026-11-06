<script setup lang="ts">
import { ref, watch } from "vue";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import type { Supplier } from "@/db/suppliers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ supplier: Supplier | null }>();
const emit = defineEmits<{ saved: [id: number] }>();

const form = ref({ name: "", contactPerson: "", phone: "", address: "", notes: "", isActive: true });
const saving = ref(false);
const error = ref("");

watch(open, (v) => {
  if (!v) return;
  error.value = "";
  const s = props.supplier;
  form.value = s
    ? { name: s.name, contactPerson: s.contact_person ?? "", phone: s.phone ?? "", address: s.address ?? "", notes: s.notes ?? "", isActive: !!s.is_active }
    : { name: "", contactPerson: "", phone: "", address: "", notes: "", isActive: true };
});

const save = async () => {
  const f = form.value;
  const supplier = {
    name: f.name,
    contactPerson: f.contactPerson || null,
    phone: f.phone || null,
    address: f.address || null,
    notes: f.notes || null,
    isActive: f.isActive,
  };
  saving.value = true;
  error.value = "";
  try {
    let id = props.supplier?.id;
    if (id) await call("update_supplier", { id, supplier });
    else id = await call<number>("create_supplier", supplier);
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
  <EditSheet v-model:open="open" :title="supplier ? $t('suppliers.edit') : $t('suppliers.add')" @submit="save">
    <div class="space-y-2">
      <Label for="s-name">{{ $t("suppliers.fields.name") }}</Label>
      <Input id="s-name" v-model="form.name" required dir="auto" />
    </div>
    <div class="grid gap-4 sm:grid-cols-2">
      <div class="space-y-2">
        <Label for="s-contact">{{ $t("suppliers.fields.contact") }}</Label>
        <Input id="s-contact" v-model="form.contactPerson" dir="auto" />
      </div>
      <div class="space-y-2">
        <Label for="s-phone">{{ $t("suppliers.fields.phone") }}</Label>
        <Input id="s-phone" v-model="form.phone" dir="ltr" inputmode="tel" />
      </div>
    </div>
    <div class="space-y-2">
      <Label for="s-address">{{ $t("suppliers.fields.address") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
      <Input id="s-address" v-model="form.address" dir="auto" />
    </div>
    <div class="space-y-2">
      <Label for="s-notes">{{ $t("suppliers.fields.notes") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
      <textarea id="s-notes" v-model="form.notes" dir="auto" rows="3" class="w-full rounded-md border border-input bg-card px-3 py-2 text-sm outline-none focus-visible:border-ring" />
    </div>
    <label v-if="supplier" class="flex items-center gap-3 text-sm">
      <input v-model="form.isActive" type="checkbox" class="size-4 accent-primary" />
      <span class="font-medium">{{ $t("suppliers.active") }}</span>
    </label>
    <template #footer>
      <Button type="submit" :disabled="saving"><Loader2 v-if="saving" class="animate-spin" />{{ supplier ? $t("common.save") : $t("suppliers.add") }}</Button>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </template>
  </EditSheet>
</template>
