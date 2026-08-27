<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  title: string
  value: string
  trend?: string
  trendUp?: boolean
  alert?: boolean
  showChart?: boolean // Support for sparkline
}>()

const cardClass = computed(() => {
  if (props.alert) {
    return 'border border-red-500/50 bg-[#1E293B] relative overflow-hidden shadow-[0_0_15px_rgba(239,68,68,0.1)]'
  }
  return 'bg-[#1E293B] border border-gray-800 shadow-lg'
})
</script>

<template>
  <div :class="[cardClass, 'rounded-2xl p-6 transition-transform hover:scale-[1.01] duration-300']">
    <div v-if="alert" class="mb-3 inline-flex items-center rounded-md bg-red-500/10 px-2.5 py-1 text-xs font-bold text-red-500 border border-red-500/20">
      <span class="mr-2 h-1.5 w-1.5 rounded-full bg-red-500 animate-pulse"></span>
      Critical Alert
    </div>

    <div class="flex items-start justify-between relative z-10">
      <div>
        <h3 class="text-sm font-medium text-gray-400 tracking-wide">{{ title }}</h3>
        <div class="mt-2 text-4xl font-extrabold text-white tracking-tight">{{ value }}</div>
      </div>
      <div v-if="$slots.icon" class="rounded-xl bg-[#0F172A] p-3 border border-gray-700/50 shadow-inner">
        <slot name="icon"></slot>
      </div>
    </div>

    <!-- Sparkline Chart for Sales Volume -->
    <div v-if="showChart" class="mt-4 h-10 w-full">
         <svg viewBox="0 0 100 20" class="w-full h-full text-blue-500 overflow-visible" preserveAspectRatio="none">
             <path d="M0 15 C 20 15, 30 5, 50 10 S 80 18, 100 2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
             <defs>
                <linearGradient id="gradient" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stop-color="currentColor" stop-opacity="0.2"/>
                <stop offset="100%" stop-color="currentColor" stop-opacity="0"/>
                </linearGradient>
            </defs>
            <path d="M0 15 C 20 15, 30 5, 50 10 S 80 18, 100 2 V 25 H 0 Z" fill="url(#gradient)" style="color: #3b82f6" opacity="0.5" />
         </svg>
    </div>

    <div v-if="trend && !alert" class="mt-4 flex items-center text-sm font-medium">
      <span
        :class="[trendUp ? 'text-green-400 bg-green-400/10' : 'text-red-400 bg-red-400/10', 'px-1.5 py-0.5 rounded flex items-center']"
      >
        {{ trendUp ? '↑' : '↓' }} {{ trend }}
      </span>
      <span class="ml-2 text-gray-500">vs last month</span>
    </div>

    <div v-if="alert" class="mt-6 flex items-center text-sm font-semibold text-red-400 bg-red-500/5 p-2 rounded-lg border border-red-500/10">
      <span class="h-2 w-2 rounded-full bg-red-500 mr-2 shadow-[0_0_8px_rgba(239,68,68,0.6)]"></span>
      Low Balance - Action Required
    </div>

     <div v-if="!alert && !trend" class="mt-4">
        <slot name="footer"></slot>
    </div>
  </div>
</template>
