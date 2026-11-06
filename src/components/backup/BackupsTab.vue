<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ArchiveRestore, DatabaseBackup, FolderOpen, HardDrive, TriangleAlert } from "lucide-vue-next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  backupNow,
  backupStatus,
  chooseBackupFolder,
  inspectBackup,
  restoreBackup,
  type BackupFile,
  type BackupStatus,
  type InspectedBackup,
} from "@/db/backup";
import { setSetting } from "@/db/settings";
import { errorMessage } from "@/lib/errors";
import { inDesktopApp } from "@/lib/ipc";
import { formatDate, formatTime, numberLocale } from "@/lib/locale";

const { t } = useI18n();

const status = ref<BackupStatus | null>(null);
const error = ref("");
const message = ref("");
const busy = ref(false);

const load = async () => {
  try {
    status.value = await backupStatus();
  } catch (e) {
    error.value = errorMessage(e);
  }
};
onMounted(load);

const when = (iso: string | null | undefined) => {
  if (!iso) return "";
  const d = new Date(iso);
  return `${formatDate(d)} ${formatTime(d)}`;
};
const size = (bytes: number) =>
  bytes >= 1024 * 1024
    ? `${(bytes / 1024 / 1024).toLocaleString(numberLocale(), { maximumFractionDigits: 1, numberingSystem: "latn" } as Intl.NumberFormatOptions)} MB`
    : `${Math.max(1, Math.round(bytes / 1024)).toLocaleString(numberLocale(), { numberingSystem: "latn" } as Intl.NumberFormatOptions)} KB`;

/** The stored error is a code (errors.*) or an English message */
const lastError = computed(() => {
  const e = status.value?.lastError;
  return e ? errorMessage({ code: e, message: e }) : "";
});

const run = async (fn: () => Promise<unknown>, done?: string) => {
  busy.value = true;
  error.value = "";
  message.value = "";
  try {
    await fn();
    if (done) message.value = done;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
    await load();
  }
};

const backUp = () => run(backupNow, t("backup.done"));
const pickFolder = () => run(async () => {
  if (await chooseBackupFolder()) message.value = t("backup.folderChanged");
});
const resetFolder = () => run(() => chooseBackupFolder(true), t("backup.folderChanged"));

// Settings: automatic backups, how many to keep
const auto = ref(true);
const keep = ref(1);
const saveOptions = () => run(async () => {
  await setSetting("backup_auto", auto.value ? "1" : "0");
  await setSetting("backup_keep", String(Math.min(365, Math.max(1, Math.round(keep.value) || 1))));
}, t("common.saved"));
onMounted(async () => {
  await load();
  if (status.value) {
    auto.value = status.value.auto;
    keep.value = status.value.keep;
  }
});

// Restore: check the file first, then confirm
const candidate = ref<InspectedBackup | null>(null);
const confirmOpen = ref(false);
const restoreError = ref("");
const inspect = (file: BackupFile | null) => run(async () => {
  const found = await inspectBackup(file?.name ?? null);
  if (!found) return;
  candidate.value = found;
  restoreError.value = "";
  confirmOpen.value = true;
});
const restoring = ref(false);
const confirmRestore = async () => {
  if (!candidate.value) return;
  restoring.value = true;
  restoreError.value = "";
  try {
    // The app restarts with the restored data; nothing after this runs
    await restoreBackup(candidate.value.token);
  } catch (e) {
    restoreError.value = errorMessage(e);
    restoring.value = false;
  }
};

const kindLabel = (f: BackupFile) =>
  f.manifest?.kind === "before_restore" ? t("backup.kindBeforeRestore") : f.manifest?.kind === "manual" ? t("backup.kindManual") : t("backup.kindAuto");
</script>

<template>
  <div class="space-y-5">
    <p v-if="!inDesktopApp" class="rounded-lg border border-warning/40 bg-warning/10 p-3 text-sm">
      {{ $t("backup.desktopOnly") }}
    </p>

    <Card>
      <CardHeader>
        <CardTitle class="flex items-center gap-2"><DatabaseBackup :size="18" />{{ $t("backup.title") }}</CardTitle>
      </CardHeader>
      <CardContent v-if="status" class="space-y-4">
        <div class="flex flex-wrap items-center gap-4">
          <div class="min-w-0 flex-1">
            <p v-if="status.lastOkAt" class="font-medium">{{ $t("backup.lastOk", { when: when(status.lastOkAt) }) }}</p>
            <p v-else class="font-medium">{{ $t("backup.never") }}</p>
            <p class="text-sm text-muted-foreground">{{ status.auto ? $t("backup.autoOn") : $t("backup.autoOff") }}</p>
          </div>
          <Button :disabled="busy" @click="backUp">
            <DatabaseBackup />{{ busy ? $t("backup.working") : $t("backup.now") }}
          </Button>
        </div>
        <p v-if="lastError" role="alert" class="flex items-start gap-2 rounded-lg bg-destructive/10 p-3 text-sm text-destructive">
          <TriangleAlert :size="16" class="mt-0.5 shrink-0" />
          <span>{{ $t("backup.lastFailed", { when: when(status.lastAttemptAt) }) }} {{ lastError }}</span>
        </p>
        <p v-if="message" role="status" class="text-sm text-success">{{ message }}</p>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      </CardContent>
      <CardContent v-else-if="error">
        <p role="alert" class="text-sm text-destructive">{{ error }}</p>
      </CardContent>
    </Card>

    <Card v-if="status">
      <CardHeader>
        <CardTitle class="flex items-center gap-2"><HardDrive :size="18" />{{ $t("backup.folder") }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <p dir="ltr" class="break-all rounded-md bg-muted px-3 py-2 text-start font-mono text-xs">{{ status.dir }}</p>
        <p v-if="status.isDefaultDir" class="flex items-start gap-2 text-sm text-warning">
          <TriangleAlert :size="16" class="mt-0.5 shrink-0" />{{ $t("backup.defaultDirWarning") }}
        </p>
        <p v-else-if="!status.dirAvailable" role="alert" class="flex items-start gap-2 text-sm text-destructive">
          <TriangleAlert :size="16" class="mt-0.5 shrink-0" />{{ $t("errors.backup_folder_unavailable") }}
        </p>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" :disabled="busy" @click="pickFolder"><FolderOpen />{{ $t("backup.chooseFolder") }}</Button>
          <Button v-if="!status.isDefaultDir" variant="ghost" :disabled="busy" @click="resetFolder">{{ $t("backup.useDefault") }}</Button>
        </div>
        <form class="flex flex-wrap items-end gap-4 border-t pt-4" @submit.prevent="saveOptions">
          <label class="flex items-center gap-3 text-sm">
            <input v-model="auto" type="checkbox" class="size-4 accent-primary" />
            <span>
              <span class="font-medium">{{ $t("backup.auto") }}</span>
              <span class="block text-muted-foreground">{{ $t("backup.autoHint") }}</span>
            </span>
          </label>
          <div class="space-y-2">
            <Label for="backupKeep">{{ $t("backup.keep") }}</Label>
            <Input id="backupKeep" v-model.number="keep" type="number" min="1" max="365" dir="ltr" class="w-28" />
          </div>
          <Button type="submit" :disabled="busy">{{ $t("common.save") }}</Button>
          <p class="basis-full text-xs text-muted-foreground">{{ $t("backup.keepHint") }}</p>
        </form>
      </CardContent>
    </Card>

    <Card v-if="status">
      <CardHeader>
        <CardTitle class="flex items-center gap-2"><ArchiveRestore :size="18" />{{ $t("backup.restoreTitle") }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-3">
        <p class="text-sm text-muted-foreground">{{ $t("backup.restoreHint") }}</p>
        <ul v-if="status.files.length" class="divide-y rounded-lg border">
          <li v-for="f in status.files" :key="f.name" class="flex flex-wrap items-center gap-3 px-4 py-3">
            <div class="min-w-0 flex-1">
              <p class="font-medium">{{ f.manifest ? when(f.manifest.createdAt) : f.name }}</p>
              <p class="text-xs text-muted-foreground">
                {{ kindLabel(f) }} · {{ size(f.bytes) }}
                <template v-if="f.manifest"> · {{ $t("backup.counts", { sales: f.manifest.sales, products: f.manifest.products }) }}</template>
              </p>
            </div>
            <Button variant="outline" size="sm" :disabled="busy" @click="inspect(f)">{{ $t("backup.restore") }}</Button>
          </li>
        </ul>
        <p v-else class="text-sm text-muted-foreground">{{ $t("backup.none") }}</p>
        <Button variant="ghost" :disabled="busy" @click="inspect(null)"><FolderOpen />{{ $t("backup.restoreFromFile") }}</Button>
      </CardContent>
    </Card>

    <Dialog v-model:open="confirmOpen">
      <DialogContent class="max-w-md">
        <div v-if="candidate" class="space-y-4">
          <div class="flex items-center gap-3">
            <span class="grid size-10 shrink-0 place-items-center rounded-full bg-warning/12 text-warning"><ArchiveRestore :size="20" /></span>
            <div>
              <DialogTitle class="font-semibold">{{ $t("backup.confirmTitle") }}</DialogTitle>
              <DialogDescription class="text-sm text-muted-foreground">{{ $t("backup.confirmHint") }}</DialogDescription>
            </div>
          </div>
          <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 rounded-lg bg-muted p-3 text-sm">
            <dt class="text-muted-foreground">{{ $t("backup.takenAt") }}</dt>
            <dd>{{ when(candidate.manifest.createdAt) }}</dd>
            <dt class="text-muted-foreground">{{ $t("backup.shop") }}</dt>
            <dd>{{ candidate.manifest.shopName ?? "—" }}</dd>
            <dt class="text-muted-foreground">{{ $t("backup.contents") }}</dt>
            <dd>{{ $t("backup.counts", { sales: candidate.manifest.sales, products: candidate.manifest.products }) }}</dd>
          </dl>
          <p class="text-sm">{{ $t("backup.confirmWarning") }}</p>
          <p v-if="restoreError" role="alert" class="text-sm font-medium text-destructive">{{ restoreError }}</p>
          <div class="flex justify-end gap-2">
            <Button variant="ghost" :disabled="restoring" @click="confirmOpen = false">{{ $t("common.cancel") }}</Button>
            <Button variant="destructive" :disabled="restoring" @click="confirmRestore">
              {{ restoring ? $t("backup.restoring") : $t("backup.confirmRestore") }}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>
