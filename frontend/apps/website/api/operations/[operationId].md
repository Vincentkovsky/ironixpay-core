---
aside: false
outline: false
---

<script setup lang="ts">
import { useRoute } from 'vitepress'

const route = useRoute()
const { operationId, pageTitle } = route.data.params
</script>

<div class="api-zh-title">{{ pageTitle }}</div>

<OAOperation :operationId="operationId" />
