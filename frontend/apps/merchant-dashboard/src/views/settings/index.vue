<template>
  <div class="space-y-6">
    <!-- Personal Info -->
    <Card class="animate-fade-in-up">
      <CardHeader>
        <div class="flex items-center gap-2">
          <div class="stat-icon-box h-8 w-8 flex items-center justify-center rounded-md">
            <User class="h-4 w-4 text-brand" />
          </div>
          <CardTitle>{{ t('settings.personalInfo') }}</CardTitle>
        </div>
      </CardHeader>
      <CardContent class="space-y-4">
        <!-- Avatar -->
        <div class="flex items-center gap-4">
          <div
            class="h-14 w-14 rounded-full flex items-center justify-center text-lg font-semibold text-white"
            :style="{ backgroundColor: avatarColor }"
          >
            {{ avatarInitials }}
          </div>
          <div>
            <p class="text-sm font-medium">{{ userStore.name || '—' }}</p>
            <p class="text-xs text-muted-foreground">{{ userStore.email || '—' }}</p>
          </div>
        </div>

        <Separator />

        <!-- Name field -->
        <div class="flex flex-wrap items-center gap-3">
          <Label class="w-[100px] text-sm text-muted-foreground shrink-0">{{ t('settings.name') }}</Label>
          <template v-if="editingName">
            <Input
              v-model="nameInput"
              class="max-w-xs"
              @keyup.enter="saveName"
              @keyup.escape="editingName = false"
            />
            <Button size="sm" @click="saveName" :disabled="savingName">
              {{ savingName ? t('settings.saving') : t('settings.save') }}
            </Button>
            <Button size="sm" variant="ghost" @click="editingName = false">
              {{ t('settings.cancel') }}
            </Button>
          </template>
          <template v-else>
            <span class="text-sm">{{ userStore.name || '—' }}</span>
            <Button size="sm" variant="ghost" @click="startEditName">
              <Pencil class="h-3.5 w-3.5" />
            </Button>
          </template>
        </div>

        <!-- Email field (read-only) -->
        <div class="flex items-center gap-3">
          <Label class="w-[100px] text-sm text-muted-foreground shrink-0">{{ t('settings.email') }}</Label>
          <span class="text-sm">{{ userStore.email || '—' }}</span>
        </div>

        <!-- Role field -->
        <div class="flex items-center gap-3">
          <Label class="w-[100px] text-sm text-muted-foreground shrink-0">{{ t('settings.role') }}</Label>
          <Badge :variant="roleBadgeVariant">{{ roleLabel }}</Badge>
        </div>

        <Separator />

        <div class="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" @click="$router.push({ name: 'TwoFactorAuth' })">
            {{ t('settings.manage2fa') }}
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- Change Password -->
    <Card class="animate-fade-in-up" style="animation-delay: 80ms">
      <CardHeader>
        <div class="flex items-center gap-2">
          <div class="stat-icon-box h-8 w-8 flex items-center justify-center rounded-md">
            <Lock class="h-4 w-4 text-brand" />
          </div>
          <CardTitle>{{ t('settings.changePassword') }}</CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <form class="space-y-4 max-w-sm" @submit.prevent="changePassword">
          <div class="space-y-1.5">
            <Label>{{ t('settings.currentPassword') }}</Label>
            <Input v-model="pwForm.oldPassword" type="password" autocomplete="current-password" />
          </div>
          <div class="space-y-1.5">
            <Label>{{ t('settings.newPassword') }}</Label>
            <Input v-model="pwForm.newPassword" type="password" autocomplete="new-password" />
          </div>
          <div class="space-y-1.5">
            <Label>{{ t('settings.confirmPassword') }}</Label>
            <Input v-model="pwForm.confirmPassword" type="password" autocomplete="new-password" />
          </div>
          <Button type="submit" :disabled="savingPassword">
            {{ savingPassword ? t('settings.saving') : t('settings.changePassword') }}
          </Button>
        </form>
      </CardContent>
    </Card>
  </div>
</template>

<script lang="ts" setup>
import { ref, reactive, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { toast } from 'vue-sonner';
import { User, Lock, Pencil } from 'lucide-vue-next';
import { useUserStore } from '@/stores';
import { http } from '@/utils/request';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';

const { t } = useI18n();
const userStore = useUserStore();

// ── Avatar ──
const avatarInitials = computed(() => {
  const name = userStore.name || userStore.email || '?';
  return name.charAt(0).toUpperCase();
});

const avatarColor = computed(() => {
  const colors = ['#3B82F6', '#8B5CF6', '#EC4899', '#14B8A6', '#F59E0B', '#EF4444'];
  const str = userStore.email || userStore.name || '';
  let hash = 0;
  for (let i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash);
  return colors[Math.abs(hash) % colors.length];
});

// ── Role ──
const roleLabel = computed(() => {
  const role = userStore.orgRole || 'owner';
  const map: Record<string, string> = {
    owner: t('team.role.owner'),
    admin: t('team.role.admin'),
    developer: t('team.role.developer'),
    finance: t('team.role.finance'),
    viewer: t('team.role.viewer'),
  };
  return map[role] || role;
});

const roleBadgeVariant = computed(() => {
  const role = userStore.orgRole || 'owner';
  const map: Record<string, string> = {
    owner: 'default',
    admin: 'secondary',
    developer: 'outline',
    finance: 'outline',
    viewer: 'outline',
  };
  return (map[role] || 'outline') as any;
});

// ── Edit Name ──
const editingName = ref(false);
const nameInput = ref('');
const savingName = ref(false);

function startEditName() {
  nameInput.value = userStore.name || '';
  editingName.value = true;
}

async function saveName() {
  if (!nameInput.value.trim()) return;
  savingName.value = true;
  try {
    await http.put('/api/internal/merchants/user/me', { name: nameInput.value.trim() });
    userStore.setInfo({ name: nameInput.value.trim() });
    editingName.value = false;
    toast.success(t('settings.nameUpdated'));
  } catch {
    // interceptor shows backend error
  } finally {
    savingName.value = false;
  }
}

// ── Change Password ──
const pwForm = reactive({ oldPassword: '', newPassword: '', confirmPassword: '' });
const savingPassword = ref(false);

async function changePassword() {
  if (pwForm.newPassword.length < 8) {
    toast.error(t('settings.passwordTooShort'));
    return;
  }
  if (pwForm.newPassword !== pwForm.confirmPassword) {
    toast.error(t('settings.passwordMismatch'));
    return;
  }
  savingPassword.value = true;
  try {
    await http.put('/api/internal/merchants/password', {
      old_password: pwForm.oldPassword,
      new_password: pwForm.newPassword,
    });
    toast.success(t('settings.passwordChanged'));
    pwForm.oldPassword = '';
    pwForm.newPassword = '';
    pwForm.confirmPassword = '';
  } catch {
    // interceptor shows backend error
  } finally {
    savingPassword.value = false;
  }
}
</script>
