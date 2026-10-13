<script lang="ts">
  import { HugeiconsIcon } from '@hugeicons/svelte';
  import Home from '@hugeicons/core-free-icons/Home01Icon';
  import Download from '@hugeicons/core-free-icons/Download04Icon';
  import Clock from '@hugeicons/core-free-icons/Clock01Icon';
  import Settings from '@hugeicons/core-free-icons/Settings01Icon';
  import Folder from '@hugeicons/core-free-icons/Folder01Icon';
  import FolderOpen from '@hugeicons/core-free-icons/FolderOpenIcon';
  import Image from '@hugeicons/core-free-icons/Image01Icon';
  import Video from '@hugeicons/core-free-icons/Video01Icon';
  import Success from '@hugeicons/core-free-icons/CheckmarkCircle02Icon';
  import Error from '@hugeicons/core-free-icons/AlertCircleIcon';
  import Search from '@hugeicons/core-free-icons/Search01Icon';
  import Refresh from '@hugeicons/core-free-icons/RefreshIcon';
  import Cancel from '@hugeicons/core-free-icons/Cancel01Icon';
  import Sun from '@hugeicons/core-free-icons/Sun01Icon';
  import Moon from '@hugeicons/core-free-icons/Moon02Icon';
  import Notification from '@hugeicons/core-free-icons/Notification01Icon';
  import FileSignal from '@hugeicons/core-free-icons/FileSignalIcon';
  import Telegram from '@hugeicons/core-free-icons/TelegramIcon';
  const icons = {
    home: Home,
    download: Download,
    clock: Clock,
    settings: Settings,
    folder: Folder,
    folderOpen: FolderOpen,
    photo: Image,
    video: Video,
    success: Success,
    error: Error,
    search: Search,
    refresh: Refresh,
    cancel: Cancel,
    sun: Sun,
    moon: Moon,
    notification: Notification,
    connection: FileSignal,
    telegram: Telegram,
  };
  let { name, size = 20 }: { name: keyof typeof icons; size?: number } =
    $props();
</script>

<span class="icon" aria-hidden="true"
  ><HugeiconsIcon
    icon={icons[name]}
    {size}
    strokeWidth={1.6}
    color="currentColor"
  /></span
>
