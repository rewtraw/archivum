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
  tags: string[];
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

export async function listDocuments(
  offset = 0,
  limit = 50
): Promise<Document[]> {
  return invoke("list_documents", { offset, limit });
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
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function saveSettings(
  apiKey?: string,
  model?: string
): Promise<Settings> {
  return invoke("save_settings", {
    apiKey: apiKey ?? null,
    model: model ?? null,
  });
}

export async function validateApiKey(apiKey: string): Promise<boolean> {
  return invoke("validate_api_key", { apiKey });
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
