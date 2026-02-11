<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";

const router = useRouter();
const username = ref("");
const password = ref("");

const handleLogin = (e: Event) => {
  e.preventDefault();
  // Add your login logic here
  router.push("/main/point-of-sale");
};
</script>
<template>
  <div class="min-h-screen flex items-center justify-center p-6 bg-background">
    <Card class="w-full max-w-lg">
      <CardHeader class="space-y-1">
        <div class="flex justify-center mb-4">
          <div class="text-4xl">🏪</div>
        </div>
        <CardTitle class="text-2xl font-bold text-center text-foreground">
          {{ $t("login.title") }}
        </CardTitle>
        <CardDescription class="text-center text-muted-foreground">
          {{ $t("login.subtitle") }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form @submit="handleLogin">
          <div class="space-y-4">
            <div class="space-y-2">
              <Label for="username" class="text-start">
                {{ $t("login.username") }}
              </Label>
              <Input
                id="username"
                v-model="username"
                type="text"
                required
                :placeholder="$t('login.usernamePlaceholder')"
                class="w-full text-start"
              />
            </div>
            <div class="space-y-2">
              <Label for="password" class="text-start">
                {{ $t("login.password") }}
              </Label>
              <Input
                id="password"
                v-model="password"
                type="password"
                required
                :placeholder="$t('login.passwordPlaceholder')"
                class="w-full text-start"
              />
            </div>
            <Button type="submit" class="w-full">
              {{ $t("login.submit") }}
            </Button>
          </div>
        </form>
      </CardContent>
      <CardFooter class="flex justify-center">
        <div class="text-sm text-muted-foreground">
          {{ $t("login.footer") }}
        </div>
      </CardFooter>
    </Card>
  </div>
</template>
