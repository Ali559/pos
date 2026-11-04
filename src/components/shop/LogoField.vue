<script setup lang="ts">
import { ref } from "vue";
import { ImagePlus, Loader2, Store } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import MediaImage from "@/components/media/MediaImage.vue";
import {
  ACCEPTED_IMAGE_TYPES,
  canUploadMedia,
  uploadMedia,
  type MediaFile,
} from "@/lib/media";

const logo = defineModel<MediaFile | null>({ required: true });

const input = ref<HTMLInputElement | null>(null);
const uploading = ref(false);
const error = ref("");

const onFile = async (e: Event) => {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  uploading.value = true;
  error.value = "";
  try {
    logo.value = await uploadMedia(file);
  } catch (err) {
    error.value = String(err);
  } finally {
    uploading.value = false;
    if (input.value) input.value.value = "";
  }
};
</script>

<template>
  <div class="flex items-center gap-4">
    <div
      class="grid size-20 shrink-0 place-items-center overflow-hidden rounded-xl border bg-muted text-muted-foreground"
    >
      <MediaImage v-if="logo" :media="logo" fit="contain" class="size-full" />
      <Store v-else :size="28" :stroke-width="1.5" />
    </div>
    <div class="min-w-0 space-y-2">
      <div class="flex gap-2">
        <input
          ref="input"
          type="file"
          :accept="ACCEPTED_IMAGE_TYPES"
          class="sr-only"
          tabindex="-1"
          @change="onFile"
        />
        <Button
          type="button"
          variant="outline"
          :disabled="!canUploadMedia || uploading"
          @click="input?.click()"
        >
          <Loader2 v-if="uploading" class="animate-spin" />
          <ImagePlus v-else />
          {{ logo ? $t("common.change") : $t("common.upload") }}
        </Button>
        <Button
          v-if="logo"
          type="button"
          variant="ghost"
          class="text-muted-foreground"
          @click="logo = null"
        >
          {{ $t("common.remove") }}
        </Button>
      </div>
      <p class="text-sm text-muted-foreground">
        {{ canUploadMedia ? $t("shop.logoHint") : $t("common.desktopOnly") }}
      </p>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </div>
  </div>
</template>
