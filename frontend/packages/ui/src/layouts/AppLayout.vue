<script setup lang="ts">
</script>

<template>
  <div class="flex h-screen w-full overflow-hidden bg-[#0F172A] font-sans">
    <slot name="sidebar"></slot>
    <div class="flex flex-1 flex-col h-full overflow-hidden">
      <slot name="header"></slot>
      <main class="flex-1 p-8 overflow-y-auto">
        <slot></slot>
      </main>
    </div>
  </div>
</template>
