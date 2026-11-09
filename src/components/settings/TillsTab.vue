<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { CheckCircle2, Cpu, Link2, Loader2, MonitorSmartphone, Server, Unplug } from "lucide-vue-next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { select } from "@/db/client";
import { call, inDesktopApp } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatDateTime } from "@/lib/locale";
import PairForm from "./PairForm.vue";

// Settings → Tills: one computer is the main one (it keeps the shop's data);
// other tills on the shop's network pair with it and work through it.
interface LanStatus {
  role: "standalone" | "main" | "till";
  version: string;
  running: boolean;
  port: number;
  addresses: string[];
  pairingCode: string | null;
  pairingSeconds: number | null;
  server: string | null;
  tillName: string | null;
}
interface TillRow {
  id: number;
  name: string;
  paired_at: string;
  last_seen_at: string | null;
  app_version: string | null;
}

const status = ref<LanStatus | null>(null);
const tills = ref<TillRow[]>([]);
const busy = ref(false);
const error = ref("");
const confirmLeave = ref(false);
const secondsLeft = ref(0);
let tick: ReturnType<typeof setInterval> | undefined;

const loadTills = async () => {
  tills.value = await select<TillRow>(
    "SELECT id, name, paired_at, last_seen_at, app_version FROM tills WHERE revoked_at IS NULL ORDER BY paired_at",
  ).catch(() => []);
};
const apply = (s: LanStatus) => {
  status.value = s;
  secondsLeft.value = s.pairingSeconds ?? 0;
};
const load = async () => {
  error.value = "";
  try {
    apply(await call<LanStatus>("lan_status"));
    if (status.value?.role !== "till") await loadTills();
  } catch (e) {
    error.value = errorMessage(e);
  }
};

const run = async (fn: () => Promise<void>) => {
  busy.value = true;
  error.value = "";
  try {
    await fn();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
const setServer = (enabled: boolean) => run(async () => apply(await call<LanStatus>("set_lan_server", { enabled })));
const newCode = () => run(async () => apply(await call<LanStatus>("new_pairing_code")));
const revoke = (id: number) => run(async () => {
  await call("revoke_till", { id });
  await loadTills();
});
const leave = () => run(async () => {
  await call("leave_main");
});

// A till counts as connected if it was heard from in the last two minutes
const online = (t: TillRow) => !!t.last_seen_at && Date.now() - new Date(t.last_seen_at).getTime() < 120_000;
const address = computed(() => (status.value?.addresses[0] ? `${status.value.addresses[0]}:${status.value.port}` : null));
const code = computed(() => (secondsLeft.value > 0 ? status.value?.pairingCode ?? null : null));
const countdown = computed(() => `${Math.floor(secondsLeft.value / 60)}:${String(secondsLeft.value % 60).padStart(2, "0")}`);

onMounted(() => {
  load();
  tick = setInterval(() => {
    if (secondsLeft.value > 0) secondsLeft.value--;
    // While a code is showing, the new till appears as soon as it pairs
    if (code.value && secondsLeft.value % 5 === 0) loadTills();
  }, 1000);
});
onUnmounted(() => clearInterval(tick));
</script>

<template>
  <div class="space-y-5">
    <p v-if="!inDesktopApp" class="rounded-lg bg-muted p-3 text-sm text-muted-foreground">{{ $t("tills.desktopOnly") }}</p>
    <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
    <div v-if="!status" class="grid h-32 place-items-center text-muted-foreground"><Loader2 class="animate-spin" /></div>

    <!-- An extra till -->
    <Card v-else-if="status.role === 'till'">
      <CardHeader>
        <CardTitle class="flex items-center gap-2"><MonitorSmartphone :size="18" /> {{ $t("tills.thisIsTill") }}</CardTitle>
      </CardHeader>
      <CardContent class="space-y-4">
        <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
          <dt class="text-muted-foreground">{{ $t("tills.tillName") }}</dt><dd><bdi>{{ status.tillName }}</bdi></dd>
          <dt class="text-muted-foreground">{{ $t("tills.mainAddress") }}</dt><dd dir="ltr" class="text-start">{{ status.server }}</dd>
          <dt class="text-muted-foreground">{{ $t("tills.version") }}</dt><dd dir="ltr" class="text-start">{{ status.version }}</dd>
        </dl>
        <p class="text-sm text-muted-foreground">{{ $t("tills.tillHint") }}</p>
        <div v-if="!confirmLeave">
          <Button variant="outline" @click="confirmLeave = true"><Unplug /> {{ $t("tills.leave") }}</Button>
        </div>
        <div v-else class="space-y-3 rounded-lg border border-destructive/40 bg-destructive/5 p-3">
          <p class="text-sm">{{ $t("tills.leaveWarning") }}</p>
          <div class="flex gap-2">
            <Button variant="destructive" :disabled="busy" @click="leave">{{ $t("tills.leaveConfirm") }}</Button>
            <Button variant="ghost" @click="confirmLeave = false">{{ $t("common.cancel") }}</Button>
          </div>
        </div>
      </CardContent>
    </Card>

    <template v-else>
      <!-- The main computer -->
      <Card>
        <CardHeader>
          <CardTitle class="flex items-center gap-2"><Server :size="18" /> {{ $t("tills.mainTitle") }}</CardTitle>
          <p class="text-sm text-muted-foreground">{{ $t("tills.mainHint") }}</p>
        </CardHeader>
        <CardContent class="space-y-4">
          <template v-if="status.running">
            <p class="flex items-center gap-2 text-sm font-medium text-success"><CheckCircle2 :size="16" /> {{ $t("tills.serving") }}</p>
            <div v-if="address" class="space-y-1">
              <Label>{{ $t("tills.addressForTills") }}</Label>
              <p class="num w-fit rounded-lg bg-muted px-3 py-2 font-mono text-lg font-semibold" dir="ltr">{{ address }}</p>
            </div>
            <p v-else class="text-sm text-destructive">{{ $t("tills.noNetwork") }}</p>

            <div class="space-y-2 rounded-lg border p-3">
              <div class="flex flex-wrap items-center gap-3">
                <Button :disabled="busy" @click="newCode"><Link2 /> {{ $t("tills.pairNew") }}</Button>
                <template v-if="code">
                  <span class="num font-mono text-3xl font-bold tracking-[0.3em]" dir="ltr">{{ code }}</span>
                  <span class="num text-sm text-muted-foreground">{{ $t("tills.codeExpires", { time: countdown }) }}</span>
                </template>
              </div>
              <p class="text-xs text-muted-foreground">{{ $t("tills.pairSteps") }}</p>
            </div>

            <div class="space-y-2">
              <h3 class="text-sm font-semibold">{{ $t("tills.paired") }}</h3>
              <p v-if="!tills.length" class="text-sm text-muted-foreground">{{ $t("tills.none") }}</p>
              <ul v-else class="divide-y rounded-lg border">
                <li v-for="t in tills" :key="t.id" class="flex flex-wrap items-center gap-3 px-3 py-2 text-sm">
                  <span class="size-2 rounded-full" :class="online(t) ? 'bg-success' : 'bg-muted-foreground/40'" />
                  <bdi class="font-medium">{{ t.name }}</bdi>
                  <span class="text-xs text-muted-foreground">
                    {{ online(t) ? $t("tills.online") : t.last_seen_at ? $t("tills.lastSeen", { time: formatDateTime(new Date(t.last_seen_at)) }) : $t("tills.neverSeen") }}
                  </span>
                  <span v-if="t.app_version && t.app_version !== status.version" class="text-xs font-medium text-destructive">
                    {{ $t("tills.needsUpdate", { version: t.app_version }) }}
                  </span>
                  <Button variant="ghost" size="sm" class="ms-auto text-destructive" :disabled="busy" @click="revoke(t.id)">{{ $t("tills.remove") }}</Button>
                </li>
              </ul>
            </div>
            <Button variant="outline" :disabled="busy" @click="setServer(false)">{{ $t("tills.stop") }}</Button>
          </template>

          <template v-else>
            <div class="rounded-lg bg-muted/60 p-4 text-sm">
              <p class="mb-2 flex items-center gap-2 font-medium"><Cpu :size="16" /> {{ $t("tills.specsTitle") }}</p>
              <ul class="list-disc space-y-1 ps-5 text-muted-foreground">
                <li v-for="i in 6" :key="i">{{ $t(`tills.specs.s${i}`) }}</li>
              </ul>
            </div>
            <Button :disabled="busy || !inDesktopApp" @click="setServer(true)"><Server /> {{ $t("tills.start") }}</Button>
          </template>
        </CardContent>
      </Card>

      <!-- Or make this computer an extra till -->
      <Card v-if="!status.running">
        <CardHeader>
          <CardTitle class="flex items-center gap-2"><MonitorSmartphone :size="18" /> {{ $t("tills.joinTitle") }}</CardTitle>
          <p class="text-sm text-muted-foreground">{{ $t("tills.joinHint") }}</p>
        </CardHeader>
        <CardContent>
          <PairForm warn />
        </CardContent>
      </Card>
    </template>
  </div>
</template>
