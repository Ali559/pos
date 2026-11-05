<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Archive,
  ArchiveRestore,
  ChevronDown,
  ChevronUp,
  CornerDownRight,
  Loader2,
  Pencil,
  Plus,
  Trash2,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import EditSheet from "@/components/EditSheet.vue";
import AppIcon from "@/components/icons/AppIcon.vue";
import IconPicker from "@/components/icons/IconPicker.vue";
import ImageField from "@/components/media/ImageField.vue";
import MediaImage from "@/components/media/MediaImage.vue";
import { ImageIcon } from "lucide-vue-next";
import type { MediaFile } from "@/lib/media";
import { listCategories, type Category } from "@/db/catalog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import { useAuth } from "@/stores/auth";

const emit = defineEmits<{ changed: [] }>();
const auth = useAuth();
const canEdit = computed(() => auth.can("catalog.categories"));

// Swatches chosen to stay readable as tile tints in both themes
const COLORS = ["#2d5a8f", "#94600c", "#5f4996", "#9a3f26", "#2f6b38", "#0e7490", "#be185d", "#57534e"];

const all = ref<Category[]>([]);
const showArchived = ref(false);
const listError = ref("");

const load = async () => {
  all.value = await listCategories(true);
};
onMounted(load);

const visible = computed(() =>
  all.value.filter((c) => showArchived.value || c.is_active),
);
const roots = computed(() => visible.value.filter((c) => c.parent_id === null));
const childrenOf = (id: number) => visible.value.filter((c) => c.parent_id === id);
const archivedCount = computed(() => all.value.filter((c) => !c.is_active).length);

const run = async (fn: () => Promise<unknown>) => {
  listError.value = "";
  try {
    await fn();
    await load();
    emit("changed");
  } catch (e) {
    listError.value = errorMessage(e);
  }
};

const move = (c: Category, offset: number) => run(() => call("move_category", { id: c.id, offset }));
const setActive = (c: Category, active: boolean) =>
  run(() => call("set_category_active", { id: c.id, active }));
const remove = (c: Category) => run(() => call("delete_category", { id: c.id }));

// ----------------------------------------------------------- edit sheet

const open = ref(false);
const editing = ref<Category | null>(null);
const form = ref({
  name: "", description: "",
  parentId: "none", iconKey: null as string | null, color: null as string | null,
});
// Optional picture; the icon stays required (it's what small places show)
const image = ref<MediaFile | null>(null);
const saving = ref(false);
const error = ref("");

const startCreate = (parent: Category | null = null) => {
  editing.value = null;
  form.value = {
    name: "", description: "",
    parentId: parent ? String(parent.id) : "none",
    iconKey: null,
    color: parent ? null : COLORS[roots.value.length % COLORS.length],
  };
  image.value = null;
  error.value = "";
  open.value = true;
};

const startEdit = (c: Category) => {
  editing.value = c;
  image.value = c.image;
  form.value = {
    name: c.name,
    description: c.description ?? "",
    parentId: c.parent_id ? String(c.parent_id) : "none",
    iconKey: c.icon_key,
    color: c.own_color,
  };
  error.value = "";
  open.value = true;
};

const parent = computed(() =>
  form.value.parentId === "none" ? null : all.value.find((c) => c.id === Number(form.value.parentId)) ?? null,
);
const parentOptions = computed(() =>
  all.value.filter((c) => c.parent_id === null && c.is_active && c.id !== editing.value?.id),
);
const hasChildren = computed(() => !!editing.value && all.value.some((c) => c.parent_id === editing.value!.id));
const suggestText = computed(() => form.value.name);
const previewColor = computed(() => form.value.color ?? parent.value?.color ?? null);

const save = async () => {
  if (!form.value.iconKey) {
    error.value = errorMessage({ code: "icon_required", message: "" });
    return;
  }
  const category = {
    name: form.value.name,
    description: form.value.description,
    parentId: parent.value?.id ?? null,
    iconKey: form.value.iconKey,
    color: form.value.color,
    imageMediaId: image.value?.id ?? null,
  };
  saving.value = true;
  error.value = "";
  try {
    if (editing.value) await call("update_category", { id: editing.value.id, category });
    else await call("create_category", category);
    open.value = false;
    await load();
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center gap-3">
      <label v-if="archivedCount" class="flex items-center gap-2 text-sm text-muted-foreground">
        <input v-model="showArchived" type="checkbox" class="size-4 accent-primary" />
        {{ $t("catalog.showArchived", { n: archivedCount }) }}
      </label>
      <Button v-if="canEdit" size="lg" class="ms-auto" @click="startCreate()">
        <Plus /> {{ $t("catalog.addCategory") }}
      </Button>
    </div>
    <p v-if="listError" role="alert" class="text-sm text-destructive">{{ listError }}</p>

    <p v-if="roots.length === 0" class="rounded-xl border border-dashed p-10 text-center text-muted-foreground">
      {{ $t("catalog.noCategories") }}
    </p>

    <ul class="space-y-3">
      <li v-for="(root, i) in roots" :key="root.id" class="overflow-hidden rounded-xl border bg-card">
        <template v-for="(c, j) in [root, ...childrenOf(root.id)]" :key="c.id">
          <div
            class="flex items-center gap-3 px-4 py-3"
            :class="[j > 0 && 'border-t ps-6', !c.is_active && 'opacity-55']"
          >
            <CornerDownRight v-if="j > 0" :size="16" class="shrink-0 text-muted-foreground rtl:-scale-x-100" />
            <div
              class="grid shrink-0 place-items-center overflow-hidden rounded-lg"
              :class="j === 0 ? 'size-11' : 'size-9'"
              :style="{ background: categoryTint(c.color), color: categoryInk(c.color) }"
            >
              <MediaImage v-if="c.image" :media="c.image" class="size-full" />
              <AppIcon v-else :icon="c.icon_key" :size="j === 0 ? 24 : 20" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="truncate" :class="j === 0 ? 'font-semibold' : 'font-medium'">
                {{ c.name }}
              </div>
              <div class="text-xs text-muted-foreground">
                {{ $t("catalog.productCount", c.product_count) }}
                <template v-if="!c.is_active"> — {{ $t("catalog.archived") }}</template>
              </div>
            </div>

            <div v-if="canEdit" class="flex shrink-0 items-center gap-0.5">
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="$t('catalog.moveUp')"
                :disabled="j === 0 ? i === 0 : j === 1"
                @click="move(c, -1)"
              >
                <ChevronUp />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="$t('catalog.moveDown')"
                :disabled="j === 0 ? i === roots.length - 1 : j === childrenOf(root.id).length"
                @click="move(c, 1)"
              >
                <ChevronDown />
              </Button>
              <Button
                v-if="j === 0 && c.is_active"
                variant="ghost"
                size="sm"
                @click="startCreate(c)"
              >
                <Plus /> {{ $t("catalog.addSubcategory") }}
              </Button>
              <Button variant="ghost" size="icon-sm" :aria-label="$t('settings.edit')" @click="startEdit(c)">
                <Pencil />
              </Button>
              <Button
                v-if="c.product_count === 0 && (j > 0 || childrenOf(c.id).length === 0)"
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground hover:text-destructive"
                :aria-label="$t('settings.delete')"
                @click="remove(c)"
              >
                <Trash2 />
              </Button>
              <Button
                v-else
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground"
                :aria-label="c.is_active ? $t('catalog.archive') : $t('catalog.restore')"
                :title="c.is_active ? $t('catalog.archiveHint') : $t('catalog.restore')"
                @click="setActive(c, !c.is_active)"
              >
                <Archive v-if="c.is_active" />
                <ArchiveRestore v-else />
              </Button>
            </div>
          </div>
        </template>
      </li>
    </ul>

    <EditSheet
      v-model:open="open"
      :title="editing ? $t('catalog.editCategory') : parent ? $t('catalog.addSubcategory') : $t('catalog.addCategory')"
      @submit="save"
    >
      <div class="space-y-2">
        <Label for="c-name">{{ $t("catalog.name") }}</Label>
        <Input id="c-name" v-model="form.name" required dir="auto" />
      </div>

      <div class="space-y-2">
        <Label>{{ $t("catalog.parent") }}</Label>
        <Select v-model="form.parentId" :key="$i18n.locale" :disabled="hasChildren">
          <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="none">{{ $t("catalog.topLevel") }}</SelectItem>
            <SelectItem v-for="p in parentOptions" :key="p.id" :value="String(p.id)">
              {{ p.name }}
            </SelectItem>
          </SelectContent>
        </Select>
        <p v-if="hasChildren" class="text-xs text-muted-foreground">{{ $t("catalog.hasChildrenHint") }}</p>
      </div>

      <div class="space-y-2">
        <Label>{{ $t("catalog.image") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
        <ImageField v-model="image" :icon="ImageIcon" :hint="$t('catalog.imageHint')" />
      </div>

      <div class="space-y-2">
        <Label>{{ $t("catalog.icon") }}</Label>
        <IconPicker v-model="form.iconKey" :suggest-from="suggestText" :boost-from="parent?.icon_key" />
      </div>

      <fieldset class="space-y-2">
        <legend class="text-sm font-medium">{{ $t("catalog.color") }}</legend>
        <div class="flex flex-wrap items-center gap-2">
          <button
            v-if="parent"
            type="button"
            class="h-9 rounded-full border px-3 text-xs aria-pressed:border-foreground aria-pressed:bg-foreground aria-pressed:text-background"
            :aria-pressed="form.color === null"
            @click="form.color = null"
          >
            {{ $t("catalog.sameAsParent") }}
          </button>
          <button
            v-for="c in COLORS"
            :key="c"
            type="button"
            class="size-9 rounded-full ring-offset-2 ring-offset-background aria-pressed:ring-2 aria-pressed:ring-foreground"
            :style="{ background: c }"
            :aria-pressed="form.color === c"
            :aria-label="c"
            @click="form.color = c"
          />
        </div>
        <div
          class="mt-3 flex h-20 w-40 flex-col justify-between rounded-xl p-3"
          :style="{ background: categoryTint(previewColor), color: categoryInk(previewColor) }"
          aria-hidden="true"
        >
          <AppIcon v-if="form.iconKey" :icon="form.iconKey" :size="22" />
          <span class="truncate text-sm font-semibold text-foreground">{{ form.name || "—" }}</span>
        </div>
      </fieldset>

      <div class="space-y-2">
        <Label for="c-desc">
          {{ $t("settings.roles.description") }}
          <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
        </Label>
        <Input id="c-desc" v-model="form.description" />
      </div>

      <template #footer>
        <Button type="submit" :disabled="saving">
          <Loader2 v-if="saving" class="animate-spin" />
          {{ editing ? $t("common.save") : $t("catalog.create") }}
        </Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      </template>
    </EditSheet>
  </div>
</template>
