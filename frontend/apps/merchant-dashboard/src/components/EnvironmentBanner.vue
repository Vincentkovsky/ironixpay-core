<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Info, ArrowRight } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { useEnvironmentStore } from '@/stores'

const { t } = useI18n()
const envStore = useEnvironmentStore()

const switchEnv = () => {
  envStore.toggleEnvironment(envStore.isSandbox ? 'live' : 'sandbox')
}
</script>

<template>
  <!-- Sandbox banner -->
  <div
    v-if="envStore.isSandbox"
    class="env-banner env-banner--sandbox"
  >
    <div class="flex items-center gap-3">
      <Badge class="env-banner__badge env-banner__badge--sandbox">
        <Info class="h-3 w-3 mr-1" />
        {{ t('env.sandbox') }}
      </Badge>
      <span class="hidden sm:inline text-sm">
        {{ t('env.sandboxHint') }}
      </span>
    </div>
    <Button
      size="sm"
      class="env-banner__switch env-banner__switch--to-live"
      @click="switchEnv"
    >
      {{ t('env.switchToLive') }}
      <ArrowRight class="h-3.5 w-3.5 ml-1.5" />
    </Button>
  </div>

  <!-- Live banner (minimal) -->
  <div
    v-else
    class="env-banner env-banner--live"
  >
    <div class="flex items-center gap-3">
      <Badge class="env-banner__badge env-banner__badge--live">
        <span class="relative flex h-2 w-2 mr-1.5">
          <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75" />
          <span class="relative inline-flex h-2 w-2 rounded-full bg-emerald-500" />
        </span>
        {{ t('env.live') }}
      </Badge>
      <span class="hidden sm:inline text-sm">
        {{ t('env.liveHint') }}
      </span>
    </div>
    <Button
      size="sm"
      variant="ghost"
      class="env-banner__switch env-banner__switch--to-sandbox"
      @click="switchEnv"
    >
      {{ t('env.switchToSandbox') }}
    </Button>
  </div>
</template>
