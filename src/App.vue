<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import RTLProvider from "./components/providers/RTLProvider.vue";
import { initFlowbite } from "flowbite";
const greetMsg = ref("");
const name = ref("");

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  try {
    greetMsg.value = await invoke("greet", { name: name.value });
    console.log(await invoke("hello_world"));
  } catch (err) {
    console.error(err);
  }
}
onMounted(() => {
  initFlowbite();
});
</script>

<template>
  <r-t-l-provider>
    <main>
      <router-view />
    </main>
  </r-t-l-provider>
</template>
