<!-- PointOfSale.vue -->
<template>
  <div class="p-6">
    <div class="grid lg:grid-cols-3 gap-6 max-w-7xl mx-auto">
      <!-- Left Column -->
      <div class="lg:col-span-2 space-y-6">
        <!-- Barcode Scanner Card -->
        <Card>
          <CardHeader>
            <CardTitle class="text-start">
              {{ $t("pos.scanBarcode") }}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div class="space-y-4">
              <div class="relative">
                <Input
                  v-model="barcode"
                  type="text"
                  :placeholder="$t('pos.barcodePlaceholder')"
                  class="w-full text-lg ps-12 text-start font-mono"
                  @keydown.enter="addItemByBarcode"
                />
                <div class="absolute start-3 top-1/2 -translate-y-1/2">
                  <svg
                    class="w-5 h-5 text-muted-foreground"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
                    />
                  </svg>
                </div>
              </div>
              <p class="text-sm text-muted-foreground text-start">
                💡 {{ $t("pos.scanHint") }}
              </p>
            </div>
          </CardContent>
        </Card>

        <!-- Cart Items Card -->
        <Card>
          <CardHeader>
            <CardTitle class="text-start">
              {{ $t("pos.cartItems") }}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead class="text-start w-75">
                    {{ $t("pos.product") }}
                  </TableHead>
                  <TableHead class="text-end">
                    {{ $t("pos.price") }}
                  </TableHead>
                  <TableHead class="text-center">
                    {{ $t("pos.quantity") }}
                  </TableHead>
                  <TableHead class="text-end">
                    {{ $t("pos.total") }}
                  </TableHead>
                  <TableHead class="text-center">
                    {{ $t("pos.actions") }}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-if="cartItems.length === 0">
                  <TableCell
                    colspan="5"
                    class="text-center text-muted-foreground py-12"
                  >
                    {{ $t("pos.emptyCart") }}
                  </TableCell>
                </TableRow>
                <TableRow v-for="item in cartItems" :key="item.id">
                  <TableCell class="text-start font-medium">
                    {{ item.name }}
                  </TableCell>
                  <TableCell class="text-end">
                    {{ formatCurrency(item.price) }}
                  </TableCell>
                  <TableCell class="text-center">
                    <div class="flex items-center justify-center gap-2">
                      <Button
                        @click="decrementQuantity(item.id)"
                        variant="outline"
                        size="icon"
                        class="h-8 w-8"
                      >
                        <Minus class="h-4 w-4" />
                      </Button>
                      <span class="w-8 text-center">{{ item.quantity }}</span>
                      <Button
                        @click="incrementQuantity(item.id)"
                        variant="outline"
                        size="icon"
                        class="h-8 w-8"
                      >
                        <Plus class="h-4 w-4" />
                      </Button>
                    </div>
                  </TableCell>
                  <TableCell class="text-end font-medium">
                    {{ formatCurrency(item.price * item.quantity) }}
                  </TableCell>
                  <TableCell class="text-center">
                    <Button
                      @click="removeFromCart(item.id)"
                      variant="ghost"
                      size="icon"
                      class="text-destructive hover:text-destructive/90"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>

      <!-- Right Column -->
      <div class="space-y-6">
        <!-- Order Summary Card -->
        <Card>
          <CardHeader>
            <CardTitle class="text-start">
              {{ $t("pos.orderSummary") }}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div class="space-y-3">
              <div class="flex justify-between text-sm">
                <span class="text-muted-foreground">{{
                  $t("pos.subtotal")
                }}</span>
                <span>{{ formatCurrency(subtotal) }}</span>
              </div>
              <div class="flex justify-between text-sm">
                <span class="text-muted-foreground"
                  >{{ $t("pos.tax") }} (10%)</span
                >
                <span>{{ formatCurrency(tax) }}</span>
              </div>
              <Separator />
              <div class="flex justify-between items-baseline">
                <span class="font-medium">{{ $t("pos.total") }}</span>
                <span class="text-2xl font-bold text-primary">
                  {{ formatCurrency(total) }}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>

        <!-- Action Buttons -->
        <Button
          @click="checkout"
          :disabled="cartItems.length === 0"
          class="w-full"
          size="lg"
        >
          {{ $t("pos.charge") }} {{ formatCurrency(total) }}
        </Button>

        <Button
          @click="clearCart"
          variant="outline"
          class="w-full"
          :disabled="cartItems.length === 0"
        >
          {{ $t("pos.clearCart") }}
        </Button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Separator } from "@/components/ui/separator";
import { Plus, Minus, Trash2 } from "lucide-vue-next";

// Types
interface CartItem {
  id: string;
  name: string;
  price: number;
  quantity: number;
}

// State
const barcode = ref("");
const cartItems = ref<CartItem[]>([
  { id: "1", name: "Coca Cola", price: 2.5, quantity: 2 },
  { id: "2", name: "Potato Chips", price: 3.0, quantity: 1 },
]);

// Computed
const subtotal = computed(() => {
  return cartItems.value.reduce(
    (sum, item) => sum + item.price * item.quantity,
    0,
  );
});

const tax = computed(() => subtotal.value * 0.1);
const total = computed(() => subtotal.value + tax.value);

// Methods
const formatCurrency = (amount: number) => {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
  }).format(amount);
};

const addItemByBarcode = () => {
  if (!barcode.value.trim()) return;

  // Simulate adding item
  const newItem: CartItem = {
    id: Date.now().toString(),
    name: `Product ${barcode.value}`,
    price: 5.99,
    quantity: 1,
  };

  cartItems.value.push(newItem);
  barcode.value = "";
};

const incrementQuantity = (id: string) => {
  const item = cartItems.value.find((item) => item.id === id);
  if (item) item.quantity++;
};

const decrementQuantity = (id: string) => {
  const item = cartItems.value.find((item) => item.id === id);
  if (item && item.quantity > 1) {
    item.quantity--;
  } else {
    removeFromCart(id);
  }
};

const removeFromCart = (id: string) => {
  cartItems.value = cartItems.value.filter((item) => item.id !== id);
};

const clearCart = () => {
  cartItems.value = [];
};

const checkout = () => {
  // Implement checkout logic
  console.log("Checking out:", cartItems.value);
  clearCart();
};
</script>
