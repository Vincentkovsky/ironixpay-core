<script setup lang="ts">
import { RouterView } from 'vue-router'
import { Toaster } from '@/components/ui/sonner'
import ReloadPrompt from '@/components/ReloadPrompt.vue'
</script>

<template>
  <RouterView />
  <Toaster rich-colors />
  <ReloadPrompt />
</template>
