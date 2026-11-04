<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { decode } from "blurhash";
import { mediaUrl, type MediaFile } from "@/lib/media";

// Paints the blurhash immediately, then fades the real image in on load
const props = defineProps<{
  media: MediaFile;
  alt?: string;
  fit?: "cover" | "contain";
}>();

const canvas = ref<HTMLCanvasElement | null>(null);
const src = ref<string | null>(null);
const loaded = ref(false);

const paintBlurhash = () => {
  const el = canvas.value;
  if (!el) return;
  const pixels = decode(props.media.blurhash, 32, 32);
  const ctx = el.getContext("2d");
  if (!ctx) return;
  const data = ctx.createImageData(32, 32);
  data.data.set(pixels);
  ctx.putImageData(data, 0, 0);
};

const load = async () => {
  loaded.value = false;
  paintBlurhash();
  src.value = await mediaUrl(props.media);
};

onMounted(load);
watch(() => props.media.id, load);
</script>

<template>
  <div class="relative overflow-hidden">
    <canvas
      ref="canvas"
      width="32"
      height="32"
      v-show="!loaded"
      aria-hidden="true"
      class="absolute inset-0 size-full"
    />
    <img
      v-if="src"
      :src="src"
      :alt="alt ?? ''"
      :width="media.width"
      :height="media.height"
      class="relative size-full transition-opacity duration-300"
      :class="[
        fit === 'contain' ? 'object-contain' : 'object-cover',
        loaded ? 'opacity-100' : 'opacity-0',
      ]"
      @load="loaded = true"
    />
  </div>
</template>
