<script setup lang="ts">
import { ref } from "vue";
import { ChevronLeft, ChevronRight, ImagePlus, Loader2, Star, X } from "lucide-vue-next";
import MediaImage from "./MediaImage.vue";
import { ACCEPTED_IMAGE_TYPES, canUploadMedia, uploadMedia, type MediaFile } from "@/lib/media";
import { errorMessage } from "@/lib/errors";

// Ordered images; the first is the cover. Uploads go through the Rust
// pipeline (WebP q80 + blurhash) one file at a time.
const images = defineModel<MediaFile[]>({ required: true });
/** True while uploads are in flight, so the parent can hold its Save */
const busy = defineModel<boolean>("busy", { default: false });

const input = ref<HTMLInputElement | null>(null);
const uploading = ref(0);
const error = ref("");

const onFiles = async (e: Event) => {
  const files = [...((e.target as HTMLInputElement).files ?? [])];
  error.value = "";
  uploading.value = files.length;
  busy.value = files.length > 0;
  for (const file of files) {
    try {
      const media = await uploadMedia(file);
      if (!images.value.some((m) => m.id === media.id)) images.value = [...images.value, media];
    } catch (err) {
      error.value = `${file.name}: ${errorMessage(err)}`;
    }
    uploading.value--;
  }
  busy.value = false;
  if (input.value) input.value.value = "";
};

const move = (i: number, by: number) => {
  const next = [...images.value];
  const j = i + by;
  if (j < 0 || j >= next.length) return;
  [next[i], next[j]] = [next[j], next[i]];
  images.value = next;
};
const makeCover = (i: number) => {
  const next = [...images.value];
  next.unshift(...next.splice(i, 1));
  images.value = next;
};
const remove = (i: number) => {
  images.value = images.value.filter((_, k) => k !== i);
};
</script>

<template>
  <div class="space-y-2">
    <ul class="flex flex-wrap gap-2">
      <li
        v-for="(m, i) in images"
        :key="m.id"
        class="group relative size-24 overflow-hidden rounded-lg border bg-muted"
      >
        <MediaImage :media="m" class="size-full" />
        <span
          v-if="i === 0"
          class="absolute start-1 top-1 rounded bg-sun px-1.5 py-0.5 text-[10px] font-semibold text-sun-foreground"
        >
          {{ $t("media.cover") }}
        </span>
        <div
          class="absolute inset-x-0 bottom-0 flex justify-between bg-black/55 p-1 text-white opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100"
        >
          <button type="button" class="rounded p-0.5 hover:bg-white/20" :aria-label="$t('media.earlier')" @click="move(i, -1)">
            <ChevronLeft :size="16" class="rtl:rotate-180" />
          </button>
          <button
            v-if="i > 0"
            type="button"
            class="rounded p-0.5 hover:bg-white/20"
            :aria-label="$t('media.makeCover')"
            :title="$t('media.makeCover')"
            @click="makeCover(i)"
          >
            <Star :size="16" />
          </button>
          <button type="button" class="rounded p-0.5 hover:bg-white/20" :aria-label="$t('common.remove')" @click="remove(i)">
            <X :size="16" />
          </button>
          <button type="button" class="rounded p-0.5 hover:bg-white/20" :aria-label="$t('media.later')" @click="move(i, 1)">
            <ChevronRight :size="16" class="rtl:rotate-180" />
          </button>
        </div>
      </li>
      <li>
        <input
          ref="input"
          type="file"
          multiple
          :accept="ACCEPTED_IMAGE_TYPES"
          class="sr-only"
          tabindex="-1"
          @change="onFiles"
        />
        <button
          type="button"
          class="grid size-24 place-items-center rounded-lg border border-dashed text-muted-foreground hover:bg-accent hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!canUploadMedia || uploading > 0"
          :aria-label="$t('media.add')"
          @click="input?.click()"
        >
          <Loader2 v-if="uploading" class="animate-spin" />
          <span v-else class="flex flex-col items-center gap-1 text-xs">
            <ImagePlus :size="20" /> {{ $t("media.add") }}
          </span>
        </button>
      </li>
    </ul>
    <p class="text-xs text-muted-foreground">
      {{ canUploadMedia ? $t("media.hint") : $t("common.desktopOnly") }}
    </p>
    <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
  </div>
</template>
