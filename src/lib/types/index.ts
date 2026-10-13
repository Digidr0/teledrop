export type Theme = 'light' | 'dark' | 'system';
export type ConnectionState =
  | 'not_configured'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'authentication_error'
  | 'network_error'
  | 'stopped';
export type DownloadStatus =
  | 'queued'
  | 'downloading'
  | 'finalizing'
  | 'completed'
  | 'failed'
  | 'cancelled';
export interface Settings {
  destination: string;
  separateFolders: boolean;
  concurrency: number;
  theme: Theme;
  notifications: boolean;
  skipDuplicates: boolean;
  autoConnect: boolean;
  allowedChatIds: number[];
}
export interface Connection {
  running: boolean;
  state: ConnectionState;
  message: string;
  botUsername: string | null;
  botId: number | null;
  lastSuccess: number | null;
}
export interface Download {
  id: number;
  botId: number;
  updateId: number;
  chatId: number;
  messageId: number;
  mediaGroupId: string | null;
  filename: string;
  mediaType: 'photo' | 'video';
  totalBytes: number | null;
  downloadedBytes: number;
  speed: number;
  status: DownloadStatus;
  destination: string;
  finalPath: string | null;
  error: string | null;
  attempts: number;
  createdAt: number;
  startedAt: number | null;
  completedAt: number | null;
}
export interface Counts {
  queued: number;
  downloading: number;
  completed: number;
  failed: number;
  cancelled: number;
}
export interface Activity {
  id: number;
  time: number;
  level: string;
  message: string;
}
export interface Snapshot {
  settings: Settings;
  connection: Connection;
  counts: Counts;
  downloads: Download[];
  activity: Activity[];
  tokenConfigured: boolean;
  version: string;
}
export interface HistoryQuery {
  search: string;
  status: string;
  mediaType: string;
  page: number;
}
export interface HistoryPage {
  items: Download[];
  total: number;
}
export type Page =
  'overview' | 'downloads' | 'history' | 'settings' | 'activity';
