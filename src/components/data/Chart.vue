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

// Vue casts an absent boolean prop to false, so the default must be explicit
const props = withDefaults(
  defineProps<{
    type: ChartType;
    data: ChartData;
    options?: ChartOptions;
    responsive?: boolean;
  }>(),
  { responsive: true },
);

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
      responsive: props.responsive,
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
  <canvas ref="canvasRef"></canvas>
</template>
