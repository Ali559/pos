<script setup lang="ts">
import { ChevronLeft, ChevronRight } from "lucide-vue-next";
</script>

<template>
  <div class="p-6">
    <div class="max-w-7xl mx-auto">
      <div class="mb-6 flex justify-between items-center">
        <div>
          <h2 class="text-2xl font-bold text-foreground mb-1">
            {{ $t("inventory.title") }}
          </h2>
          <p class="text-muted-foreground">{{ $t("inventory.subtitle") }}</p>
        </div>
        <button
          onclick="openModal('product')"
          class="bg-primary hover:bg-primary/90 text-primary-foreground py-2 px-6 rounded-lg shadow-sm transition-colors"
        >
          + {{ $t("inventory.addProduct") }}
        </button>
      </div>
      <div
        class="bg-card rounded-xl border border-border overflow-hidden shadow-sm"
      >
        <div class="p-6 border-b border-border flex gap-4">
          <input
            type="text"
            id="searchInv"
            :placeholder="$t('inventory.searchPlaceholder')"
            class="flex-1 px-4 py-2 bg-background border border-input rounded-lg text-foreground placeholder:text-muted-foreground"
          />
          <select
            class="px-4 py-2 bg-background border border-input rounded-lg text-foreground"
          >
            <option>{{ $t("inventory.allCategories") }}</option>
            <option>Beverages</option>
            <option>Snacks</option>
            <option>Dairy</option>
          </select>
        </div>
        <table class="w-full">
          <thead class="bg-muted border-b border-border">
            <tr>
              <th
                class="px-6 py-3 text-left text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.sku") }}
              </th>
              <th
                class="px-6 py-3 text-left text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.name") }}
              </th>
              <th
                class="px-6 py-3 text-left text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.category") }}
              </th>
              <th
                class="px-6 py-3 text-left text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.barcode") }}
              </th>
              <th
                class="px-6 py-3 text-right text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.price") }}
              </th>
              <th
                class="px-6 py-3 text-right text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.stock") }}
              </th>
              <th
                class="px-6 py-3 text-center text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.status") }}
              </th>
              <th
                class="px-6 py-3 text-center text-xs font-medium uppercase text-muted-foreground"
              >
                {{ $t("inventory.table.actions") }}
              </th>
            </tr>
          </thead>
          <tbody id="invBody" class="divide-y divide-border"></tbody>
        </table>
        <div class="px-6 py-4 border-t border-border flex justify-between">
          <div class="text-sm text-muted-foreground">
            {{ $t("inventory.pagination.showing") }}
            <span id="showing">1-10</span> {{ $t("inventory.pagination.of") }}

            <span id="totalProd">50</span>
          </div>
          <div class="flex gap-2 rtl:flex-row-reverse">
            <button
              class="px-3 py-1 flex items-center gap-2 bg-background border border-input rounded hover:bg-accent text-foreground transition-colors rtl:flex-row-reverse"
            >
              {{ $t("inventory.pagination.prev") }}
              <ChevronLeft />
            </button>
            <button
              class="px-3 flex items-center gap-2 py-1 bg-background border border-input rounded hover:bg-accent text-foreground transition-colors rtl:flex-row-reverse"
            >
              {{ $t("inventory.pagination.next") }}
              <ChevronRight />
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
