---
aside: false
outline: false
---

<script setup lang="ts">
import { useRoute } from 'vitepress'

const route = useRoute()
const { operationId } = route.data.params
</script>

<OAOperation :operationId="operationId" />
