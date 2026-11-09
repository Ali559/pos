<script setup lang="ts">
import { Download, FileSpreadsheet, FileText, Table2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { inDesktopApp } from "@/lib/ipc";
import type { ExportFormat } from "@/lib/exportTable";

// One Export button per report: CSV, Excel or PDF.
defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ export: [format: ExportFormat] }>();
</script>

<template>
  <!-- A wrapping element so a class from the parent (ms-auto) applies -->
  <div>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button variant="ghost" size="sm" :disabled="disabled"><Download /> {{ $t("exports.button") }}</Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">
      <DropdownMenuItem @select="emit('export', 'xlsx')" :disabled="!inDesktopApp"><FileSpreadsheet /> {{ $t("exports.xlsx") }}</DropdownMenuItem>
      <DropdownMenuItem @select="emit('export', 'pdf')"><FileText /> {{ $t("exports.pdf") }}</DropdownMenuItem>
      <DropdownMenuItem @select="emit('export', 'csv')"><Table2 /> {{ $t("exports.csv") }}</DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
  </div>
</template>
