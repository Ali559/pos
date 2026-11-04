<script setup lang="ts">
import { ref, onMounted, watch, onUnmounted } from "vue";
import {
  Chart,
  LineController,
  LineElement,
  PointElement,
  LinearScale,
  CategoryScale,
  BarController,
  BarElement,
  Title,
  Tooltip,
  Legend,
  Filler,
  type ChartConfiguration,
  type ChartType,
  type ChartData,
  type ChartOptions,
} from "chart.js";

Chart.register(
  LineController,
  LineElement,
  PointElement,
  BarController,
  BarElement,
  LinearScale,
  CategoryScale,
  Title,
  Tooltip,
  Legend,
  Filler,
);

const props = defineProps<{
  type: ChartType;
  data: ChartData;
  options?: ChartOptions;
  responsive?: boolean;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
let chartInstance: Chart | null = null;

const initChart = () => {
  if (!canvasRef.value) return;

  if (chartInstance) {
    chartInstance.destroy();
  }

  const config: ChartConfiguration = {
    type: props.type,
    data: props.data,
    options: {
      responsive: props.responsive ?? true,
      maintainAspectRatio: false,
      ...props.options,
    },
  };

  chartInstance = new Chart(canvasRef.value, config);
};

const updateChart = () => {
  if (chartInstance) {
    chartInstance.data = props.data;
    chartInstance.update();
  }
};

onMounted(() => {
  initChart();
});

onUnmounted(() => {
  if (chartInstance) {
    chartInstance.destroy();
  }
});

watch(() => props.data, updateChart, { deep: true });
</script>

<template>
  <canvas ref="canvasRef" class="w-full h-full"></canvas>
</template>
