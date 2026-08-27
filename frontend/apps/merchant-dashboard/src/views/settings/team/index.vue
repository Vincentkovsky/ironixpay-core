<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between animate-fade-in-up">
      <div>
        <h1 class="text-xl font-bold tracking-tight">{{ t('team.title') }}</h1>
        <p class="text-sm text-muted-foreground mt-0.5">{{ t('team.subtitle') }}</p>
      </div>
      <Button
        v-if="!isSandbox && (userStore.orgRole === 'owner' || userStore.orgRole === 'admin')"
        id="invite-member-btn"
        @click="showInviteDialog = true"
      >
        <UserPlus class="h-4 w-4 mr-2" />
        {{ t('team.invite') }}
      </Button>
    </div>

    <!-- Organization Info -->
    <Card class="animate-fade-in-up" style="animation-delay: 40ms">
      <CardHeader>
        <div class="flex items-center gap-2">
          <div class="stat-icon-box h-8 w-8 flex items-center justify-center rounded-md">
            <Building2 class="h-4 w-4 text-brand" />
          </div>
          <CardTitle>{{ t('team.orgInfo') }}</CardTitle>
        </div>
      </CardHeader>
      <CardContent class="space-y-4">
        <!-- Org Name -->
        <div class="flex flex-wrap items-center gap-3">
          <Label class="w-[100px] text-sm text-muted-foreground shrink-0">{{ t('team.orgName') }}</Label>
          <template v-if="editingOrgName">
            <Input
              v-model="orgNameInput"
              class="max-w-xs"
              @keyup.enter="saveOrgName"
              @keyup.escape="editingOrgName = false"
            />
            <Button size="sm" @click="saveOrgName" :disabled="savingOrgName">
              {{ savingOrgName ? t('settings.saving') : t('settings.save') }}
            </Button>
            <Button size="sm" variant="ghost" @click="editingOrgName = false">
              {{ t('settings.cancel') }}
            </Button>
          </template>
          <template v-else>
            <span class="text-sm font-medium">{{ userStore.orgName || '—' }}</span>
            <Button
              v-if="userStore.orgRole === 'owner' || userStore.orgRole === 'admin'"
              size="sm"
              variant="ghost"
              @click="startEditOrgName"
            >
              <Pencil class="h-3.5 w-3.5" />
            </Button>
          </template>
        </div>

        <!-- Merchant ID -->
        <div class="flex items-center gap-3">
          <Label class="w-[100px] text-sm text-muted-foreground shrink-0">{{ t('team.merchantId') }}</Label>
          <code class="text-xs bg-muted px-2 py-1 rounded font-mono select-all">{{ userStore.orgId || '—' }}</code>
        </div>

        <p class="text-xs text-muted-foreground">{{ t('team.orgNameHint') }}</p>

        <!-- Checkout Customization (Logo) -->
        <template v-if="userStore.orgRole === 'owner' || userStore.orgRole === 'admin'">
          <Separator />
          <div class="space-y-4">
            <div class="flex items-center gap-2">
              <Palette class="h-4 w-4 text-brand" />
              <span class="text-sm font-semibold">{{ t('branding.title') }}</span>
            </div>
            <p class="text-xs text-muted-foreground -mt-2">{{ t('branding.description') }}</p>

            <!-- Logo upload area -->
            <div class="flex items-start gap-5">
              <div
                class="h-16 w-16 rounded-xl border-2 border-dashed flex items-center justify-center overflow-hidden shrink-0 transition-colors"
                :class="logoUrl ? 'border-brand/30 bg-brand/5' : 'border-muted-foreground/20 bg-muted/50'"
              >
                <img
                  v-if="logoPreviewUrl || logoUrl"
                  :src="logoPreviewUrl || logoUrl!"
                  alt="Logo"
                  class="h-full w-full object-contain p-1"
                />
                <ImageIcon v-else class="h-6 w-6 text-muted-foreground/40" />
              </div>

              <div class="space-y-2 flex-1">
                <div>
                  <p class="text-sm font-medium">{{ logoUrl ? t('branding.currentLogo') : t('branding.noLogo') }}</p>
                  <p class="text-xs text-muted-foreground">{{ t('branding.requirements') }}</p>
                </div>
                <div class="flex items-center gap-2">
                  <Button size="sm" variant="outline" @click="triggerLogoUpload" :disabled="logoUploading">
                    <Upload class="h-3.5 w-3.5 mr-1.5" />
                    {{ logoUploading ? t('branding.uploading') : (logoUrl ? t('branding.changeLogo') : t('branding.uploadLogo')) }}
                  </Button>
                  <Button v-if="logoUrl" size="sm" variant="destructive" @click="deleteLogo" :disabled="logoDeleting">
                    <Trash2 class="h-3.5 w-3.5 mr-1.5" />
                    {{ logoDeleting ? t('branding.deleting') : t('branding.deleteLogo') }}
                  </Button>
                </div>
              </div>
            </div>

            <!-- Hidden file input -->
            <input
              ref="logoFileInput"
              type="file"
              accept="image/png,image/jpeg,image/webp"
              class="hidden"
              @change="handleLogoFileChange"
            />

            <!-- New file selected preview -->
            <div v-if="logoPreviewUrl && logoSelectedFile" class="rounded-lg border bg-muted/30 p-3 space-y-2">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <CheckCircle class="h-4 w-4 text-emerald-500" />
                  <span class="text-sm font-medium">{{ logoSelectedFile.name }}</span>
                  <span class="text-xs text-muted-foreground">({{ formatBytes(logoSelectedFile.size) }})</span>
                </div>
                <Button size="sm" variant="ghost" @click="clearLogoPreview">
                  <X class="h-3.5 w-3.5" />
                </Button>
              </div>
              <Button size="sm" @click="uploadLogo" :disabled="logoUploading">
                {{ logoUploading ? t('branding.uploading') : t('branding.confirmUpload') }}
              </Button>
            </div>

            <!-- Inline checkout preview (matches checkout hero card) -->
            <div class="rounded-2xl border bg-background shadow-sm overflow-hidden">
              <div class="h-1.5 w-full bg-blue-600"></div>
              <div class="p-4 flex items-center gap-3">
                <div v-if="logoPreviewUrl || logoUrl" class="w-12 h-12 rounded-xl bg-muted/50 border flex items-center justify-center overflow-hidden shrink-0">
                  <img :src="logoPreviewUrl || logoUrl!" alt="Preview" class="w-10 h-10 object-contain" />
                </div>
                <div v-else class="w-12 h-12 rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shrink-0">
                  <span class="text-lg font-bold text-white">{{ (userStore.orgName || 'M').charAt(0).toUpperCase() }}</span>
                </div>
                <div class="min-w-0 flex-1">
                  <p class="text-base font-bold truncate">{{ userStore.orgName || 'Your Merchant' }}</p>
                  <div class="flex items-center gap-1.5 mt-0.5">
                    <ShieldCheck class="w-3.5 h-3.5 text-emerald-500 shrink-0" />
                    <span class="text-xs font-medium text-emerald-600 dark:text-emerald-400">{{ t('branding.verifiedMerchant') }}</span>
                  </div>
                </div>
              </div>
              <div class="px-4 pb-3">
                <p class="text-[11px] text-muted-foreground">{{ t('branding.previewHint') }}</p>
              </div>
            </div>
          </div>
        </template>
      </CardContent>
    </Card>

    <!-- Members Table -->
    <Card class="animate-fade-in-up" style="animation-delay: 80ms">
      <CardContent class="p-0">
        <div v-if="loading" class="flex items-center justify-center py-16 text-muted-foreground">
          <Loader2 class="h-5 w-5 animate-spin mr-2" />
          {{ t('team.loading') }}
        </div>

        <div v-else-if="members.length === 0" class="flex flex-col items-center justify-center py-16 text-muted-foreground">
          <Users class="h-10 w-10 mb-3 opacity-40" />
          <p>{{ t('team.noMembers') }}</p>
        </div>

        <table v-else class="w-full">
          <thead>
            <tr class="border-b text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/60">
              <th class="px-4 py-3">{{ t('team.member') }}</th>
              <th class="px-4 py-3">{{ t('team.role') }}</th>
              <th class="px-4 py-3">{{ t('team.status') }}</th>
              <th class="px-4 py-3 text-right">{{ t('team.actions') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="m in members"
              :key="m.id"
              class="border-b last:border-b-0 hover:bg-accent/30 transition-colors"
            >
              <!-- Member info -->
              <td class="px-4 py-3">
                <div class="flex items-center gap-3">
                  <div class="flex h-8 w-8 items-center justify-center rounded-full bg-brand/10 text-xs font-semibold text-brand shrink-0">
                    {{ nameInitials(m) }}
                  </div>
                  <div class="min-w-0">
                    <p class="text-sm font-medium truncate">{{ m.name || m.email }}</p>
                    <p v-if="m.name" class="text-[11px] text-muted-foreground truncate">{{ m.email }}</p>
                  </div>
                </div>
              </td>

              <!-- Role badge -->
              <td class="px-4 py-3">
                <span :class="roleBadgeClass(m.role)" class="inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium capitalize">
                  {{ t(`team.role.${m.role}`) }}
                </span>
              </td>

              <!-- Status -->
              <td class="px-4 py-3">
                <span :class="statusBadgeClass(m.status)" class="inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium capitalize">
                  {{ t(`team.status.${m.status}`) }}
                </span>
              </td>

              <!-- Actions -->
              <td class="px-4 py-3 text-right">
                <div class="flex items-center justify-end gap-1">
                  <!-- Role change (owner only, not on self) -->
                  <Select
                    v-if="userStore.orgRole === 'owner' && m.user_id !== userStore.userId && m.status === 'active' && m.role !== 'owner'"
                    :model-value="m.role"
                    @update:model-value="(val: any) => changeRole(m, String(val))"
                  >
                    <SelectTrigger class="h-7 w-[110px] text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="admin">Admin</SelectItem>
                      <SelectItem value="developer">Developer</SelectItem>
                      <SelectItem value="finance">Finance</SelectItem>
                      <SelectItem value="viewer">Viewer</SelectItem>
                    </SelectContent>
                  </Select>

                  <!-- Remove -->
                  <Button
                    v-if="m.user_id !== userStore.userId"
                    variant="ghost"
                    size="sm"
                    class="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
                    :title="m.status === 'pending' ? t('team.revoke') : t('team.remove')"
                    @click="confirmRemove(m)"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
                  </Button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </CardContent>
    </Card>

    <!-- Invite Dialog -->
    <Dialog :open="showInviteDialog" @update:open="showInviteDialog = $event">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('team.inviteTitle') }}</DialogTitle>
          <DialogDescription>{{ t('team.inviteDesc') }}</DialogDescription>
        </DialogHeader>

        <form class="space-y-4" @submit.prevent="sendInvite">
          <div class="space-y-1.5">
            <Label for="invite-email">{{ t('team.email') }}</Label>
            <Input
              id="invite-email"
              v-model="inviteForm.email"
              type="email"
              placeholder="colleague@company.com"
              autofocus
            />
          </div>
          <div class="space-y-1.5">
            <Label for="invite-role">{{ t('team.role') }}</Label>
            <Select v-model="inviteForm.role">
              <SelectTrigger id="invite-role" class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="admin">Admin</SelectItem>
                <SelectItem value="developer">Developer</SelectItem>
                <SelectItem value="finance">Finance</SelectItem>
                <SelectItem value="viewer">Viewer</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="flex justify-end gap-2 pt-2">
            <Button variant="outline" type="button" @click="showInviteDialog = false">
              {{ t('team.cancel') }}
            </Button>
            <Button type="submit" :disabled="inviting">
              <Loader2 v-if="inviting" class="h-4 w-4 mr-2 animate-spin" />
              {{ t('team.sendInvite') }}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>

    <!-- Remove Confirm Dialog -->
    <Dialog :open="showRemoveDialog" @update:open="showRemoveDialog = $event">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ t('team.removeTitle') }}</DialogTitle>
          <DialogDescription>
            {{ t('team.removeDesc', { name: removingMember?.name || removingMember?.email || '' }) }}
          </DialogDescription>
        </DialogHeader>
        <div class="flex justify-end gap-2 pt-2">
          <Button variant="outline" @click="showRemoveDialog = false">{{ t('team.cancel') }}</Button>
          <Button variant="destructive" :disabled="removing" @click="doRemove">
            <Loader2 v-if="removing" class="h-4 w-4 mr-2 animate-spin" />
            {{ t('team.confirmRemove') }}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted, reactive, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { toast } from 'vue-sonner';
import { Users, UserPlus, Trash2, Loader2, Building2, Pencil, Palette, Upload, Image as ImageIcon, ShieldCheck, CheckCircle, X } from 'lucide-vue-next';
import { useUserStore, useEnvironmentStore } from '@/stores';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { http } from '@/utils/request';
import { inviteMember, listMembers, changeMemberRole, removeMember, type TeamMember } from '@/api/team';

const { t } = useI18n();
const userStore = useUserStore();
const envStore = useEnvironmentStore();
const isSandbox = computed(() => envStore.isSandbox);

const loading = ref(true);
const members = ref<TeamMember[]>([]);

async function fetchMembers() {
  loading.value = true;
  try {
    const res = await listMembers();
    members.value = res.members;
  } catch {
    // interceptor handles error
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  fetchMembers();
  fetchBranding();
});

// ── Branding / Logo ──
const logoUrl = ref<string | null>(null);
const logoPreviewUrl = ref<string | null>(null);
const logoSelectedFile = ref<File | null>(null);
const logoUploading = ref(false);
const logoDeleting = ref(false);
const logoFileInput = ref<HTMLInputElement | null>(null);

async function fetchBranding() {
  try {
    const res: any = await http.get('/api/internal/branding');
    logoUrl.value = res.logo_url || null;
  } catch {
    // silently fail — no logo yet
  }
}

function triggerLogoUpload() {
  logoFileInput.value?.click();
}

function handleLogoFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  if (file.size > 2 * 1024 * 1024) {
    toast.error(t('branding.fileTooLarge'));
    input.value = '';
    return;
  }
  if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) {
    toast.error(t('branding.invalidType'));
    input.value = '';
    return;
  }

  logoSelectedFile.value = file;
  logoPreviewUrl.value = URL.createObjectURL(file);
}

function clearLogoPreview() {
  if (logoPreviewUrl.value) URL.revokeObjectURL(logoPreviewUrl.value);
  logoPreviewUrl.value = null;
  logoSelectedFile.value = null;
  if (logoFileInput.value) logoFileInput.value.value = '';
}

async function uploadLogo() {
  if (!logoSelectedFile.value) return;
  logoUploading.value = true;
  try {
    const formData = new FormData();
    formData.append('logo', logoSelectedFile.value);
    const res: any = await http.post('/api/internal/branding/logo', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
    logoUrl.value = res.logo_url;
    clearLogoPreview();
    toast.success(t('branding.uploadSuccess'));
  } catch {
    // interceptor shows backend error
  } finally {
    logoUploading.value = false;
  }
}

async function deleteLogo() {
  logoDeleting.value = true;
  try {
    await http.delete('/api/internal/branding/logo');
    logoUrl.value = null;
    toast.success(t('branding.deleteSuccess'));
  } catch {
    // interceptor shows backend error
  } finally {
    logoDeleting.value = false;
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

// ── Org Name Editing ──
const editingOrgName = ref(false);
const orgNameInput = ref('');
const savingOrgName = ref(false);

function startEditOrgName() {
  orgNameInput.value = userStore.orgName || '';
  editingOrgName.value = true;
}

async function saveOrgName() {
  if (!orgNameInput.value.trim()) return;
  savingOrgName.value = true;
  try {
    await http.put('/api/internal/merchants/me', { name: orgNameInput.value.trim() });
    userStore.setInfo({ orgName: orgNameInput.value.trim() });
    editingOrgName.value = false;
    toast.success(t('team.orgNameUpdated'));
  } catch {
    // interceptor shows error
  } finally {
    savingOrgName.value = false;
  }
}

// ── Invite ──
const showInviteDialog = ref(false);
const inviting = ref(false);
const inviteForm = reactive({ email: '', role: 'developer' });

async function sendInvite() {
  if (!inviteForm.email.trim()) return;
  inviting.value = true;
  try {
    await inviteMember({ email: inviteForm.email.trim(), role: inviteForm.role });
    toast.success(t('team.inviteSent'));
    showInviteDialog.value = false;
    inviteForm.email = '';
    inviteForm.role = 'developer';
    await fetchMembers();
  } catch {
    // interceptor
  } finally {
    inviting.value = false;
  }
}

// ── Role Change ──
async function changeRole(member: TeamMember, newRole: string) {
  try {
    await changeMemberRole(member.id, newRole);
    toast.success(t('team.roleChanged'));
    await fetchMembers();
  } catch {
    // interceptor
  }
}

// ── Remove ──
const showRemoveDialog = ref(false);
const removing = ref(false);
const removingMember = ref<TeamMember | null>(null);

function confirmRemove(m: TeamMember) {
  removingMember.value = m;
  showRemoveDialog.value = true;
}

async function doRemove() {
  if (!removingMember.value) return;
  removing.value = true;
  try {
    await removeMember(removingMember.value.id);
    toast.success(t('team.removed'));
    showRemoveDialog.value = false;
    removingMember.value = null;
    await fetchMembers();
  } catch {
    // interceptor
  } finally {
    removing.value = false;
  }
}

// ── Helpers ──
function nameInitials(m: TeamMember): string {
  const name = m.name || m.email;
  return name
    .split(/[\s@]/)
    .map(w => w[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);
}

function roleBadgeClass(role: string): string {
  const map: Record<string, string> = {
    owner: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400',
    admin: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
    developer: 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400',
    finance: 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400',
    viewer: 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400',
  };
  return map[role] || map.viewer || '';
}

function statusBadgeClass(status: string): string {
  return status === 'active'
    ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
    : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400';
}
</script>
