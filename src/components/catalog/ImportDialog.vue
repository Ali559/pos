<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FileSpreadsheet, Loader2, Upload } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { call, inDesktopApp } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

// Import products from a spreadsheet: open the file, check which column is
// which, see what would happen (nothing written), then import. Rust does
// the reading and the matching (src-tauri/src/import.rs).
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ imported: [] }>();
const { t } = useI18n();

export interface OpenedFile {
  token: string;
  fileName: string;
  headers: string[];
  sample: string[][];
  rowCount: number;
  guess: Record<string, number>;
}
interface Report {
  created: number;
  updated: number;
  skipped: number;
  errors: number;
  newCategories: string[];
  rows: { row: number; status: string; name: string; reason: string | null }[];
}

const FIELDS = ["name", "barcode", "sku", "price", "cost", "wholesale", "stock", "minStock", "category", "unit"] as const;

const file = ref<OpenedFile | null>(null);
const mapping = ref<Record<string, string>>({});
const updateExisting = ref(true);
const preview = ref<Report | null>(null);
const done = ref<Report | null>(null);
const busy = ref(false);
const error = ref("");

watch(open, (v) => {
  if (!v) return;
  file.value = null;
  preview.value = null;
  done.value = null;
  error.value = "";
});

const pick = async () => {
  error.value = "";
  busy.value = true;
  try {
    const f = await call<OpenedFile | null>("open_import_file");
    if (!f) return;
    file.value = f;
    preview.value = null;
    done.value = null;
    mapping.value = Object.fromEntries(FIELDS.map((k) => [k, f.guess[k] !== undefined ? String(f.guess[k]) : ""]));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};

const options = computed(() => ({
  token: file.value?.token,
  mapping: Object.fromEntries(Object.entries(mapping.value).filter(([, v]) => v !== "").map(([k, v]) => [k, Number(v)])),
  updateExisting: updateExisting.value,
}));
// A change to the mapping invalidates the preview
watch([mapping, updateExisting], () => (preview.value = null), { deep: true });

const run = async (dryRun: boolean) => {
  error.value = "";
  busy.value = true;
  try {
    const r = await call<Report>("import_products", { ...options.value, dryRun });
    if (dryRun) preview.value = r;
    else {
      done.value = r;
      emit("imported");
    }
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};

const shown = computed(() => done.value ?? preview.value);
const problems = computed(() => shown.value?.rows.filter((r) => r.status === "error") ?? []);
const reasonText = (code: string | null) => (code ? errorMessage({ code, message: code }) : "");
const sampleOf = (col: string) => (col === "" || !file.value ? "" : file.value.sample.map((r) => r[Number(col)] ?? "").filter(Boolean).slice(0, 2).join(", "));
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[90vh] max-w-2xl overflow-y-auto">
      <div class="space-y-4">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><FileSpreadsheet :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("importer.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("importer.hint") }}</DialogDescription>
          </div>
        </div>
        <p v-if="!inDesktopApp" class="rounded-lg bg-warning/10 p-3 text-sm">{{ $t("importer.desktopOnly") }}</p>

        <div class="flex flex-wrap items-center gap-3">
          <Button variant="outline" :disabled="busy" @click="pick"><Upload />{{ file ? $t("importer.another") : $t("importer.choose") }}</Button>
          <span v-if="file" class="text-sm"><bdi class="font-medium">{{ file.fileName }}</bdi> · {{ $t("importer.rows", file.rowCount) }}</span>
        </div>

        <template v-if="file && !done">
          <div class="overflow-hidden rounded-lg border">
            <table class="w-full text-sm">
              <thead class="bg-muted/50 text-xs text-muted-foreground">
                <tr><th class="px-3 py-2 text-start font-medium">{{ $t("importer.field") }}</th><th class="px-3 py-2 text-start font-medium">{{ $t("importer.column") }}</th><th class="px-3 py-2 text-start font-medium">{{ $t("importer.example") }}</th></tr>
              </thead>
              <tbody class="divide-y">
                <tr v-for="f in FIELDS" :key="f">
                  <td class="px-3 py-1.5 font-medium">{{ $t(`importer.fields.${f}`) }}</td>
                  <td class="px-3 py-1.5">
                    <select v-model="mapping[f]" class="h-8 w-full max-w-56 rounded-md border bg-card px-2 text-sm">
                      <option value="">{{ $t("importer.notInFile") }}</option>
                      <option v-for="(h, i) in file.headers" :key="i" :value="String(i)">{{ h || t("importer.columnN", { n: i + 1 }) }}</option>
                    </select>
                  </td>
                  <td class="max-w-48 truncate px-3 py-1.5 text-muted-foreground"><bdi>{{ sampleOf(mapping[f] ?? "") }}</bdi></td>
                </tr>
              </tbody>
            </table>
          </div>
          <label class="flex items-center gap-3 text-sm">
            <input v-model="updateExisting" type="checkbox" class="size-4 accent-primary" />
            <span>
              <span class="font-medium">{{ $t("importer.update") }}</span>
              <span class="block text-muted-foreground">{{ $t("importer.updateHint") }}</span>
            </span>
          </label>
        </template>

        <div v-if="shown" class="space-y-2 rounded-lg border p-4" role="status">
          <p class="font-medium">{{ done ? $t("importer.done") : $t("importer.wouldDo") }}</p>
          <dl class="grid grid-cols-4 gap-2 text-center text-sm">
            <div class="rounded-md bg-success/10 p-2"><dt class="text-xs text-muted-foreground">{{ $t("importer.created") }}</dt><dd class="num text-lg font-semibold">{{ shown.created }}</dd></div>
            <div class="rounded-md bg-primary/10 p-2"><dt class="text-xs text-muted-foreground">{{ $t("importer.updated") }}</dt><dd class="num text-lg font-semibold">{{ shown.updated }}</dd></div>
            <div class="rounded-md bg-muted p-2"><dt class="text-xs text-muted-foreground">{{ $t("importer.skipped") }}</dt><dd class="num text-lg font-semibold">{{ shown.skipped }}</dd></div>
            <div class="rounded-md bg-destructive/10 p-2"><dt class="text-xs text-muted-foreground">{{ $t("importer.problems") }}</dt><dd class="num text-lg font-semibold">{{ shown.errors }}</dd></div>
          </dl>
          <p v-if="shown.newCategories.length" class="text-sm text-muted-foreground">{{ $t("importer.newCategories", { list: shown.newCategories.join("، ") }) }}</p>
          <ul v-if="problems.length" class="max-h-40 space-y-1 overflow-y-auto text-sm">
            <li v-for="p in problems" :key="p.row" class="text-destructive">{{ $t("importer.rowN", { n: p.row }) }} <bdi v-if="p.name">({{ p.name }})</bdi>: {{ reasonText(p.reason) }}</li>
          </ul>
          <p v-if="!done && problems.length" class="text-xs text-muted-foreground">{{ $t("importer.problemsHint") }}</p>
        </div>

        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button variant="ghost" @click="open = false">{{ done ? $t("common.close") : $t("common.cancel") }}</Button>
          <template v-if="file && !done">
            <Button variant="outline" :disabled="busy" @click="run(true)"><Loader2 v-if="busy" class="animate-spin" />{{ $t("importer.check") }}</Button>
            <Button :disabled="busy || !preview || preview.created + preview.updated === 0" @click="run(false)">
              {{ $t("importer.import", { n: preview ? preview.created + preview.updated : 0 }) }}
            </Button>
          </template>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
