import { invoke } from "@tauri-apps/api/core";

export interface Document {
  id: string;
  title: string;
  author: string;
  isbn: string | null;
  language: string | null;
  publisher: string | null;
  published_date: string | null;
  description: string | null;
  page_count: number | null;
  original_format: string;
  file_hash: string;
  file_size: number;
  original_path: string;
  markdown_path: string | null;
  cover_path: string | null;
  status: string;
  error_message: string | null;
  imported_at: string;
  updated_at: string;
  processed_at: string | null;
  reading_status: string | null;
  reading_progress: number | null;
  last_read_at: string | null;
  tags: string[];
  duration_seconds: number | null;
  source_url: string | null;
}

export interface SearchResult {
  id: string;
  title: string;
  author: string;
  original_format: string;
  snippet: string;
  cover_path: string | null;
}

export interface ImportResult {
  document_id: string;
  task_id: string;
  filename: string;
}

export interface Task {
  id: string;
  document_id: string | null;
  task_type: string;
  status: string;
  progress: number;
  message: string | null;
  error: string | null;
  created_at: string;
}

export interface LibraryStats {
  total_documents: number;
  total_size_bytes: number;
  format_counts: [string, number][];
  status_counts: [string, number][];
}

export async function importFiles(paths: string[]): Promise<ImportResult[]> {
  return invoke("import_files", { paths });
}

export interface ListDocumentsOptions {
  offset?: number;
  limit?: number;
  sortBy?: string;
  sortDir?: string;
  formatFilter?: string;
  statusFilter?: string;
  tagFilter?: string;
}

export async function listDocuments(
  opts: ListDocumentsOptions = {}
): Promise<Document[]> {
  return invoke("list_documents", {
    offset: opts.offset ?? 0,
    limit: opts.limit ?? 50,
    sortBy: opts.sortBy ?? null,
    sortDir: opts.sortDir ?? null,
    formatFilter: opts.formatFilter ?? null,
    statusFilter: opts.statusFilter ?? null,
    tagFilter: opts.tagFilter ?? null,
  });
}

export async function getDocument(id: string): Promise<Document | null> {
  return invoke("get_document", { id });
}

export async function deleteDocument(id: string): Promise<void> {
  return invoke("delete_document", { id });
}

export async function searchDocuments(
  query: string,
  limit = 20
): Promise<SearchResult[]> {
  return invoke("search_documents", { query, limit });
}

export async function getStats(): Promise<LibraryStats> {
  return invoke("get_stats");
}

export async function getDocumentMarkdown(id: string): Promise<string> {
  return invoke("get_document_markdown", { id });
}

export async function getTasks(limit = 20): Promise<Task[]> {
  return invoke("get_tasks", { limit });
}

export async function deleteTask(id: string): Promise<void> {
  return invoke("delete_task", { id });
}

export async function clearFinishedTasks(): Promise<void> {
  return invoke("clear_finished_tasks");
}

export interface Settings {
  has_api_key: boolean;
  api_key_preview: string;
  model: string;
  has_cloudflare: boolean;
  cloudflare_account_id_preview: string;
  selected_whisper_model: string | null;
  ai_provider: string;
  ollama_model: string;
  ollama_base_url: string;
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function saveSettings(opts: {
  apiKey?: string;
  model?: string;
  cloudflareAccountId?: string;
  cloudflareApiToken?: string;
  aiProvider?: string;
  ollamaModel?: string;
  ollamaBaseUrl?: string;
} = {}): Promise<Settings> {
  return invoke("save_settings", {
    apiKey: opts.apiKey ?? null,
    model: opts.model ?? null,
    cloudflareAccountId: opts.cloudflareAccountId ?? null,
    cloudflareApiToken: opts.cloudflareApiToken ?? null,
    aiProvider: opts.aiProvider ?? null,
    ollamaModel: opts.ollamaModel ?? null,
    ollamaBaseUrl: opts.ollamaBaseUrl ?? null,
  });
}

export async function importUrl(url: string): Promise<ImportResult> {
  return invoke("import_url", { url });
}

export async function validateApiKey(apiKey: string): Promise<boolean> {
  return invoke("validate_api_key", { apiKey });
}

export async function getMobiHtml(id: string): Promise<string> {
  return invoke("get_mobi_html", { id });
}

export async function getDocumentCover(id: string): Promise<number[]> {
  return invoke("get_document_cover", { id });
}

export async function getOriginalBytes(id: string): Promise<number[]> {
  return invoke("get_original_bytes", { id });
}

export async function getOriginalPath(id: string): Promise<string> {
  return invoke("get_original_path", { id });
}

export interface SourceChunk {
  content: string;
  chunk_index: number;
  distance: number;
  document_id?: string;
  document_title?: string;
}

export interface ChatEvent {
  event: "token" | "done" | "error" | "context";
  data: {
    text?: string;
    full_text?: string;
    message?: string;
    chunks?: SourceChunk[];
  };
}

export interface ChatSession {
  id: string;
  title: string;
  document_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface ChatMessageRecord {
  id: number;
  session_id: string;
  role: string;
  content: string;
  sources: string | null;
  created_at: string;
}

export async function askDocument(
  documentId: string,
  question: string,
  onEvent: (event: ChatEvent) => void
): Promise<void> {
  const { Channel } = await import("@tauri-apps/api/core");
  const channel = new Channel<ChatEvent>();
  channel.onmessage = onEvent;
  return invoke("ask_document", {
    documentId,
    question,
    onToken: channel,
  });
}

export async function getDocumentHasChunks(
  documentId: string
): Promise<boolean> {
  return invoke("get_document_has_chunks", { documentId });
}

export async function reembedDocument(documentId: string): Promise<void> {
  return invoke("reembed_document", { documentId });
}

export async function askLibrary(
  question: string,
  onEvent: (event: ChatEvent) => void
): Promise<void> {
  const { Channel } = await import("@tauri-apps/api/core");
  const channel = new Channel<ChatEvent>();
  channel.onmessage = onEvent;
  return invoke("ask_library", { question, onToken: channel });
}

export async function createChatSession(
  title?: string,
  documentId?: string
): Promise<ChatSession> {
  return invoke("create_chat_session", {
    title: title ?? null,
    documentId: documentId ?? null,
  });
}

export async function listChatSessions(
  documentId?: string | null,
  limit = 50
): Promise<ChatSession[]> {
  return invoke("list_chat_sessions", {
    documentId: documentId ?? null,
    limit,
  });
}

export async function getChatMessages(
  sessionId: string
): Promise<ChatMessageRecord[]> {
  return invoke("get_chat_messages", { sessionId });
}

export async function deleteChatSession(sessionId: string): Promise<void> {
  return invoke("delete_chat_session", { sessionId });
}

export async function saveChatMessage(
  sessionId: string,
  role: string,
  content: string,
  sources?: string
): Promise<void> {
  return invoke("save_chat_message", {
    sessionId,
    role,
    content,
    sources: sources ?? null,
  });
}

export async function updateSessionTitle(
  sessionId: string,
  title: string
): Promise<void> {
  return invoke("update_session_title", { sessionId, title });
}

export async function searchSemantic(
  query: string,
  limit = 20
): Promise<SearchResult[]> {
  return invoke("search_semantic", { query, limit });
}

export async function getRelatedDocuments(
  documentId: string,
  limit = 5
): Promise<SearchResult[]> {
  return invoke("get_related_documents", { documentId, limit });
}

export interface EmbeddingStatsResult {
  total_documents: number;
  embedded_documents: number;
}

export async function getEmbeddingStats(): Promise<EmbeddingStatsResult> {
  return invoke("get_embedding_stats");
}

export async function batchReembed(): Promise<string> {
  return invoke("batch_reembed");
}

export async function regenerateCovers(): Promise<string> {
  return invoke("regenerate_covers");
}

// --- Reading progress ---

export async function saveReadingProgress(
  id: string,
  position: number
): Promise<void> {
  return invoke("save_reading_progress", { id, position });
}

export async function getReadingProgress(
  id: string
): Promise<number | null> {
  return invoke("get_reading_progress", { id });
}

// --- Table of contents ---

export interface TocEntry {
  title: string;
  level: number;
  href: string | null;
}

export async function getDocumentToc(id: string): Promise<TocEntry[]> {
  return invoke("get_document_toc", { id });
}

// --- Summaries ---

export async function getSummary(
  id: string,
  length: string
): Promise<string | null> {
  return invoke("get_summary", { id, length });
}

export async function getAllSummaries(
  id: string
): Promise<[string, string][]> {
  return invoke("get_all_summaries", { id });
}

export async function generateSummary(
  id: string,
  length: string
): Promise<string> {
  return invoke("generate_summary", { id, length });
}

// --- Tags ---

export async function listTags(): Promise<string[]> {
  return invoke("list_tags");
}

export async function updateReadingStatus(
  id: string,
  readingStatus: string | null
): Promise<void> {
  return invoke("update_reading_status", { id, readingStatus });
}

export function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function formatDate(dateStr: string): string {
  const date = new Date(dateStr + "Z");
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

// --- Whisper models ---

export interface WhisperModel {
  id: string;
  name: string;
  filename: string;
  size_bytes: number;
  status: string;
  download_progress: number;
  downloaded_at: string | null;
  error: string | null;
}

export interface ExternalToolsStatus {
  yt_dlp_available: boolean;
  yt_dlp_path: string | null;
  ffmpeg_available: boolean;
  ffmpeg_path: string | null;
}

export async function listWhisperModels(): Promise<WhisperModel[]> {
  return invoke("list_whisper_models");
}

export async function downloadWhisperModel(modelId: string): Promise<string> {
  return invoke("download_whisper_model", { modelId });
}

export async function deleteWhisperModel(modelId: string): Promise<void> {
  return invoke("delete_whisper_model", { modelId });
}

export async function selectWhisperModel(modelId: string): Promise<void> {
  return invoke("select_whisper_model", { modelId });
}

export async function checkExternalTools(): Promise<ExternalToolsStatus> {
  return invoke("check_external_tools");
}

export async function importYoutube(url: string): Promise<ImportResult> {
  return invoke("import_youtube", { url });
}

// --- Ollama ---

export interface OllamaStatus {
  available: boolean;
  version: string | null;
}

export interface OllamaModelInfo {
  name: string;
  size: number;
  modified_at: string;
  parameter_size: string | null;
  family: string | null;
}

export interface RecommendedModel {
  name: string;
  label: string;
  description: string;
}

export async function checkOllamaStatus(): Promise<OllamaStatus> {
  return invoke("check_ollama_status");
}

export async function listOllamaModels(): Promise<OllamaModelInfo[]> {
  return invoke("list_ollama_models");
}

export async function listRecommendedOllamaModels(): Promise<RecommendedModel[]> {
  return invoke("list_recommended_ollama_models");
}

export async function pullOllamaModel(name: string): Promise<string> {
  return invoke("pull_ollama_model", { name });
}

export async function deleteOllamaModel(name: string): Promise<void> {
  return invoke("delete_ollama_model", { name });
}

export interface HardwareInfo {
  total_ram_gb: number;
  cpu_name: string;
  gpu_name: string | null;
  gpu_vram_gb: number | null;
  unified_memory: boolean;
  backend: string;
}

export interface ModelFitInfo {
  name: string;
  parameter_count: string;
  use_case: string;
  fit_level: string;
  run_mode: string;
  memory_required_gb: number;
  estimated_tps: number;
  best_quant: string;
  score: number;
  score_quality: number;
  score_speed: number;
  score_fit: number;
  score_context: number;
  context_length: number;
  installed: boolean;
}

export async function getSystemHardware(): Promise<HardwareInfo> {
  return invoke("get_system_hardware");
}

export async function getModelFits(
  limit?: number,
  useCaseFilter?: string
): Promise<ModelFitInfo[]> {
  return invoke("get_model_fits", {
    limit: limit ?? null,
    useCaseFilter: useCaseFilter ?? null,
  });
}

export function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}
